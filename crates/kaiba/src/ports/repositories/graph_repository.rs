//! GraphRepository - Knowledge Graph storage interface
//!
//! Abstract interface for Neo4j graph operations in GraphKai.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{DomainError, EdgeType, GraphEdge, GraphNode, GraphPath, NodeType};

/// Query parameters for graph traversal
#[derive(Debug, Clone, Default)]
pub struct TraversalQuery {
    /// Maximum traversal depth
    pub max_depth: u32,
    /// Filter by edge types
    pub edge_types: Option<Vec<EdgeType>>,
    /// Minimum edge strength threshold
    pub min_strength: Option<f32>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

impl TraversalQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    pub fn with_min_strength(mut self, strength: f32) -> Self {
        self.min_strength = Some(strength);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Result of batch node operations
#[derive(Debug, Clone)]
pub struct NodeBatchResult {
    /// Number of nodes created
    pub created: usize,
    /// Number of nodes updated
    pub updated: usize,
    /// IDs that failed
    pub failed: Vec<Uuid>,
}

/// Result of batch edge operations
#[derive(Debug, Clone)]
pub struct EdgeBatchResult {
    /// Number of edges created
    pub created: usize,
    /// Number of edges that already existed
    pub existing: usize,
    /// Errors encountered
    pub failed: Vec<String>,
}

/// Abstract interface for knowledge graph operations
#[async_trait]
pub trait GraphRepository: Send + Sync {
    // ===================
    // Node Operations
    // ===================

    /// Upsert a single node (create or update)
    async fn upsert_node(&self, node: &GraphNode) -> Result<GraphNode, DomainError>;

    /// Upsert multiple nodes (batch operation)
    async fn upsert_nodes(&self, nodes: &[GraphNode]) -> Result<NodeBatchResult, DomainError>;

    /// Get a node by ID
    async fn get_node(&self, id: Uuid) -> Result<Option<GraphNode>, DomainError>;

    /// Find nodes by text (exact or fuzzy match)
    async fn find_nodes_by_text(
        &self,
        rei_id: Uuid,
        text: &str,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError>;

    /// Find nodes by type within a Rei
    async fn find_nodes_by_type(
        &self,
        rei_id: Uuid,
        node_type: NodeType,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError>;

    /// Delete a node and its connected edges
    async fn delete_node(&self, id: Uuid) -> Result<bool, DomainError>;

    /// Delete all nodes for a document (cascade delete)
    async fn delete_nodes_by_document(&self, doc_id: Uuid) -> Result<usize, DomainError>;

    // ===================
    // Edge Operations
    // ===================

    /// Create or update an edge
    async fn upsert_edge(&self, edge: &GraphEdge) -> Result<GraphEdge, DomainError>;

    /// Create multiple edges (batch operation)
    async fn upsert_edges(&self, edges: &[GraphEdge]) -> Result<EdgeBatchResult, DomainError>;

    /// Get edges from a node
    async fn get_edges_from(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError>;

    /// Get edges to a node
    async fn get_edges_to(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError>;

    /// Delete an edge
    async fn delete_edge(&self, from_id: Uuid, to_id: Uuid) -> Result<bool, DomainError>;

    // ===================
    // Traversal Operations
    // ===================

    /// Get immediate neighbors of a node
    async fn get_neighbors(
        &self,
        node_id: Uuid,
        depth: u32,
    ) -> Result<Vec<GraphNode>, DomainError>;

    /// Traverse the graph from a starting node
    async fn traverse(
        &self,
        start_id: Uuid,
        query: &TraversalQuery,
    ) -> Result<Vec<GraphPath>, DomainError>;

    // ===================
    // Similarity Search
    // ===================

    /// Find nodes similar to the given embedding
    async fn find_by_embedding(
        &self,
        rei_id: Uuid,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<(GraphNode, f32)>, DomainError>;

    // ===================
    // Maintenance
    // ===================

    /// Clear all graph data for a Rei
    async fn clear_rei_graph(&self, rei_id: Uuid) -> Result<usize, DomainError>;

    /// Get graph statistics for a Rei
    async fn get_stats(&self, rei_id: Uuid) -> Result<GraphStats, DomainError>;
}

/// Graph statistics
#[derive(Debug, Clone, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_type: std::collections::HashMap<NodeType, usize>,
    pub edges_by_type: std::collections::HashMap<EdgeType, usize>,
}
