//! Memory Routes - Long-term memory storage in MemoryKai (Qdrant)

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::models::{CreateMemoryRequest, Memory, MemoryResponse, SearchMemoriesRequest};
use crate::AppState;

/// Add a memory to MemoryKai
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

    let memory = Memory {
        id: Uuid::new_v4().to_string(),
        rei_id: rei_id.to_string(),
        content: payload.content.clone(),
        memory_type: payload.memory_type,
        importance: payload.importance.unwrap_or(0.5),
        tags: payload.tags,
        metadata: payload.metadata,
        created_at: Utc::now(),
    };

    // Generate embedding using OpenAI API
    let embedding = embedding_service
        .embed(&payload.content)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    memory_kai
        .add_memory(&rei_id.to_string(), memory.clone(), embedding)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(memory.into()))
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

    let limit = payload.limit.unwrap_or(10);

    let memories = memory_kai
        .search_memories(&rei_id.to_string(), query_vector, limit)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(memories.into_iter().map(MemoryResponse::from).collect()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/memories", post(add_memory))
        .route("/kaiba/rei/:rei_id/memories/search", post(search_memories))
}
