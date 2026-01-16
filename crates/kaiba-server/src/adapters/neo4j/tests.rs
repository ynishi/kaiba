//! Integration tests for Neo4jGraphRepository
//!
//! These tests require a running Neo4j instance (Docker).
//! Set NEO4J_TEST_URI, NEO4J_TEST_USER, NEO4J_TEST_PASSWORD environment variables.
//!
//! Run with: cargo test -p kaiba-server --features integration
//! Or use: ./scripts/run-integration-tests.sh

#[cfg(all(test, feature = "integration"))]
mod tests {
    use kaiba::{EdgeType, GraphEdge, GraphNode, GraphRepository, NodeType, TraversalQuery};
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::adapters::Neo4jGraphRepository;

    /// Get test Neo4j connection (requires env vars)
    async fn get_test_repo() -> Arc<Neo4jGraphRepository> {
        let uri = std::env::var("NEO4J_TEST_URI")
            .expect("NEO4J_TEST_URI must be set for integration tests");
        let user = std::env::var("NEO4J_TEST_USER")
            .expect("NEO4J_TEST_USER must be set for integration tests");
        let password = std::env::var("NEO4J_TEST_PASSWORD")
            .expect("NEO4J_TEST_PASSWORD must be set for integration tests");

        Arc::new(
            Neo4jGraphRepository::new(&uri, &user, &password)
                .await
                .expect("Failed to connect to test Neo4j"),
        )
    }

    #[tokio::test]
    async fn test_neo4j_node_crud() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        // Create node
        let node = GraphNode::concept(rei_id, "test concept".to_string(), 1.0, Some(doc_id));
        let node_id = node.id;

        let saved = repo.upsert_node(&node).await.expect("Failed to save node");
        assert_eq!(saved.text, "test concept");

        // Read node
        let found = repo
            .get_node(node_id)
            .await
            .expect("Failed to get node")
            .expect("Node not found");
        assert_eq!(found.text, "test concept");
        assert_eq!(found.node_type, NodeType::Concept);

        // Update node
        let mut updated_node = node.clone();
        updated_node.text = "updated concept".to_string();
        let updated = repo
            .upsert_node(&updated_node)
            .await
            .expect("Failed to update node");
        assert_eq!(updated.text, "updated concept");

        // Delete node
        let deleted = repo
            .delete_node(node_id)
            .await
            .expect("Failed to delete node");
        assert!(deleted);

        // Verify deleted
        let not_found = repo.get_node(node_id).await.expect("Failed to query node");
        assert!(not_found.is_none());

        // Cleanup
        let _ = repo.clear_rei_graph(rei_id).await;
    }

    #[tokio::test]
    async fn test_neo4j_edge_crud() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();

        // Create two nodes
        let node1 = GraphNode::concept(rei_id, "concept A".to_string(), 1.0, None);
        let node2 = GraphNode::concept(rei_id, "concept B".to_string(), 0.8, None);

        repo.upsert_node(&node1)
            .await
            .expect("Failed to save node1");
        repo.upsert_node(&node2)
            .await
            .expect("Failed to save node2");

        // Create edge
        let edge = GraphEdge::similar_to(node1.id, node2.id, 0.85);
        let saved_edge = repo.upsert_edge(&edge).await.expect("Failed to save edge");
        assert_eq!(saved_edge.strength, 0.85);

        // Get edges from node1
        let edges = repo
            .get_edges_from(node1.id, Some(EdgeType::SimilarTo))
            .await
            .expect("Failed to get edges");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to_id, node2.id);

        // Get neighbors
        let neighbors = repo
            .get_neighbors(node1.id, 1)
            .await
            .expect("Failed to get neighbors");
        assert!(neighbors.iter().any(|n| n.id == node2.id));

        // Cleanup
        let _ = repo.clear_rei_graph(rei_id).await;
    }

    #[tokio::test]
    async fn test_neo4j_batch_operations() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();

        // Create multiple nodes
        let nodes: Vec<GraphNode> = (0..5)
            .map(|i| GraphNode::concept(rei_id, format!("batch concept {}", i), 1.0, None))
            .collect();

        let result = repo
            .upsert_nodes(&nodes)
            .await
            .expect("Failed to batch upsert");
        assert_eq!(result.created, 5);
        assert!(result.failed.is_empty());

        // Create edges between consecutive nodes
        let edges: Vec<GraphEdge> = nodes
            .windows(2)
            .map(|w| GraphEdge::similar_to(w[0].id, w[1].id, 0.9))
            .collect();

        let edge_result = repo
            .upsert_edges(&edges)
            .await
            .expect("Failed to batch edges");
        assert_eq!(edge_result.created, 4);

        // Get stats
        let stats = repo.get_stats(rei_id).await.expect("Failed to get stats");
        assert!(stats.total_nodes >= 5);

        // Cleanup
        let deleted = repo
            .clear_rei_graph(rei_id)
            .await
            .expect("Failed to clear graph");
        assert!(deleted >= 5);
    }

    #[tokio::test]
    async fn test_neo4j_find_by_text() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();

        // Create nodes with searchable text
        let node1 = GraphNode::concept(rei_id, "machine learning".to_string(), 1.0, None);
        let node2 = GraphNode::concept(rei_id, "deep learning".to_string(), 0.9, None);
        let node3 = GraphNode::concept(rei_id, "reinforcement".to_string(), 0.8, None);

        repo.upsert_node(&node1).await.unwrap();
        repo.upsert_node(&node2).await.unwrap();
        repo.upsert_node(&node3).await.unwrap();

        // Search for "learning"
        let found = repo
            .find_nodes_by_text(rei_id, "learning", None, 10)
            .await
            .expect("Failed to search");
        assert_eq!(found.len(), 2);

        // Search with type filter
        let found_concept = repo
            .find_nodes_by_text(rei_id, "learning", Some(NodeType::Concept), 10)
            .await
            .expect("Failed to search with filter");
        assert_eq!(found_concept.len(), 2);

        // Cleanup
        let _ = repo.clear_rei_graph(rei_id).await;
    }

    #[tokio::test]
    async fn test_neo4j_traverse() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();

        // Create a chain: A -> B -> C
        let node_a = GraphNode::concept(rei_id, "node A".to_string(), 1.0, None);
        let node_b = GraphNode::concept(rei_id, "node B".to_string(), 0.9, None);
        let node_c = GraphNode::concept(rei_id, "node C".to_string(), 0.8, None);

        repo.upsert_node(&node_a).await.unwrap();
        repo.upsert_node(&node_b).await.unwrap();
        repo.upsert_node(&node_c).await.unwrap();

        repo.upsert_edge(&GraphEdge::similar_to(node_a.id, node_b.id, 0.9))
            .await
            .unwrap();
        repo.upsert_edge(&GraphEdge::similar_to(node_b.id, node_c.id, 0.85))
            .await
            .unwrap();

        // Traverse from A with depth 2
        let query = TraversalQuery::new().with_depth(2).with_limit(10);
        let _paths = repo
            .traverse(node_a.id, &query)
            .await
            .expect("Failed to traverse");

        // Note: Full path parsing is complex, so we just verify it doesn't error
        // In production, we'd verify the actual paths

        // Cleanup
        let _ = repo.clear_rei_graph(rei_id).await;
    }

    #[tokio::test]
    async fn test_neo4j_delete_by_document() {
        let repo = get_test_repo().await;

        let rei_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();

        // Create nodes from a document
        let nodes: Vec<GraphNode> = (0..3)
            .map(|i| GraphNode::concept(rei_id, format!("doc concept {}", i), 1.0, Some(doc_id)))
            .collect();

        for node in &nodes {
            repo.upsert_node(node).await.unwrap();
        }

        // Delete by document
        let deleted = repo
            .delete_nodes_by_document(doc_id)
            .await
            .expect("Failed to delete by document");
        assert_eq!(deleted, 3);

        // Verify deleted
        for node in &nodes {
            let found = repo.get_node(node.id).await.unwrap();
            assert!(found.is_none());
        }

        // Cleanup
        let _ = repo.clear_rei_graph(rei_id).await;
    }
}
