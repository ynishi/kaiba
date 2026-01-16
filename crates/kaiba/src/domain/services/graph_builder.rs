//! GraphBuilder - Converts EmphasisNodes to Knowledge Graph
//!
//! Transforms parsed emphasis from documents into GraphNodes and GraphEdges
//! based on LinkageConfig settings.

use uuid::Uuid;

use crate::domain::entities::{EmphasisNode, GraphEdge, GraphNode};
use crate::domain::value_objects::LinkageConfig;

/// Result of building a graph from a document
#[derive(Debug, Clone, Default)]
pub struct GraphBuildResult {
    /// Nodes created from emphasis
    pub nodes: Vec<GraphNode>,
    /// Co-occurrence edges between nodes
    pub co_occurrence_edges: Vec<GraphEdge>,
    /// Document node (represents the source document)
    pub doc_node: Option<GraphNode>,
    /// Extraction edges (node -> document)
    pub extraction_edges: Vec<GraphEdge>,
    /// Statistics
    pub stats: BuildStats,
}

/// Build statistics
#[derive(Debug, Clone, Default)]
pub struct BuildStats {
    /// Total emphasis nodes processed
    pub emphasis_processed: usize,
    /// Nodes created (after filtering)
    pub nodes_created: usize,
    /// Nodes skipped (below weight threshold)
    pub nodes_skipped: usize,
    /// Co-occurrence edges created
    pub co_occurrence_edges: usize,
}

/// Service for building knowledge graphs from documents
pub struct GraphBuilder {
    config: LinkageConfig,
}

impl GraphBuilder {
    /// Create a new GraphBuilder with the given configuration
    pub fn new(config: LinkageConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self {
            config: LinkageConfig::default(),
        }
    }

    /// Build graph nodes and edges from emphasis parse result
    pub fn build_from_emphasis(
        &self,
        rei_id: Uuid,
        doc_id: Uuid,
        doc_title: &str,
        emphasis_nodes: &[EmphasisNode],
    ) -> GraphBuildResult {
        let mut result = GraphBuildResult::default();
        result.stats.emphasis_processed = emphasis_nodes.len();

        // Create document node
        let doc_node = GraphNode::document(rei_id, doc_id, doc_title.to_string());
        result.doc_node = Some(doc_node.clone());

        // Convert emphasis nodes to graph nodes
        let mut created_nodes: Vec<GraphNode> = Vec::new();

        for emphasis in emphasis_nodes {
            let weight = self.config.weight_for_style(&emphasis.style);

            if self.config.should_create_node(weight) {
                let node = GraphNode::concept(
                    rei_id,
                    emphasis.text.clone(),
                    weight,
                    Some(doc_id),
                );

                // Create extraction edge (node -> document)
                let extraction_edge = GraphEdge::extracted_from(node.id, doc_id);
                result.extraction_edges.push(extraction_edge);

                created_nodes.push(node);
                result.stats.nodes_created += 1;
            } else {
                result.stats.nodes_skipped += 1;
            }
        }

        // Detect co-occurrences and create edges
        let co_occurrence_edges = self.detect_co_occurrences(&created_nodes, emphasis_nodes);
        result.stats.co_occurrence_edges = co_occurrence_edges.len();
        result.co_occurrence_edges = co_occurrence_edges;

        result.nodes = created_nodes;
        result
    }

    /// Detect co-occurrences between nodes based on position proximity
    fn detect_co_occurrences(
        &self,
        nodes: &[GraphNode],
        emphasis_nodes: &[EmphasisNode],
    ) -> Vec<GraphEdge> {
        let mut edges = Vec::new();

        // Build a map of node text to node for lookup
        let node_map: std::collections::HashMap<&str, &GraphNode> = nodes
            .iter()
            .map(|n| (n.text.as_str(), n))
            .collect();

        // Group emphasis nodes by proximity (same context window = co-occurrence)
        // We use line number as a simple proxy for context
        let mut line_groups: std::collections::HashMap<usize, Vec<&EmphasisNode>> =
            std::collections::HashMap::new();

        for emphasis in emphasis_nodes {
            let line = emphasis.position.line;
            // Group nodes within 5 lines of each other
            let group_key = line / 5;
            line_groups
                .entry(group_key)
                .or_default()
                .push(emphasis);
        }

        // Create co-occurrence edges for nodes in the same group
        for (_group, group_nodes) in line_groups.iter() {
            if group_nodes.len() < 2 {
                continue;
            }

            // Create edges between all pairs in the group
            for i in 0..group_nodes.len() {
                for j in (i + 1)..group_nodes.len() {
                    let node_a = node_map.get(group_nodes[i].text.as_str());
                    let node_b = node_map.get(group_nodes[j].text.as_str());

                    if let (Some(a), Some(b)) = (node_a, node_b) {
                        // Avoid self-loops and duplicate edges
                        if a.id != b.id {
                            let strength = self.calculate_co_occurrence_strength(
                                group_nodes[i],
                                group_nodes[j],
                            );
                            let edge = GraphEdge::co_occurs_with(a.id, b.id, strength);
                            edges.push(edge);
                        }
                    }
                }
            }
        }

        // Limit edges per node
        self.limit_edges_per_node(&mut edges);

        edges
    }

    /// Calculate co-occurrence strength based on emphasis weights and proximity
    fn calculate_co_occurrence_strength(
        &self,
        a: &EmphasisNode,
        b: &EmphasisNode,
    ) -> f32 {
        let weight_a = self.config.weight_for_style(&a.style);
        let weight_b = self.config.weight_for_style(&b.style);

        // Base strength from co-occurrence weight config
        let base = self.config.linkage_strategy.co_occurrence_weight;

        // Boost based on combined emphasis weights
        let weight_factor = (weight_a + weight_b) / 2.0;

        // Proximity factor (closer = stronger)
        let line_distance = (a.position.line as i32 - b.position.line as i32).unsigned_abs() as f32;
        let proximity_factor = 1.0 / (1.0 + line_distance * 0.1);

        (base * weight_factor * proximity_factor).min(1.0)
    }

    /// Limit the number of edges per node
    fn limit_edges_per_node(&self, edges: &mut Vec<GraphEdge>) {
        let max_edges = self.config.linkage_strategy.max_edges_per_node;

        // Count edges per node
        let mut edge_count: std::collections::HashMap<Uuid, usize> =
            std::collections::HashMap::new();

        // Sort by strength (descending) to keep strongest edges
        edges.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap());

        // Filter edges that exceed the limit
        edges.retain(|edge| {
            let from_count = *edge_count.get(&edge.from_id).unwrap_or(&0);
            let to_count = *edge_count.get(&edge.to_id).unwrap_or(&0);

            if from_count < max_edges && to_count < max_edges {
                *edge_count.entry(edge.from_id).or_insert(0) += 1;
                *edge_count.entry(edge.to_id).or_insert(0) += 1;
                true
            } else {
                false
            }
        });
    }

    /// Get the current configuration
    pub fn config(&self) -> &LinkageConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: LinkageConfig) {
        self.config = config;
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::TextPosition;
    use crate::domain::value_objects::EmphasisStyle;

    fn create_emphasis_node(
        doc_id: Uuid,
        text: &str,
        style: EmphasisStyle,
        line: usize,
    ) -> EmphasisNode {
        EmphasisNode::new(
            doc_id,
            text.to_string(),
            style,
            TextPosition::new(0, line, 1),
            format!("context around {}", text),
        )
    }

    #[test]
    fn test_build_from_emphasis() {
        let builder = GraphBuilder::with_defaults();
        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        let emphasis_nodes = vec![
            create_emphasis_node(doc_id, "important concept", EmphasisStyle::Bold, 1),
            create_emphasis_node(doc_id, "related idea", EmphasisStyle::Italic, 2),
            create_emphasis_node(doc_id, "code example", EmphasisStyle::Code, 5),
        ];

        let result = builder.build_from_emphasis(rei_id, doc_id, "Test Doc", &emphasis_nodes);

        assert!(result.doc_node.is_some());
        assert_eq!(result.stats.emphasis_processed, 3);
        assert_eq!(result.stats.nodes_created, 3); // All above min_weight
        assert!(!result.extraction_edges.is_empty());
    }

    #[test]
    fn test_weight_filtering() {
        let mut config = LinkageConfig::default();
        config.linkage_strategy.min_node_weight = 0.75; // Filter out italic (0.7)

        let builder = GraphBuilder::new(config);
        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        let emphasis_nodes = vec![
            create_emphasis_node(doc_id, "bold text", EmphasisStyle::Bold, 1),     // 1.0 - included
            create_emphasis_node(doc_id, "italic text", EmphasisStyle::Italic, 2), // 0.7 - excluded
        ];

        let result = builder.build_from_emphasis(rei_id, doc_id, "Test", &emphasis_nodes);

        assert_eq!(result.stats.nodes_created, 1);
        assert_eq!(result.stats.nodes_skipped, 1);
        assert_eq!(result.nodes[0].text, "bold text");
    }

    #[test]
    fn test_co_occurrence_detection() {
        let builder = GraphBuilder::with_defaults();
        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        // Two nodes on same line should co-occur
        let emphasis_nodes = vec![
            create_emphasis_node(doc_id, "concept A", EmphasisStyle::Bold, 1),
            create_emphasis_node(doc_id, "concept B", EmphasisStyle::Bold, 1),
            create_emphasis_node(doc_id, "concept C", EmphasisStyle::Bold, 100), // Far away
        ];

        let result = builder.build_from_emphasis(rei_id, doc_id, "Test", &emphasis_nodes);

        // A and B should have co-occurrence edge, C is too far
        assert!(!result.co_occurrence_edges.is_empty());

        // Check that A-B edge exists
        let ab_edge = result.co_occurrence_edges.iter().find(|e| {
            (e.from_id == result.nodes[0].id && e.to_id == result.nodes[1].id)
                || (e.from_id == result.nodes[1].id && e.to_id == result.nodes[0].id)
        });
        assert!(ab_edge.is_some());
    }

    #[test]
    fn test_edge_limit() {
        let mut config = LinkageConfig::default();
        config.linkage_strategy.max_edges_per_node = 2;

        let builder = GraphBuilder::new(config);
        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        // Create many nodes in same proximity
        let emphasis_nodes: Vec<EmphasisNode> = (0..10)
            .map(|i| create_emphasis_node(doc_id, &format!("concept {}", i), EmphasisStyle::Bold, 1))
            .collect();

        let result = builder.build_from_emphasis(rei_id, doc_id, "Test", &emphasis_nodes);

        // Should have limited edges per node
        let mut edge_count: std::collections::HashMap<Uuid, usize> =
            std::collections::HashMap::new();
        for edge in &result.co_occurrence_edges {
            *edge_count.entry(edge.from_id).or_insert(0) += 1;
            *edge_count.entry(edge.to_id).or_insert(0) += 1;
        }

        // No node should have more than max_edges_per_node edges
        for count in edge_count.values() {
            assert!(*count <= 2);
        }
    }
}
