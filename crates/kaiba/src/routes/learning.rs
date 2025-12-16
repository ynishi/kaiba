//! Learning Routes - Self-learning API endpoints
//!
//! POST /kaiba/rei/:rei_id/learn - Trigger learning for a specific Rei
//! POST /kaiba/learn/all - Trigger learning for all Reis

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::self_learning::{LearningConfig, LearningSession, SelfLearningService};
use crate::AppState;

/// Learning request (optional config override)
#[derive(Debug, Deserialize)]
pub struct LearnRequest {
    pub max_queries: Option<usize>,
}

/// Learning response
#[derive(Debug, Serialize)]
pub struct LearnResponse {
    pub success: bool,
    pub session: Option<LearningSession>,
    pub error: Option<String>,
}

/// Batch learning response
#[derive(Debug, Serialize)]
pub struct BatchLearnResponse {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub sessions: Vec<LearnResponse>,
}

/// Trigger learning for a specific Rei
async fn learn_rei(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<Option<LearnRequest>>,
) -> Result<Json<LearnResponse>, (axum::http::StatusCode, String)> {
    // Check required services
    let memory_kai = state.memory_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "MemoryKai not available".to_string(),
    ))?;

    let embedding = state.embedding.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "Embedding service not available".to_string(),
    ))?;

    let web_search = state.web_search.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "WebSearch not available".to_string(),
    ))?;

    // Build config from request
    let config = payload.and_then(|p| {
        p.max_queries.map(|max| LearningConfig {
            max_queries: max,
            ..Default::default()
        })
    });

    // Create service and execute learning
    let service = SelfLearningService::new(
        state.pool.clone(),
        memory_kai.clone(),
        embedding.clone(),
        web_search.clone(),
        config,
    );

    match service.learn(rei_id).await {
        Ok(session) => {
            tracing::info!(
                "🎓 Learning completed for {}: {} memories stored",
                session.rei_name,
                session.memories_stored
            );
            Ok(Json(LearnResponse {
                success: true,
                session: Some(session),
                error: None,
            }))
        }
        Err(e) => {
            tracing::warn!("⚠️  Learning failed for {}: {}", rei_id, e);
            Ok(Json(LearnResponse {
                success: false,
                session: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

/// Trigger learning for all Reis
async fn learn_all(
    State(state): State<AppState>,
) -> Result<Json<BatchLearnResponse>, (axum::http::StatusCode, String)> {
    // Check required services
    let memory_kai = state.memory_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "MemoryKai not available".to_string(),
    ))?;

    let embedding = state.embedding.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "Embedding service not available".to_string(),
    ))?;

    let web_search = state.web_search.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "WebSearch not available".to_string(),
    ))?;

    let service = SelfLearningService::new(
        state.pool.clone(),
        memory_kai.clone(),
        embedding.clone(),
        web_search.clone(),
        None,
    );

    let results = service.learn_all().await;

    let mut sessions = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for result in results {
        match result {
            Ok(session) => {
                successful += 1;
                sessions.push(LearnResponse {
                    success: true,
                    session: Some(session),
                    error: None,
                });
            }
            Err(e) => {
                failed += 1;
                sessions.push(LearnResponse {
                    success: false,
                    session: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    tracing::info!(
        "🎓 Batch learning completed: {} successful, {} failed",
        successful,
        failed
    );

    Ok(Json(BatchLearnResponse {
        total: sessions.len(),
        successful,
        failed,
        sessions,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/learn", post(learn_rei))
        .route("/kaiba/learn/all", post(learn_all))
}
