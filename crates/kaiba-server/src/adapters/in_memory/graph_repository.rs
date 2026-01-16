//! In-Memory GraphRepository implementation for testing
//!
//! Thread-safe mock that stores all graph data in memory.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use kaiba::{
    DomainError, EdgeBatchResult, EdgeType, GraphEdge, GraphNode, GraphPath, GraphRepository,
    GraphStats, NodeBatchResult, NodeType, TraversalQuery,
};

/// In-memory implementation of GraphRepository for testing
pub struct InMemoryGraphRepository {
    nodes: RwLock<HashMap<Uuid, GraphNode>>,
    edges: RwLock<HashMap<(Uuid, Uuid), GraphEdge>>,
}

impl Default for InMemoryGraphRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryGraphRepository {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
        }
    }

    /// Get all nodes (for testing)
    pub fn get_all_nodes(&self) -> Vec<GraphNode> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    /// Get all edges (for testing)
    pub fn get_all_edges(&self) -> Vec<GraphEdge> {
        self.edges.read().unwrap().values().cloned().collect()
    }

    /// Clear all data
    pub fn clear(&self) {
        self.nodes.write().unwrap().clear();
        self.edges.write().unwrap().clear();
    }
}

#[async_trait]
impl GraphRepository for InMemoryGraphRepository {
    // ===================
    // Node Operations
    // ===================

    async fn upsert_node(&self, node: &GraphNode) -> Result<GraphNode, DomainError> {
        let mut nodes = self.nodes.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        nodes.insert(node.id, node.clone());
        Ok(node.clone())
    }

    async fn upsert_nodes(&self, nodes: &[GraphNode]) -> Result<NodeBatchResult, DomainError> {
        let mut store = self.nodes.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let mut created = 0;
        let mut updated = 0;

        for node in nodes {
            if store.contains_key(&node.id) {
                updated += 1;
            } else {
                created += 1;
            }
            store.insert(node.id, node.clone());
        }

        Ok(NodeBatchResult {
            created,
            updated,
            failed: vec![],
        })
    }

    async fn get_node(&self, id: Uuid) -> Result<Option<GraphNode>, DomainError> {
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        Ok(nodes.get(&id).cloned())
    }

    async fn find_nodes_by_text(
        &self,
        rei_id: Uuid,
        text: &str,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError> {
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let text_lower = text.to_lowercase();

        let results: Vec<GraphNode> = nodes
            .values()
            .filter(|n| {
                n.rei_id == rei_id
                    && n.text.to_lowercase().contains(&text_lower)
                    && node_type.as_ref().map_or(true, |t| &n.node_type == t)
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(results)
    }

    async fn find_nodes_by_type(
        &self,
        rei_id: Uuid,
        node_type: NodeType,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError> {
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let results: Vec<GraphNode> = nodes
            .values()
            .filter(|n| n.rei_id == rei_id && n.node_type == node_type)
            .take(limit)
            .cloned()
            .collect();

        Ok(results)
    }

    async fn delete_node(&self, id: Uuid) -> Result<bool, DomainError> {
        let mut nodes = self.nodes.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let mut edges = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        // Remove all edges connected to this node
        edges.retain(|(from, to), _| *from != id && *to != id);

        Ok(nodes.remove(&id).is_some())
    }

    async fn delete_nodes_by_document(&self, doc_id: Uuid) -> Result<usize, DomainError> {
        let mut nodes = self.nodes.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let mut edges = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        // Find nodes to delete
        let node_ids: Vec<Uuid> = nodes
            .values()
            .filter(|n| n.source_doc_id == Some(doc_id))
            .map(|n| n.id)
            .collect();

        // Remove edges connected to these nodes
        for node_id in &node_ids {
            edges.retain(|(from, to), _| from != node_id && to != node_id);
        }

        // Remove nodes
        let count = node_ids.len();
        for node_id in node_ids {
            nodes.remove(&node_id);
        }

        Ok(count)
    }

    // ===================
    // Edge Operations
    // ===================

    async fn upsert_edge(&self, edge: &GraphEdge) -> Result<GraphEdge, DomainError> {
        let mut edges = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        edges.insert((edge.from_id, edge.to_id), edge.clone());
        Ok(edge.clone())
    }

    async fn upsert_edges(&self, edges: &[GraphEdge]) -> Result<EdgeBatchResult, DomainError> {
        let mut store = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let mut created = 0;
        let mut existing = 0;

        for edge in edges {
            let key = (edge.from_id, edge.to_id);
            if store.contains_key(&key) {
                existing += 1;
            } else {
                created += 1;
            }
            store.insert(key, edge.clone());
        }

        Ok(EdgeBatchResult {
            created,
            existing,
            failed: vec![],
        })
    }

    async fn get_edges_from(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError> {
        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let results: Vec<GraphEdge> = edges
            .iter()
            .filter(|((from, _), e)| {
                *from == node_id && edge_type.as_ref().map_or(true, |t| &e.edge_type == t)
            })
            .map(|(_, e)| e.clone())
            .collect();

        Ok(results)
    }

    async fn get_edges_to(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError> {
        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let results: Vec<GraphEdge> = edges
            .iter()
            .filter(|((_, to), e)| {
                *to == node_id && edge_type.as_ref().map_or(true, |t| &e.edge_type == t)
            })
            .map(|(_, e)| e.clone())
            .collect();

        Ok(results)
    }

    async fn delete_edge(&self, from_id: Uuid, to_id: Uuid) -> Result<bool, DomainError> {
        let mut edges = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        Ok(edges.remove(&(from_id, to_id)).is_some())
    }

    async fn get_edges_for_nodes(&self, node_ids: &[Uuid]) -> Result<Vec<GraphEdge>, DomainError> {
        if node_ids.is_empty() {
            return Ok(vec![]);
        }

        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let node_id_set: std::collections::HashSet<Uuid> = node_ids.iter().copied().collect();

        let results: Vec<GraphEdge> = edges
            .iter()
            .filter(|((from, to), _)| node_id_set.contains(from) || node_id_set.contains(to))
            .map(|(_, e)| e.clone())
            .collect();

        Ok(results)
    }

    // ===================
    // Traversal Operations
    // ===================

    async fn get_neighbors(
        &self,
        node_id: Uuid,
        depth: u32,
    ) -> Result<Vec<GraphNode>, DomainError> {
        if depth == 0 {
            return Ok(vec![]);
        }

        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let mut visited: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let mut current_layer: Vec<Uuid> = vec![node_id];
        visited.insert(node_id);

        for _ in 0..depth {
            let mut next_layer = Vec::new();

            for current_id in &current_layer {
                // Find all connected nodes
                for ((from, to), _) in edges.iter() {
                    let neighbor_id = if from == current_id {
                        *to
                    } else if to == current_id {
                        *from
                    } else {
                        continue;
                    };

                    if !visited.contains(&neighbor_id) {
                        visited.insert(neighbor_id);
                        next_layer.push(neighbor_id);
                    }
                }
            }

            if next_layer.is_empty() {
                break;
            }
            current_layer = next_layer;
        }

        // Collect all visited nodes except the starting node
        visited.remove(&node_id);
        let results: Vec<GraphNode> = visited
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();

        Ok(results)
    }

    async fn traverse(
        &self,
        start_id: Uuid,
        query: &TraversalQuery,
    ) -> Result<Vec<GraphPath>, DomainError> {
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let start_node = match nodes.get(&start_id) {
            Some(n) => n.clone(),
            None => return Ok(vec![]),
        };

        let mut paths = Vec::new();
        let mut visited: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        visited.insert(start_id);

        // BFS traversal
        let mut queue: std::collections::VecDeque<(GraphNode, Vec<GraphEdge>, u32)> =
            std::collections::VecDeque::new();
        queue.push_back((start_node.clone(), vec![], 0));

        while let Some((current_node, path_edges, depth)) = queue.pop_front() {
            if depth >= query.max_depth {
                continue;
            }

            // Find outgoing edges
            for ((from, to), edge) in edges.iter() {
                if *from != current_node.id {
                    continue;
                }

                // Apply filters
                if let Some(ref types) = query.edge_types {
                    if !types.contains(&edge.edge_type) {
                        continue;
                    }
                }
                if let Some(min_str) = query.min_strength {
                    if edge.strength < min_str {
                        continue;
                    }
                }

                if visited.contains(to) {
                    continue;
                }
                visited.insert(*to);

                if let Some(next_node) = nodes.get(to) {
                    let mut new_path = path_edges.clone();
                    new_path.push(edge.clone());

                    // Create path
                    let mut path_nodes = vec![start_node.clone()];
                    for e in &new_path {
                        if let Some(n) = nodes.get(&e.to_id) {
                            path_nodes.push(n.clone());
                        }
                    }

                    paths.push(GraphPath {
                        nodes: path_nodes,
                        edges: new_path.clone(),
                        total_weight: new_path.iter().map(|e| e.strength).product(),
                    });

                    queue.push_back((next_node.clone(), new_path, depth + 1));
                }
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            paths.truncate(limit);
        }

        Ok(paths)
    }

    // ===================
    // Similarity Search
    // ===================

    async fn find_by_embedding(
        &self,
        rei_id: Uuid,
        _embedding: &[f32],
        _threshold: f32,
        limit: usize,
    ) -> Result<Vec<(GraphNode, f32)>, DomainError> {
        // Simplified: just return nodes for the rei_id with mock similarity scores
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let results: Vec<(GraphNode, f32)> = nodes
            .values()
            .filter(|n| n.rei_id == rei_id)
            .take(limit)
            .map(|n| (n.clone(), 0.85)) // Mock similarity
            .collect();

        Ok(results)
    }

    // ===================
    // Maintenance
    // ===================

    async fn clear_rei_graph(&self, rei_id: Uuid) -> Result<usize, DomainError> {
        let mut nodes = self.nodes.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let mut edges = self.edges.write().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        // Find nodes to delete
        let node_ids: Vec<Uuid> = nodes
            .values()
            .filter(|n| n.rei_id == rei_id)
            .map(|n| n.id)
            .collect();

        // Remove edges connected to these nodes
        for node_id in &node_ids {
            edges.retain(|(from, to), _| from != node_id && to != node_id);
        }

        // Remove nodes
        let count = node_ids.len();
        for node_id in node_ids {
            nodes.remove(&node_id);
        }

        Ok(count)
    }

    async fn get_stats(&self, rei_id: Uuid) -> Result<GraphStats, DomainError> {
        let nodes = self.nodes.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;
        let edges = self.edges.read().map_err(|e| {
            DomainError::Repository(format!("RwLock poisoned: {}", e))
        })?;

        let rei_nodes: Vec<&GraphNode> = nodes.values().filter(|n| n.rei_id == rei_id).collect();

        let rei_node_ids: std::collections::HashSet<Uuid> =
            rei_nodes.iter().map(|n| n.id).collect();

        let rei_edges: Vec<&GraphEdge> = edges
            .values()
            .filter(|e| rei_node_ids.contains(&e.from_id) || rei_node_ids.contains(&e.to_id))
            .collect();

        let mut nodes_by_type: HashMap<NodeType, usize> = HashMap::new();
        for node in &rei_nodes {
            *nodes_by_type.entry(node.node_type.clone()).or_insert(0) += 1;
        }

        let mut edges_by_type: HashMap<EdgeType, usize> = HashMap::new();
        for edge in &rei_edges {
            *edges_by_type.entry(edge.edge_type.clone()).or_insert(0) += 1;
        }

        Ok(GraphStats {
            total_nodes: rei_nodes.len(),
            total_edges: rei_edges.len(),
            nodes_by_type,
            edges_by_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_node(rei_id: Uuid, text: &str, node_type: NodeType) -> GraphNode {
        GraphNode {
            id: Uuid::new_v4(),
            rei_id,
            text: text.to_string(),
            node_type,
            weight: 1.0,
            embedding: None,
            source_doc_id: None,
            metadata: json!({}),
        }
    }

    fn create_test_edge(from: &GraphNode, to: &GraphNode, edge_type: EdgeType) -> GraphEdge {
        GraphEdge {
            id: Uuid::new_v4(),
            from_id: from.id,
            to_id: to.id,
            edge_type,
            strength: 1.0,
            metadata: json!({}),
        }
    }

    #[tokio::test]
    async fn test_node_crud() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        // Create
        let node = create_test_node(rei_id, "Test Node", NodeType::Concept);
        repo.upsert_node(&node).await.unwrap();

        // Read
        let fetched = repo.get_node(node.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().text, "Test Node");

        // Delete
        let deleted = repo.delete_node(node.id).await.unwrap();
        assert!(deleted);

        let fetched = repo.get_node(node.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_edge_crud() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        let node1 = create_test_node(rei_id, "Node 1", NodeType::Concept);
        let node2 = create_test_node(rei_id, "Node 2", NodeType::Concept);
        repo.upsert_node(&node1).await.unwrap();
        repo.upsert_node(&node2).await.unwrap();

        // Create edge
        let edge = create_test_edge(&node1, &node2, EdgeType::SimilarTo);
        repo.upsert_edge(&edge).await.unwrap();

        // Read edges
        let edges_from = repo.get_edges_from(node1.id, None).await.unwrap();
        assert_eq!(edges_from.len(), 1);
        assert_eq!(edges_from[0].to_id, node2.id);

        let edges_to = repo.get_edges_to(node2.id, None).await.unwrap();
        assert_eq!(edges_to.len(), 1);
        assert_eq!(edges_to[0].from_id, node1.id);

        // Delete edge
        let deleted = repo.delete_edge(node1.id, node2.id).await.unwrap();
        assert!(deleted);

        let edges_from = repo.get_edges_from(node1.id, None).await.unwrap();
        assert!(edges_from.is_empty());
    }

    #[tokio::test]
    async fn test_find_nodes_by_text() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        let node1 = create_test_node(rei_id, "GraphRAG Implementation", NodeType::Concept);
        let node2 = create_test_node(rei_id, "Knowledge Graph", NodeType::Concept);
        let node3 = create_test_node(rei_id, "Vector Database", NodeType::Entity);

        repo.upsert_node(&node1).await.unwrap();
        repo.upsert_node(&node2).await.unwrap();
        repo.upsert_node(&node3).await.unwrap();

        // Search by text
        let results = repo
            .find_nodes_by_text(rei_id, "graph", None, 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        // Search with type filter
        let results = repo
            .find_nodes_by_text(rei_id, "graph", Some(NodeType::Concept), 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_get_neighbors() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        // Create a chain: A -> B -> C
        let node_a = create_test_node(rei_id, "A", NodeType::Concept);
        let node_b = create_test_node(rei_id, "B", NodeType::Concept);
        let node_c = create_test_node(rei_id, "C", NodeType::Concept);

        repo.upsert_node(&node_a).await.unwrap();
        repo.upsert_node(&node_b).await.unwrap();
        repo.upsert_node(&node_c).await.unwrap();

        repo.upsert_edge(&create_test_edge(&node_a, &node_b, EdgeType::SimilarTo))
            .await
            .unwrap();
        repo.upsert_edge(&create_test_edge(&node_b, &node_c, EdgeType::SimilarTo))
            .await
            .unwrap();

        // Depth 1 from A should find B
        let neighbors = repo.get_neighbors(node_a.id, 1).await.unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].text, "B");

        // Depth 2 from A should find B and C
        let neighbors = repo.get_neighbors(node_a.id, 2).await.unwrap();
        assert_eq!(neighbors.len(), 2);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        let nodes: Vec<GraphNode> = (0..5)
            .map(|i| create_test_node(rei_id, &format!("Node {}", i), NodeType::Concept))
            .collect();

        let result = repo.upsert_nodes(&nodes).await.unwrap();
        assert_eq!(result.created, 5);
        assert_eq!(result.updated, 0);

        // Upsert again
        let result = repo.upsert_nodes(&nodes).await.unwrap();
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 5);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let repo = InMemoryGraphRepository::new();
        let rei_id = Uuid::new_v4();

        let node1 = create_test_node(rei_id, "Concept 1", NodeType::Concept);
        let node2 = create_test_node(rei_id, "Entity 1", NodeType::Entity);

        repo.upsert_node(&node1).await.unwrap();
        repo.upsert_node(&node2).await.unwrap();
        repo.upsert_edge(&create_test_edge(&node1, &node2, EdgeType::SimilarTo))
            .await
            .unwrap();

        let stats = repo.get_stats(rei_id).await.unwrap();
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.total_edges, 1);
        assert_eq!(stats.nodes_by_type.get(&NodeType::Concept), Some(&1));
        assert_eq!(stats.nodes_by_type.get(&NodeType::Entity), Some(&1));
    }
}
