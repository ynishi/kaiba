//! Learning Routes - Self-learning & energy management API endpoints
//!
//! POST /kaiba/rei/:rei_id/learn - Trigger learning for a specific Rei
//! POST /kaiba/learn/all - Trigger learning for all Reis
//! POST /kaiba/rei/:rei_id/recharge - Manually recharge Rei's energy

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::self_learning::{LearningConfig, LearningSession, SelfLearningService};
use crate::AppState;

/// Learning request (optional config override)
#[derive(Debug, Deserialize, ToSchema)]
pub struct LearnRequest {
    pub max_queries: Option<usize>,
    /// Force learning even if energy is low
    #[serde(default)]
    pub force: bool,
}

/// Learning response
#[derive(Debug, Serialize, ToSchema)]
pub struct LearnResponse {
    pub success: bool,
    pub session: Option<LearningSession>,
    pub error: Option<String>,
}

/// Batch learning response
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchLearnResponse {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub sessions: Vec<LearnResponse>,
}

/// Trigger learning for a specific Rei
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/learn",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = Option<LearnRequest>,
    responses(
        (status = 200, description = "Learning result", body = LearnResponse),
        (status = 503, description = "Required services unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Learning"
)]
pub async fn learn_rei(
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
    let config = payload.map(|p| LearningConfig {
        max_queries: p.max_queries.unwrap_or(3),
        force: p.force,
        ..Default::default()
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
#[utoipa::path(
    post,
    path = "/kaiba/learn/all",
    responses(
        (status = 200, description = "Batch learning results", body = BatchLearnResponse),
        (status = 503, description = "Required services unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Learning"
)]
pub async fn learn_all(
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

/// Recharge request
#[derive(Debug, Deserialize, ToSchema)]
pub struct RechargeRequest {
    /// Energy to add (can be negative to drain)
    pub energy: i32,
}

/// Recharge response
#[derive(Debug, Serialize, ToSchema)]
pub struct RechargeResponse {
    pub rei_id: Uuid,
    pub previous_energy: i32,
    pub current_energy: i32,
    pub energy_regen_per_hour: i32,
}

/// Helper struct for query result
#[derive(Debug, FromRow)]
struct EnergyUpdate {
    energy_level: i32,
    energy_regen_per_hour: i32,
}

/// Manually recharge Rei's energy
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/recharge",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = RechargeRequest,
    responses(
        (status = 200, description = "Recharge result", body = RechargeResponse),
        (status = 404, description = "Rei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Learning"
)]
pub async fn recharge_rei(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<RechargeRequest>,
) -> Result<Json<RechargeResponse>, (axum::http::StatusCode, String)> {
    // Get current energy
    let current: EnergyUpdate = sqlx::query_as(
        "SELECT energy_level, energy_regen_per_hour FROM rei_states WHERE rei_id = $1",
    )
    .bind(rei_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((
        axum::http::StatusCode::NOT_FOUND,
        "Rei not found".to_string(),
    ))?;

    let previous_energy = current.energy_level;

    // Calculate new energy (clamped to 0-100)
    let new_energy = (current.energy_level + payload.energy).clamp(0, 100);

    // Update energy
    sqlx::query("UPDATE rei_states SET energy_level = $1 WHERE rei_id = $2")
        .bind(new_energy)
        .bind(rei_id)
        .execute(&state.pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        "⚡ Recharged Rei {}: {} -> {} (+{})",
        rei_id,
        previous_energy,
        new_energy,
        payload.energy
    );

    Ok(Json(RechargeResponse {
        rei_id,
        previous_energy,
        current_energy: new_energy,
        energy_regen_per_hour: current.energy_regen_per_hour,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/learn", post(learn_rei))
        .route("/kaiba/rei/:rei_id/recharge", post(recharge_rei))
        .route("/kaiba/learn/all", post(learn_all))
}
