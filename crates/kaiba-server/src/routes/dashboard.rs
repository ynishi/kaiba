//! Dashboard Routes - Status overview for a Rei
//!
//! Provides comprehensive status information at a glance.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::models::{
    DashboardActivity, DashboardReiInfo, DashboardResponse, DashboardState, DashboardStats,
    DashboardWebhooks,
};
use crate::AppState;

/// Get Rei dashboard - comprehensive status overview
#[utoipa::path(
    get,
    path = "/kaiba/rei/{id}/dashboard",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    responses(
        (status = 200, description = "Dashboard data", body = DashboardResponse),
        (status = 404, description = "Rei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Dashboard"
)]
pub async fn get_dashboard(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DashboardResponse>, (axum::http::StatusCode, String)> {
    // Get Rei and state
    let (rei, rei_state) = state
        .rei_service
        .get_by_id(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ))?;

    // Get memory count from Qdrant
    let memory_count = match &state.memory_kai {
        Some(kai) => kai.count_memories(&id.to_string()).await.unwrap_or(0),
        None => 0,
    };

    // Get Tei count
    let tei_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rei_teis WHERE rei_id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    // Get webhook stats
    let webhook_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM webhooks WHERE rei_id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let last_delivery: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        r#"
        SELECT MAX(wd.created_at)
        FROM webhook_deliveries wd
        JOIN webhooks w ON w.id = wd.webhook_id
        WHERE w.rei_id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();

    let recent_failures: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM webhook_deliveries wd
        JOIN webhooks w ON w.id = wd.webhook_id
        WHERE w.rei_id = $1
          AND wd.success = false
          AND wd.created_at > NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let response = DashboardResponse {
        rei: DashboardReiInfo {
            id: rei.id,
            name: rei.name,
            role: rei.role,
            avatar_url: rei.avatar_url,
        },
        state: DashboardState {
            energy_level: rei_state.energy_level,
            mood: rei_state.mood,
            tokens_used: rei_state.tokens_used,
            token_budget: rei_state.token_budget,
            energy_regen_per_hour: rei_state.energy_regen_per_hour,
        },
        activity: DashboardActivity {
            last_active_at: rei_state.last_active_at,
            last_learn_at: rei_state.last_learn_at,
            last_digest_at: rei_state.last_digest_at,
        },
        stats: DashboardStats {
            memory_count,
            tei_count,
        },
        webhooks: DashboardWebhooks {
            webhook_count,
            last_delivery_at: last_delivery,
            recent_failures,
        },
    };

    Ok(Json(response))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/kaiba/rei/:id/dashboard", get(get_dashboard))
}
