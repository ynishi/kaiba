//! Graph - GraphKai API models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use kaiba::{LinkageConfig, SearchStrategy};

// ============================================
// Request DTOs
// ============================================

/// Request to rebuild the knowledge graph
#[derive(Debug, Deserialize, ToSchema)]
pub struct RebuildGraphRequest {
    /// Optional custom linkage configuration
    /// If not provided, uses default configuration
    #[serde(default)]
    pub config: Option<LinkageConfig>,
    /// Only rebuild for specific document IDs
    /// If not provided, rebuilds entire graph
    #[serde(default)]
    pub doc_ids: Option<Vec<Uuid>>,
    /// If true, clear existing graph before rebuild
    #[serde(default)]
    pub clear_existing: bool,
}

/// Request for graph search/traversal
#[derive(Debug, Deserialize, ToSchema)]
pub struct GraphSearchRequest {
    /// Search query text
    pub query: String,
    /// Search strategy to use
    #[serde(default)]
    pub strategy: Option<SearchStrategy>,
    /// Maximum traversal depth
    #[serde(default)]
    pub depth: Option<u32>,
    /// Maximum results to return
    #[serde(default)]
    pub limit: Option<usize>,
}

// ============================================
// Response DTOs
// ============================================

/// Result of graph rebuild operation
#[derive(Debug, Serialize, ToSchema)]
pub struct RebuildGraphResponse {
    /// Number of documents processed
    pub documents_processed: usize,
    /// Number of nodes created
    pub nodes_created: usize,
    /// Number of edges created
    pub edges_created: usize,
    /// Number of nodes skipped (below threshold)
    pub nodes_skipped: usize,
    /// Errors encountered during rebuild
    pub errors: Vec<String>,
    /// Time taken in milliseconds
    pub duration_ms: u64,
}

/// Graph node summary for API responses
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphNodeSummary {
    pub id: Uuid,
    pub text: String,
    pub node_type: String,
    pub weight: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_doc_id: Option<Uuid>,
}

impl From<kaiba::GraphNode> for GraphNodeSummary {
    fn from(node: kaiba::GraphNode) -> Self {
        Self {
            id: node.id,
            text: node.text,
            node_type: node.node_type.to_string(),
            weight: node.weight,
            source_doc_id: node.source_doc_id,
        }
    }
}

/// Graph edge summary for API responses
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphEdgeSummary {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: String,
    pub strength: f32,
}

impl From<kaiba::GraphEdge> for GraphEdgeSummary {
    fn from(edge: kaiba::GraphEdge) -> Self {
        Self {
            from_id: edge.from_id,
            to_id: edge.to_id,
            edge_type: edge.edge_type.to_string(),
            strength: edge.strength,
        }
    }
}

/// Graph statistics response
#[derive(Debug, Serialize, ToSchema)]
pub struct GraphStatsResponse {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_type: std::collections::HashMap<String, usize>,
    pub edges_by_type: std::collections::HashMap<String, usize>,
}

impl From<kaiba::GraphStats> for GraphStatsResponse {
    fn from(stats: kaiba::GraphStats) -> Self {
        Self {
            total_nodes: stats.total_nodes,
            total_edges: stats.total_edges,
            nodes_by_type: stats
                .nodes_by_type
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            edges_by_type: stats
                .edges_by_type
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }
}

/// Node neighbors response
#[derive(Debug, Serialize, ToSchema)]
pub struct NodeNeighborsResponse {
    pub node: GraphNodeSummary,
    pub neighbors: Vec<GraphNodeSummary>,
    pub edges: Vec<GraphEdgeSummary>,
}
