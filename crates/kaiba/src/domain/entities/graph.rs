//! GraphNode & GraphEdge - Knowledge Graph entities for GraphKai
//!
//! Represents nodes and edges in the Neo4j knowledge graph.
//! Derived from DocStore + EmphasisParser output.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of node in the knowledge graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Key concept extracted from emphasis
    Concept,
    /// Named entity (person, place, organization)
    Entity,
    /// Tag/category label
    Tag,
    /// Document reference
    Document,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Concept => write!(f, "Concept"),
            NodeType::Entity => write!(f, "Entity"),
            NodeType::Tag => write!(f, "Tag"),
            NodeType::Document => write!(f, "Document"),
        }
    }
}

/// Type of edge connecting nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Semantic similarity based on embeddings
    SimilarTo,
    /// Co-occurrence in same context window
    CoOccursWith,
    /// Tag membership relationship
    BelongsTo,
    /// Extracted from document
    ExtractedFrom,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::SimilarTo => write!(f, "SIMILAR_TO"),
            EdgeType::CoOccursWith => write!(f, "CO_OCCURS_WITH"),
            EdgeType::BelongsTo => write!(f, "BELONGS_TO"),
            EdgeType::ExtractedFrom => write!(f, "EXTRACTED_FROM"),
        }
    }
}

/// A node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique identifier
    pub id: Uuid,
    /// Rei this node belongs to
    pub rei_id: Uuid,
    /// Text content of the node
    pub text: String,
    /// Type of node
    pub node_type: NodeType,
    /// Semantic weight (from emphasis style)
    pub weight: f32,
    /// Contextual embedding vector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Source document ID (if extracted from document)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_doc_id: Option<Uuid>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl GraphNode {
    /// Create a new concept node
    pub fn concept(rei_id: Uuid, text: String, weight: f32, source_doc_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            rei_id,
            text,
            node_type: NodeType::Concept,
            weight,
            embedding: None,
            source_doc_id,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    /// Create a new tag node
    pub fn tag(rei_id: Uuid, name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            rei_id,
            text: name,
            node_type: NodeType::Tag,
            weight: 1.0,
            embedding: None,
            source_doc_id: None,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    /// Create a new document node
    pub fn document(rei_id: Uuid, doc_id: Uuid, title: String) -> Self {
        Self {
            id: doc_id,
            rei_id,
            text: title,
            node_type: NodeType::Document,
            weight: 1.0,
            embedding: None,
            source_doc_id: None,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    /// Set the embedding vector
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// An edge connecting two nodes in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Unique identifier
    pub id: Uuid,
    /// Source node ID
    pub from_id: Uuid,
    /// Target node ID
    pub to_id: Uuid,
    /// Type of relationship
    pub edge_type: EdgeType,
    /// Relationship strength (0.0 - 1.0)
    pub strength: f32,
    /// Additional metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl GraphEdge {
    /// Create a new edge
    pub fn new(from_id: Uuid, to_id: Uuid, edge_type: EdgeType, strength: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_id,
            to_id,
            edge_type,
            strength: strength.clamp(0.0, 1.0),
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    /// Create a similarity edge
    pub fn similar_to(from_id: Uuid, to_id: Uuid, strength: f32) -> Self {
        Self::new(from_id, to_id, EdgeType::SimilarTo, strength)
    }

    /// Create a co-occurrence edge
    pub fn co_occurs_with(from_id: Uuid, to_id: Uuid, strength: f32) -> Self {
        Self::new(from_id, to_id, EdgeType::CoOccursWith, strength)
    }

    /// Create a tag membership edge
    pub fn belongs_to(node_id: Uuid, tag_id: Uuid) -> Self {
        Self::new(node_id, tag_id, EdgeType::BelongsTo, 1.0)
    }

    /// Create a document extraction edge
    pub fn extracted_from(node_id: Uuid, doc_id: Uuid) -> Self {
        Self::new(node_id, doc_id, EdgeType::ExtractedFrom, 1.0)
    }
}

/// A path through the graph (for traversal results)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// Nodes in the path (in order)
    pub nodes: Vec<GraphNode>,
    /// Edges connecting the nodes
    pub edges: Vec<GraphEdge>,
    /// Total path weight (product of edge strengths)
    pub total_weight: f32,
}

impl GraphPath {
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        let total_weight = edges.iter().map(|e| e.strength).product();
        Self {
            nodes,
            edges,
            total_weight,
        }
    }

    /// Get the start node
    pub fn start(&self) -> Option<&GraphNode> {
        self.nodes.first()
    }

    /// Get the end node
    pub fn end(&self) -> Option<&GraphNode> {
        self.nodes.last()
    }

    /// Get path length (number of edges)
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_concept_node() {
        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let node = GraphNode::concept(rei_id, "important concept".to_string(), 1.0, Some(doc_id));

        assert_eq!(node.rei_id, rei_id);
        assert_eq!(node.text, "important concept");
        assert_eq!(node.node_type, NodeType::Concept);
        assert_eq!(node.weight, 1.0);
        assert_eq!(node.source_doc_id, Some(doc_id));
    }

    #[test]
    fn test_create_edge() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let edge = GraphEdge::similar_to(from_id, to_id, 0.85);

        assert_eq!(edge.from_id, from_id);
        assert_eq!(edge.to_id, to_id);
        assert_eq!(edge.edge_type, EdgeType::SimilarTo);
        assert_eq!(edge.strength, 0.85);
    }

    #[test]
    fn test_edge_strength_clamping() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();

        let edge_high = GraphEdge::new(from_id, to_id, EdgeType::SimilarTo, 1.5);
        assert_eq!(edge_high.strength, 1.0);

        let edge_low = GraphEdge::new(from_id, to_id, EdgeType::SimilarTo, -0.5);
        assert_eq!(edge_low.strength, 0.0);
    }

    #[test]
    fn test_graph_path() {
        let rei_id = Uuid::new_v4();
        let node1 = GraphNode::concept(rei_id, "A".to_string(), 1.0, None);
        let node2 = GraphNode::concept(rei_id, "B".to_string(), 1.0, None);
        let edge = GraphEdge::similar_to(node1.id, node2.id, 0.9);

        let path = GraphPath::new(vec![node1, node2], vec![edge]);

        assert_eq!(path.len(), 1);
        assert_eq!(path.total_weight, 0.9);
        assert_eq!(path.start().unwrap().text, "A");
        assert_eq!(path.end().unwrap().text, "B");
    }
}
