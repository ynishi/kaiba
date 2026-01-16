//! LinkageConfig - Graph building configuration
//!
//! Defines thresholds and weights for GraphKai graph construction.
//! Can be loaded from YAML or set via API.

use serde::{Deserialize, Serialize};

/// Configuration for emphasis weight mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmphasisWeights {
    /// Weight for bold text (**text**)
    pub bold: f32,
    /// Weight for italic text (*text*)
    pub italic: f32,
    /// Weight for bold+italic text (***text***)
    pub bold_italic: f32,
    /// Weight for code text (`text`)
    pub code: f32,
}

impl Default for EmphasisWeights {
    fn default() -> Self {
        Self {
            bold: 1.0,
            italic: 0.7,
            bold_italic: 1.2,
            code: 0.8,
        }
    }
}

/// Configuration for linkage strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkageStrategy {
    /// Minimum similarity threshold for creating SIMILAR_TO edges (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Weight multiplier for co-occurrence edges
    pub co_occurrence_weight: f32,
    /// Weight for tag membership edges
    pub tag_membership_weight: f32,
    /// Maximum number of edges per node
    pub max_edges_per_node: usize,
    /// Decay factor for repeated emphasis in same paragraph
    pub decay_factor: f32,
    /// Minimum weight threshold for creating nodes
    pub min_node_weight: f32,
}

impl Default for LinkageStrategy {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.85,
            co_occurrence_weight: 0.6,
            tag_membership_weight: 1.0,
            max_edges_per_node: 20,
            decay_factor: 0.9,
            min_node_weight: 0.5,
        }
    }
}

/// Configuration for hybrid search behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Default search strategy
    pub default_strategy: SearchStrategy,
    /// Maximum graph traversal depth
    pub graph_depth: u32,
    /// Number of RAG results to retrieve
    pub rag_top_k: usize,
    /// Weight for graph results in final ranking
    pub graph_weight: f32,
    /// Weight for RAG results in final ranking
    pub rag_weight: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_strategy: SearchStrategy::Auto,
            graph_depth: 2,
            rag_top_k: 5,
            graph_weight: 0.6,
            rag_weight: 0.4,
        }
    }
}

/// Search strategy for hybrid queries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchStrategy {
    /// Graph traversal first, then RAG supplement
    GraphFirst,
    /// RAG search first, then graph expansion
    RagFirst,
    /// Execute both in parallel and merge
    Parallel,
    /// Automatically determine based on query
    #[default]
    Auto,
}

impl std::fmt::Display for SearchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchStrategy::GraphFirst => write!(f, "graph_first"),
            SearchStrategy::RagFirst => write!(f, "rag_first"),
            SearchStrategy::Parallel => write!(f, "parallel"),
            SearchStrategy::Auto => write!(f, "auto"),
        }
    }
}

/// Complete GraphKai configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkageConfig {
    /// Emphasis weight mapping
    #[serde(default)]
    pub emphasis_weights: EmphasisWeights,
    /// Linkage strategy settings
    #[serde(default)]
    pub linkage_strategy: LinkageStrategy,
    /// Search configuration
    #[serde(default)]
    pub search: SearchConfig,
}

impl LinkageConfig {
    /// Create a new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a strict config (high thresholds, fewer edges)
    pub fn strict() -> Self {
        Self {
            linkage_strategy: LinkageStrategy {
                similarity_threshold: 0.92,
                max_edges_per_node: 10,
                min_node_weight: 0.7,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a loose config (low thresholds, more edges)
    pub fn loose() -> Self {
        Self {
            linkage_strategy: LinkageStrategy {
                similarity_threshold: 0.75,
                max_edges_per_node: 50,
                min_node_weight: 0.3,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Get the weight for a given emphasis style
    pub fn weight_for_style(&self, style: &crate::domain::value_objects::EmphasisStyle) -> f32 {
        use crate::domain::value_objects::EmphasisStyle;
        match style {
            EmphasisStyle::Bold => self.emphasis_weights.bold,
            EmphasisStyle::Italic => self.emphasis_weights.italic,
            EmphasisStyle::BoldItalic => self.emphasis_weights.bold_italic,
            EmphasisStyle::Code => self.emphasis_weights.code,
        }
    }

    /// Check if a node should be created based on weight
    pub fn should_create_node(&self, weight: f32) -> bool {
        weight >= self.linkage_strategy.min_node_weight
    }

    /// Check if an edge should be created based on similarity
    pub fn should_create_edge(&self, similarity: f32) -> bool {
        similarity >= self.linkage_strategy.similarity_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::EmphasisStyle;

    #[test]
    fn test_default_config() {
        let config = LinkageConfig::new();
        assert_eq!(config.emphasis_weights.bold, 1.0);
        assert_eq!(config.linkage_strategy.similarity_threshold, 0.85);
        assert_eq!(config.search.default_strategy, SearchStrategy::Auto);
    }

    #[test]
    fn test_weight_for_style() {
        let config = LinkageConfig::new();
        assert_eq!(config.weight_for_style(&EmphasisStyle::Bold), 1.0);
        assert_eq!(config.weight_for_style(&EmphasisStyle::Italic), 0.7);
        assert_eq!(config.weight_for_style(&EmphasisStyle::BoldItalic), 1.2);
        assert_eq!(config.weight_for_style(&EmphasisStyle::Code), 0.8);
    }

    #[test]
    fn test_should_create_node() {
        let config = LinkageConfig::new();
        assert!(config.should_create_node(0.7));
        assert!(config.should_create_node(0.5));
        assert!(!config.should_create_node(0.4));
    }

    #[test]
    fn test_should_create_edge() {
        let config = LinkageConfig::new();
        assert!(config.should_create_edge(0.90));
        assert!(config.should_create_edge(0.85));
        assert!(!config.should_create_edge(0.80));
    }

    #[test]
    fn test_strict_config() {
        let config = LinkageConfig::strict();
        assert_eq!(config.linkage_strategy.similarity_threshold, 0.92);
        assert_eq!(config.linkage_strategy.max_edges_per_node, 10);
    }

    #[test]
    fn test_loose_config() {
        let config = LinkageConfig::loose();
        assert_eq!(config.linkage_strategy.similarity_threshold, 0.75);
        assert_eq!(config.linkage_strategy.max_edges_per_node, 50);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = LinkageConfig::new();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LinkageConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.emphasis_weights.bold, config.emphasis_weights.bold);
    }
}
