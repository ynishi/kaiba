//! Rei (霊) Routes - Persona Identity Management
//!
//! HTTP handlers that delegate to ReiService for business logic.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::models::{
    CreateReiRequest, ReiResponse, ReiStateResponse, UpdateReiRequest, UpdateReiStateRequest,
};
use crate::AppState;

/// List all Reis
#[utoipa::path(
    get,
    path = "/kaiba/rei",
    responses(
        (status = 200, description = "List of all Reis", body = Vec<ReiResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn list_reis(
    State(state): State<AppState>,
) -> Result<Json<Vec<ReiResponse>>, (axum::http::StatusCode, String)> {
    let results = state
        .rei_service
        .list_all()
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<ReiResponse> = results
        .into_iter()
        .map(|(rei, rei_state)| ReiResponse {
            id: rei.id,
            name: rei.name,
            role: rei.role,
            avatar_url: rei.avatar_url,
            manifest: rei.manifest,
            state: ReiStateResponse {
                energy_level: rei_state.energy_level,
                mood: rei_state.mood,
                token_budget: rei_state.token_budget,
                tokens_used: rei_state.tokens_used,
                last_active_at: rei_state.last_active_at,
                energy_regen_per_hour: rei_state.energy_regen_per_hour,
            },
            created_at: rei.created_at,
            updated_at: rei.updated_at,
        })
        .collect();

    Ok(Json(responses))
}

/// Create new Rei
#[utoipa::path(
    post,
    path = "/kaiba/rei",
    request_body = CreateReiRequest,
    responses(
        (status = 200, description = "Rei created successfully", body = ReiResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn create_rei(
    State(state): State<AppState>,
    Json(payload): Json<CreateReiRequest>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    let (rei, rei_state) = state
        .rei_service
        .create(
            payload.name,
            payload.role,
            payload.avatar_url,
            payload.manifest,
        )
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: ReiStateResponse {
            energy_level: rei_state.energy_level,
            mood: rei_state.mood,
            token_budget: rei_state.token_budget,
            tokens_used: rei_state.tokens_used,
            last_active_at: rei_state.last_active_at,
            energy_regen_per_hour: rei_state.energy_regen_per_hour,
        },
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Get Rei by ID
#[utoipa::path(
    get,
    path = "/kaiba/rei/{id}",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    responses(
        (status = 200, description = "Rei found", body = ReiResponse),
        (status = 404, description = "Rei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn get_rei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    let (rei, rei_state) = state
        .rei_service
        .get_by_id(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ))?;

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: ReiStateResponse {
            energy_level: rei_state.energy_level,
            mood: rei_state.mood,
            token_budget: rei_state.token_budget,
            tokens_used: rei_state.tokens_used,
            last_active_at: rei_state.last_active_at,
            energy_regen_per_hour: rei_state.energy_regen_per_hour,
        },
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Update Rei
#[utoipa::path(
    put,
    path = "/kaiba/rei/{id}",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    request_body = UpdateReiRequest,
    responses(
        (status = 200, description = "Rei updated successfully", body = ReiResponse),
        (status = 404, description = "Rei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn update_rei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReiRequest>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    let (rei, rei_state) = state
        .rei_service
        .update(
            id,
            payload.name,
            payload.role,
            payload.avatar_url,
            payload.manifest,
        )
        .await
        .map_err(|e| match e {
            kaiba::DomainError::NotFound { .. } => {
                (axum::http::StatusCode::NOT_FOUND, "Rei not found".to_string())
            }
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        })?;

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: ReiStateResponse {
            energy_level: rei_state.energy_level,
            mood: rei_state.mood,
            token_budget: rei_state.token_budget,
            tokens_used: rei_state.tokens_used,
            last_active_at: rei_state.last_active_at,
            energy_regen_per_hour: rei_state.energy_regen_per_hour,
        },
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Delete Rei
#[utoipa::path(
    delete,
    path = "/kaiba/rei/{id}",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    responses(
        (status = 200, description = "Rei deleted successfully"),
        (status = 404, description = "Rei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn delete_rei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let deleted = state
        .rei_service
        .delete(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !deleted {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Rei deleted"
    })))
}

/// Get Rei state
#[utoipa::path(
    get,
    path = "/kaiba/rei/{id}/state",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    responses(
        (status = 200, description = "Rei state found", body = ReiStateResponse),
        (status = 404, description = "Rei state not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn get_rei_state(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReiStateResponse>, (axum::http::StatusCode, String)> {
    let rei_state = state
        .rei_service
        .get_state(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei state not found".to_string(),
        ))?;

    Ok(Json(ReiStateResponse {
        energy_level: rei_state.energy_level,
        mood: rei_state.mood,
        token_budget: rei_state.token_budget,
        tokens_used: rei_state.tokens_used,
        last_active_at: rei_state.last_active_at,
        energy_regen_per_hour: rei_state.energy_regen_per_hour,
    }))
}

/// Update Rei state
#[utoipa::path(
    put,
    path = "/kaiba/rei/{id}/state",
    params(
        ("id" = Uuid, Path, description = "Rei ID")
    ),
    request_body = UpdateReiStateRequest,
    responses(
        (status = 200, description = "Rei state updated", body = ReiStateResponse),
        (status = 404, description = "Rei state not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Rei"
)]
pub async fn update_rei_state(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReiStateRequest>,
) -> Result<Json<ReiStateResponse>, (axum::http::StatusCode, String)> {
    let rei_state = state
        .rei_service
        .update_state(
            id,
            payload.energy_level,
            payload.mood,
            payload.token_budget,
            payload.tokens_used,
            payload.energy_regen_per_hour,
        )
        .await
        .map_err(|e| match e {
            kaiba::DomainError::NotFound { .. } => (
                axum::http::StatusCode::NOT_FOUND,
                "Rei state not found".to_string(),
            ),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        })?;

    Ok(Json(ReiStateResponse {
        energy_level: rei_state.energy_level,
        mood: rei_state.mood,
        token_budget: rei_state.token_budget,
        tokens_used: rei_state.tokens_used,
        last_active_at: rei_state.last_active_at,
        energy_regen_per_hour: rei_state.energy_regen_per_hour,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei", get(list_reis).post(create_rei))
        .route(
            "/kaiba/rei/:id",
            get(get_rei).put(update_rei).delete(delete_rei),
        )
        .route(
            "/kaiba/rei/:id/state",
            get(get_rei_state).put(update_rei_state),
        )
}
