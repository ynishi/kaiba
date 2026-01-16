//! Neo4jGraphRepository - Neo4j implementation of GraphRepository
//!
//! Provides knowledge graph storage using Neo4j Aura Free tier.

use async_trait::async_trait;
use neo4rs::{query, Graph, Node, Relation, Row};
use std::sync::Arc;
use uuid::Uuid;

use kaiba::{
    DomainError, EdgeBatchResult, EdgeType, GraphEdge, GraphNode, GraphPath, GraphRepository,
    GraphStats, NodeBatchResult, NodeType, TraversalQuery,
};

/// Neo4j-based graph repository
pub struct Neo4jGraphRepository {
    graph: Arc<Graph>,
}

impl Neo4jGraphRepository {
    /// Create a new Neo4j graph repository
    pub async fn new(uri: &str, user: &str, password: &str) -> Result<Self, DomainError> {
        let graph = Graph::new(uri, user, password)
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j connection failed: {}", e)))?;

        Ok(Self {
            graph: Arc::new(graph),
        })
    }

    /// Create from existing Graph instance
    pub fn from_graph(graph: Arc<Graph>) -> Self {
        Self { graph }
    }

    /// Convert NodeType to Neo4j label
    fn node_type_to_label(node_type: NodeType) -> &'static str {
        match node_type {
            NodeType::Concept => "Concept",
            NodeType::Entity => "Entity",
            NodeType::Tag => "Tag",
            NodeType::Document => "Document",
        }
    }

    /// Convert Neo4j label to NodeType
    fn label_to_node_type(label: &str) -> NodeType {
        match label {
            "Concept" => NodeType::Concept,
            "Entity" => NodeType::Entity,
            "Tag" => NodeType::Tag,
            "Document" => NodeType::Document,
            _ => NodeType::Concept,
        }
    }

    /// Convert EdgeType to Neo4j relationship type
    fn edge_type_to_rel(edge_type: EdgeType) -> &'static str {
        match edge_type {
            EdgeType::SimilarTo => "SIMILAR_TO",
            EdgeType::CoOccursWith => "CO_OCCURS_WITH",
            EdgeType::BelongsTo => "BELONGS_TO",
            EdgeType::ExtractedFrom => "EXTRACTED_FROM",
        }
    }

    /// Convert Neo4j relationship type to EdgeType
    fn rel_to_edge_type(rel_type: &str) -> EdgeType {
        match rel_type {
            "SIMILAR_TO" => EdgeType::SimilarTo,
            "CO_OCCURS_WITH" => EdgeType::CoOccursWith,
            "BELONGS_TO" => EdgeType::BelongsTo,
            "EXTRACTED_FROM" => EdgeType::ExtractedFrom,
            _ => EdgeType::SimilarTo,
        }
    }

    /// Convert Row to GraphNode
    fn row_to_node(row: &Row, key: &str) -> Result<GraphNode, DomainError> {
        let node: Node = row
            .get(key)
            .map_err(|e| DomainError::Repository(format!("Failed to get node: {}", e)))?;

        let id_str: String = node
            .get("id")
            .map_err(|_| DomainError::Repository("Missing node id".to_string()))?;
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| DomainError::Repository(format!("Invalid UUID: {}", e)))?;

        let rei_id_str: String = node
            .get("rei_id")
            .map_err(|_| DomainError::Repository("Missing rei_id".to_string()))?;
        let rei_id = Uuid::parse_str(&rei_id_str)
            .map_err(|e| DomainError::Repository(format!("Invalid rei_id UUID: {}", e)))?;

        let text: String = node.get("text").unwrap_or_default();
        let weight: f64 = node.get("weight").unwrap_or(1.0);

        let source_doc_id: Option<Uuid> = node
            .get::<String>("source_doc_id")
            .ok()
            .and_then(|s: String| Uuid::parse_str(&s).ok());

        let metadata_str: String = node.get("metadata").unwrap_or_else(|_| "{}".to_string());
        let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
            .unwrap_or(serde_json::Value::Object(Default::default()));

        // Get node label for type
        let labels = node.labels();
        let node_type = labels
            .first()
            .map(|l| Self::label_to_node_type(l))
            .unwrap_or(NodeType::Concept);

        // Embedding is stored as JSON array string
        let embedding: Option<Vec<f32>> = node
            .get::<String>("embedding")
            .ok()
            .and_then(|s: String| serde_json::from_str(&s).ok());

        Ok(GraphNode {
            id,
            rei_id,
            text,
            node_type,
            weight: weight as f32,
            embedding,
            source_doc_id,
            metadata,
        })
    }
}

#[async_trait]
impl GraphRepository for Neo4jGraphRepository {
    async fn upsert_node(&self, node: &GraphNode) -> Result<GraphNode, DomainError> {
        let label = Self::node_type_to_label(node.node_type);
        let embedding_json = node
            .embedding
            .as_ref()
            .map(|e| serde_json::to_string(e).unwrap_or_else(|_| "[]".to_string()));
        let metadata_json =
            serde_json::to_string(&node.metadata).unwrap_or_else(|_| "{}".to_string());
        let source_doc_id_str = node.source_doc_id.map(|id| id.to_string());

        let cypher = format!(
            r#"
            MERGE (n:{} {{id: $id}})
            SET n.rei_id = $rei_id,
                n.text = $text,
                n.weight = $weight,
                n.embedding = $embedding,
                n.source_doc_id = $source_doc_id,
                n.metadata = $metadata,
                n.updated_at = datetime()
            ON CREATE SET n.created_at = datetime()
            RETURN n
            "#,
            label
        );

        let mut result = self
            .graph
            .execute(
                query(&cypher)
                    .param("id", node.id.to_string())
                    .param("rei_id", node.rei_id.to_string())
                    .param("text", node.text.clone())
                    .param("weight", node.weight as f64)
                    .param("embedding", embedding_json)
                    .param("source_doc_id", source_doc_id_str)
                    .param("metadata", metadata_json),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j upsert failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            Self::row_to_node(&row, "n")
        } else {
            Ok(node.clone())
        }
    }

    async fn upsert_nodes(&self, nodes: &[GraphNode]) -> Result<NodeBatchResult, DomainError> {
        let mut created = 0;
        let updated = 0; // We can't easily distinguish create vs update
        let mut failed = Vec::new();

        for node in nodes {
            match self.upsert_node(node).await {
                Ok(_) => {
                    created += 1;
                }
                Err(_) => {
                    failed.push(node.id);
                }
            }
        }

        Ok(NodeBatchResult {
            created,
            updated,
            failed,
        })
    }

    async fn get_node(&self, id: Uuid) -> Result<Option<GraphNode>, DomainError> {
        let cypher = r#"
            MATCH (n)
            WHERE n.id = $id
            RETURN n
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            Ok(Some(Self::row_to_node(&row, "n")?))
        } else {
            Ok(None)
        }
    }

    async fn find_nodes_by_text(
        &self,
        rei_id: Uuid,
        text: &str,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError> {
        let label_filter = node_type
            .map(|t| format!(":{}", Self::node_type_to_label(t)))
            .unwrap_or_default();

        let cypher = format!(
            r#"
            MATCH (n{})
            WHERE n.rei_id = $rei_id AND toLower(n.text) CONTAINS toLower($text)
            RETURN n
            LIMIT $limit
            "#,
            label_filter
        );

        let mut result = self
            .graph
            .execute(
                query(&cypher)
                    .param("rei_id", rei_id.to_string())
                    .param("text", text.to_string())
                    .param("limit", limit as i64),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut nodes = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            nodes.push(Self::row_to_node(&row, "n")?);
        }

        Ok(nodes)
    }

    async fn find_nodes_by_type(
        &self,
        rei_id: Uuid,
        node_type: NodeType,
        limit: usize,
    ) -> Result<Vec<GraphNode>, DomainError> {
        let label = Self::node_type_to_label(node_type);

        let cypher = format!(
            r#"
            MATCH (n:{})
            WHERE n.rei_id = $rei_id
            RETURN n
            ORDER BY n.weight DESC
            LIMIT $limit
            "#,
            label
        );

        let mut result = self
            .graph
            .execute(
                query(&cypher)
                    .param("rei_id", rei_id.to_string())
                    .param("limit", limit as i64),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut nodes = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            nodes.push(Self::row_to_node(&row, "n")?);
        }

        Ok(nodes)
    }

    async fn delete_node(&self, id: Uuid) -> Result<bool, DomainError> {
        let cypher = r#"
            MATCH (n {id: $id})
            DETACH DELETE n
            RETURN count(n) as deleted
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j delete failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let deleted: i64 = row.get("deleted").unwrap_or(0);
            Ok(deleted > 0)
        } else {
            Ok(false)
        }
    }

    async fn delete_nodes_by_document(&self, doc_id: Uuid) -> Result<usize, DomainError> {
        let cypher = r#"
            MATCH (n {source_doc_id: $doc_id})
            DETACH DELETE n
            RETURN count(n) as deleted
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("doc_id", doc_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j delete failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let deleted: i64 = row.get("deleted").unwrap_or(0);
            Ok(deleted as usize)
        } else {
            Ok(0)
        }
    }

    async fn upsert_edge(&self, edge: &GraphEdge) -> Result<GraphEdge, DomainError> {
        let rel_type = Self::edge_type_to_rel(edge.edge_type);
        let metadata_json =
            serde_json::to_string(&edge.metadata).unwrap_or_else(|_| "{}".to_string());

        let cypher = format!(
            r#"
            MATCH (from {{id: $from_id}})
            MATCH (to {{id: $to_id}})
            MERGE (from)-[r:{}]->(to)
            SET r.id = $id,
                r.strength = $strength,
                r.metadata = $metadata,
                r.updated_at = datetime()
            ON CREATE SET r.created_at = datetime()
            RETURN r
            "#,
            rel_type
        );

        let _ = self
            .graph
            .execute(
                query(&cypher)
                    .param("id", edge.id.to_string())
                    .param("from_id", edge.from_id.to_string())
                    .param("to_id", edge.to_id.to_string())
                    .param("strength", edge.strength as f64)
                    .param("metadata", metadata_json),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j edge upsert failed: {}", e)))?;

        Ok(edge.clone())
    }

    async fn upsert_edges(&self, edges: &[GraphEdge]) -> Result<EdgeBatchResult, DomainError> {
        let mut created = 0;
        let existing = 0; // We don't track existing edges in this implementation
        let mut failed = Vec::new();

        for edge in edges {
            match self.upsert_edge(edge).await {
                Ok(_) => created += 1,
                Err(e) => failed.push(e.to_string()),
            }
        }

        Ok(EdgeBatchResult {
            created,
            existing,
            failed,
        })
    }

    async fn get_edges_from(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError> {
        let rel_filter = edge_type
            .map(|t| format!(":{}", Self::edge_type_to_rel(t)))
            .unwrap_or_default();

        let cypher = format!(
            r#"
            MATCH (from {{id: $node_id}})-[r{}]->(to)
            RETURN r, from.id as from_id, to.id as to_id, type(r) as rel_type
            "#,
            rel_filter
        );

        let mut result = self
            .graph
            .execute(query(&cypher).param("node_id", node_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut edges = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let rel: Relation = row
                .get("r")
                .map_err(|e| DomainError::Repository(format!("Failed to get relation: {}", e)))?;

            let id_str: String = rel.get("id").unwrap_or_else(|_| Uuid::new_v4().to_string());
            let id = Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4());

            let from_id_str: String = row.get("from_id").unwrap_or_default();
            let from_id = Uuid::parse_str(&from_id_str).unwrap_or_else(|_| Uuid::nil());

            let to_id_str: String = row.get("to_id").unwrap_or_default();
            let to_id = Uuid::parse_str(&to_id_str).unwrap_or_else(|_| Uuid::nil());

            let rel_type: String = row.get("rel_type").unwrap_or_default();
            let edge_type = Self::rel_to_edge_type(&rel_type);

            let strength: f64 = rel.get("strength").unwrap_or(1.0);

            let metadata_str: String = rel.get("metadata").unwrap_or_else(|_| "{}".to_string());
            let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));

            edges.push(GraphEdge {
                id,
                from_id,
                to_id,
                edge_type,
                strength: strength as f32,
                metadata,
            });
        }

        Ok(edges)
    }

    async fn get_edges_to(
        &self,
        node_id: Uuid,
        edge_type: Option<EdgeType>,
    ) -> Result<Vec<GraphEdge>, DomainError> {
        let rel_filter = edge_type
            .map(|t| format!(":{}", Self::edge_type_to_rel(t)))
            .unwrap_or_default();

        let cypher = format!(
            r#"
            MATCH (from)-[r{}]->(to {{id: $node_id}})
            RETURN r, from.id as from_id, to.id as to_id, type(r) as rel_type
            "#,
            rel_filter
        );

        let mut result = self
            .graph
            .execute(query(&cypher).param("node_id", node_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut edges = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let rel: Relation = row
                .get("r")
                .map_err(|e| DomainError::Repository(format!("Failed to get relation: {}", e)))?;

            let id_str: String = rel.get("id").unwrap_or_else(|_| Uuid::new_v4().to_string());
            let id = Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4());

            let from_id_str: String = row.get("from_id").unwrap_or_default();
            let from_id = Uuid::parse_str(&from_id_str).unwrap_or_else(|_| Uuid::nil());

            let to_id_str: String = row.get("to_id").unwrap_or_default();
            let to_id = Uuid::parse_str(&to_id_str).unwrap_or_else(|_| Uuid::nil());

            let rel_type: String = row.get("rel_type").unwrap_or_default();
            let edge_type = Self::rel_to_edge_type(&rel_type);

            let strength: f64 = rel.get("strength").unwrap_or(1.0);

            let metadata_str: String = rel.get("metadata").unwrap_or_else(|_| "{}".to_string());
            let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));

            edges.push(GraphEdge {
                id,
                from_id,
                to_id,
                edge_type,
                strength: strength as f32,
                metadata,
            });
        }

        Ok(edges)
    }

    async fn delete_edge(&self, from_id: Uuid, to_id: Uuid) -> Result<bool, DomainError> {
        let cypher = r#"
            MATCH (from {id: $from_id})-[r]->(to {id: $to_id})
            DELETE r
            RETURN count(r) as deleted
        "#;

        let mut result = self
            .graph
            .execute(
                query(cypher)
                    .param("from_id", from_id.to_string())
                    .param("to_id", to_id.to_string()),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j delete failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let deleted: i64 = row.get("deleted").unwrap_or(0);
            Ok(deleted > 0)
        } else {
            Ok(false)
        }
    }

    async fn get_edges_for_nodes(&self, node_ids: &[Uuid]) -> Result<Vec<GraphEdge>, DomainError> {
        if node_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids_str: Vec<String> = node_ids.iter().map(|id| id.to_string()).collect();

        let cypher = r#"
            MATCH (from)-[r]->(to)
            WHERE from.id IN $node_ids OR to.id IN $node_ids
            RETURN r, from.id as from_id, to.id as to_id, type(r) as rel_type
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("node_ids", ids_str))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut edges = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let rel: Relation = row
                .get("r")
                .map_err(|e| DomainError::Repository(format!("Failed to get relation: {}", e)))?;

            let id_str: String = rel.get("id").unwrap_or_else(|_| Uuid::new_v4().to_string());
            let id = Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4());

            let from_id_str: String = row.get("from_id").unwrap_or_default();
            let from_id = Uuid::parse_str(&from_id_str).unwrap_or_else(|_| Uuid::nil());

            let to_id_str: String = row.get("to_id").unwrap_or_default();
            let to_id = Uuid::parse_str(&to_id_str).unwrap_or_else(|_| Uuid::nil());

            let rel_type: String = row.get("rel_type").unwrap_or_default();
            let edge_type = Self::rel_to_edge_type(&rel_type);

            let strength: f64 = rel.get("strength").unwrap_or(1.0);

            let metadata_str: String = rel.get("metadata").unwrap_or_else(|_| "{}".to_string());
            let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));

            edges.push(GraphEdge {
                id,
                from_id,
                to_id,
                edge_type,
                strength: strength as f32,
                metadata,
            });
        }

        Ok(edges)
    }

    async fn get_neighbors(
        &self,
        node_id: Uuid,
        depth: u32,
    ) -> Result<Vec<GraphNode>, DomainError> {
        let cypher = format!(
            r#"
            MATCH (start {{id: $node_id}})-[*1..{}]-(neighbor)
            RETURN DISTINCT neighbor as n
            "#,
            depth
        );

        let mut result = self
            .graph
            .execute(query(&cypher).param("node_id", node_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut nodes = Vec::new();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            nodes.push(Self::row_to_node(&row, "n")?);
        }

        Ok(nodes)
    }

    async fn traverse(
        &self,
        start_id: Uuid,
        query_params: &TraversalQuery,
    ) -> Result<Vec<GraphPath>, DomainError> {
        let depth = query_params.max_depth.max(1);
        let limit = query_params.limit.unwrap_or(100);

        let rel_filter = query_params
            .edge_types
            .as_ref()
            .map(|types| {
                let rels: Vec<String> = types
                    .iter()
                    .map(|t| Self::edge_type_to_rel(*t).to_string())
                    .collect();
                format!(":{}", rels.join("|"))
            })
            .unwrap_or_default();

        let strength_filter = query_params
            .min_strength
            .map(|s| {
                format!(
                    "WHERE ALL(r IN relationships(path) WHERE r.strength >= {})",
                    s
                )
            })
            .unwrap_or_default();

        let cypher = format!(
            r#"
            MATCH path = (start {{id: $start_id}})-[{}*1..{}]->(end)
            {}
            RETURN path
            LIMIT $limit
            "#,
            rel_filter, depth, strength_filter
        );

        let mut result = self
            .graph
            .execute(
                query(&cypher)
                    .param("start_id", start_id.to_string())
                    .param("limit", limit as i64),
            )
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        // For simplicity, return empty vec - full path parsing is complex
        // In production, we'd parse the path properly
        let paths = Vec::new();
        while result.next().await.is_ok() {
            // Path parsing would go here
        }

        Ok(paths)
    }

    async fn find_by_embedding(
        &self,
        rei_id: Uuid,
        _embedding: &[f32],
        _threshold: f32,
        limit: usize,
    ) -> Result<Vec<(GraphNode, f32)>, DomainError> {
        // Neo4j Aura Free doesn't have vector index support
        // For now, return top nodes by weight
        // In production, use Neo4j Vector Index or external vector search
        let nodes = self
            .find_nodes_by_type(rei_id, NodeType::Concept, limit)
            .await?;
        Ok(nodes.into_iter().map(|n| (n, 1.0)).collect())
    }

    async fn clear_rei_graph(&self, rei_id: Uuid) -> Result<usize, DomainError> {
        let cypher = r#"
            MATCH (n {rei_id: $rei_id})
            DETACH DELETE n
            RETURN count(n) as deleted
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("rei_id", rei_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j delete failed: {}", e)))?;

        if let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let deleted: i64 = row.get("deleted").unwrap_or(0);
            Ok(deleted as usize)
        } else {
            Ok(0)
        }
    }

    async fn get_stats(&self, rei_id: Uuid) -> Result<GraphStats, DomainError> {
        let cypher = r#"
            MATCH (n {rei_id: $rei_id})
            WITH labels(n) as nodeLabels, count(n) as nodeCount
            RETURN nodeLabels, nodeCount
        "#;

        let mut result = self
            .graph
            .execute(query(cypher).param("rei_id", rei_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        let mut stats = GraphStats::default();
        while let Some(row) = result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let labels: Vec<String> = row.get("nodeLabels").unwrap_or_default();
            let count: i64 = row.get("nodeCount").unwrap_or(0);

            if let Some(label) = labels.first() {
                let node_type = Self::label_to_node_type(label);
                stats.nodes_by_type.insert(node_type, count as usize);
                stats.total_nodes += count as usize;
            }
        }

        // Get edge counts
        let edge_cypher = r#"
            MATCH (n {rei_id: $rei_id})-[r]->()
            RETURN type(r) as relType, count(r) as relCount
        "#;

        let mut edge_result = self
            .graph
            .execute(query(edge_cypher).param("rei_id", rei_id.to_string()))
            .await
            .map_err(|e| DomainError::Repository(format!("Neo4j query failed: {}", e)))?;

        while let Some(row) = edge_result
            .next()
            .await
            .map_err(|e| DomainError::Repository(format!("Failed to get result: {}", e)))?
        {
            let rel_type: String = row.get("relType").unwrap_or_default();
            let count: i64 = row.get("relCount").unwrap_or(0);

            let edge_type = Self::rel_to_edge_type(&rel_type);
            stats.edges_by_type.insert(edge_type, count as usize);
            stats.total_edges += count as usize;
        }

        Ok(stats)
    }
}
