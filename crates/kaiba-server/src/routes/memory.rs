//! Memory Routes - Long-term memory storage
//!
//! Saves to: MemoryKai (Qdrant) + GraphKai (Neo4j)

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use kaiba::{EmphasisParser, GraphBuilder, GraphRepository};

use crate::models::{CreateMemoryRequest, Memory, MemoryResponse, SearchMemoriesRequest};
use crate::services::{HybridSearchConfig, SearchFilter, StrategySet};
use crate::AppState;

/// Add a memory to MemoryKai (Qdrant) + GraphKai (Neo4j)
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/memories",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = CreateMemoryRequest,
    responses(
        (status = 200, description = "Memory added", body = MemoryResponse),
        (status = 503, description = "MemoryKai or Embedding service unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Memory"
)]
pub async fn add_memory(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<CreateMemoryRequest>,
) -> Result<Json<MemoryResponse>, (axum::http::StatusCode, String)> {
    let memory_kai = state.memory_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "MemoryKai not available".to_string(),
    ))?;

    let embedding_service = state.embedding.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "Embedding service not available".to_string(),
    ))?;

    let memory_id = Uuid::new_v4();

    // Merge source info into metadata
    let mut metadata = payload.metadata.unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert("source".to_string(), serde_json::json!("memory"));
        obj.insert(
            "memory_id".to_string(),
            serde_json::json!(memory_id.to_string()),
        );
    }

    let memory = Memory {
        id: memory_id.to_string(),
        rei_id: rei_id.to_string(),
        content: payload.content.clone(),
        memory_type: payload.memory_type,
        importance: payload.importance.unwrap_or(0.5),
        tags: payload.tags,
        topic_path: None,
        metadata: Some(metadata),
        created_at: Utc::now(),
    };

    // 1. Generate embedding using OpenAI API
    let embedding = embedding_service
        .embed(&payload.content)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Save to MemoryKai (Qdrant) - RAG
    memory_kai
        .add_memory(&rei_id.to_string(), memory.clone(), embedding)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. Save to GraphKai (Neo4j) - Knowledge Graph
    if let Some(graph_kai) = &state.graph_kai {
        if let Err(e) =
            save_memory_to_graph(graph_kai.as_ref(), rei_id, memory_id, &memory.content).await
        {
            // Log warning but don't fail the request - RAG save succeeded
            tracing::warn!("Failed to save memory {} to Graph: {}", memory_id, e);
        }
    }

    Ok(Json(memory.into()))
}

/// Save memory content to GraphKai (Neo4j) - builds knowledge graph from emphasis
async fn save_memory_to_graph(
    graph_kai: &dyn GraphRepository,
    rei_id: Uuid,
    memory_id: Uuid,
    content: &str,
) -> Result<usize, String> {
    let emphasis_parser = EmphasisParser::new();
    let graph_builder = GraphBuilder::new(Default::default());

    // Parse emphasis from memory content
    let parse_result = emphasis_parser.parse(memory_id, content);

    if parse_result.nodes.is_empty() {
        return Ok(0);
    }

    // Build graph nodes and edges (using memory_id as doc_id)
    let title = format!("Memory:{}", &memory_id.to_string()[..8]);
    let build_result =
        graph_builder.build_from_emphasis(rei_id, memory_id, &title, &parse_result.nodes);

    let mut nodes_created = 0;

    // Upsert document node (represents the memory)
    if let Some(doc_node) = &build_result.doc_node {
        graph_kai
            .upsert_node(doc_node)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Upsert concept nodes
    if !build_result.nodes.is_empty() {
        let result = graph_kai
            .upsert_nodes(&build_result.nodes)
            .await
            .map_err(|e| e.to_string())?;
        nodes_created = result.created + result.updated;
    }

    // Upsert edges
    if !build_result.extraction_edges.is_empty() {
        graph_kai
            .upsert_edges(&build_result.extraction_edges)
            .await
            .map_err(|e| e.to_string())?;
    }

    if !build_result.co_occurrence_edges.is_empty() {
        graph_kai
            .upsert_edges(&build_result.co_occurrence_edges)
            .await
            .map_err(|e| e.to_string())?;
    }

    tracing::debug!(
        "Memory {} saved to graph: {} nodes created",
        memory_id,
        nodes_created
    );

    Ok(nodes_created)
}

/// Search memories in MemoryKai
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/memories/search",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = SearchMemoriesRequest,
    responses(
        (status = 200, description = "Matching memories", body = Vec<MemoryResponse>),
        (status = 503, description = "MemoryKai or Embedding service unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Memory"
)]
pub async fn search_memories(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<SearchMemoriesRequest>,
) -> Result<Json<Vec<MemoryResponse>>, (axum::http::StatusCode, String)> {
    let limit = payload.limit.unwrap_or(10);

    // Use HybridSearch if available
    if let Some(hybrid_search) = &state.hybrid_search {
        let config = HybridSearchConfig {
            strategy: payload.strategy.unwrap_or_default(),
            rag_limit: limit,
            context: payload.context.clone(),
            ..Default::default()
        };

        // Multi-strategy mode: run multiple strategies and merge results
        let result = if !payload.strategies.is_empty() {
            let strategy_set = StrategySet::multiple(payload.strategies.iter().copied());
            hybrid_search
                .search_with_strategies(&rei_id, &payload.query, &strategy_set, config)
                .await
        } else {
            hybrid_search.search(&rei_id, &payload.query, config).await
        }
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let responses: Vec<MemoryResponse> = result
            .memories
            .into_iter()
            .map(|scored| {
                let mut resp = MemoryResponse::from(scored.memory);
                resp.similarity = Some(scored.score);
                resp
            })
            .collect();

        return Ok(Json(responses));
    }

    // Fallback to direct MemoryKai search
    let memory_kai = state.memory_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "MemoryKai not available".to_string(),
    ))?;

    let embedding_service = state.embedding.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "Embedding service not available".to_string(),
    ))?;

    // Generate query embedding using OpenAI API
    let query_vector = embedding_service
        .embed(&payload.query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Build search filter
    let filter = SearchFilter {
        memory_type: payload.memory_type,
        tags: payload.tags,
        tags_match_mode: payload.tags_match_mode,
        min_importance: payload.min_importance,
        ..Default::default()
    };

    let memories = memory_kai
        .search_memories_with_filter(&rei_id.to_string(), query_vector, limit, filter)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        memories.into_iter().map(MemoryResponse::from).collect(),
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/memories", post(add_memory))
        .route("/kaiba/rei/:rei_id/memories/search", post(search_memories))
}
