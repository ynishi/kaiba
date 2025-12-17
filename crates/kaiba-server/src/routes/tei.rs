//! Tei (体) Routes - Execution Interface Management
//!
//! HTTP handlers that delegate to TeiService for business logic.

use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Json, Router,
};
use uuid::Uuid;

use crate::models::{AssociateTeiRequest, CreateTeiRequest, Provider, TeiResponse, UpdateTeiRequest};
use crate::AppState;

/// Convert DTO Provider to domain Provider
fn to_domain_provider(p: Provider) -> kaiba::Provider {
    match p {
        Provider::Anthropic => kaiba::Provider::Anthropic,
        Provider::OpenAI => kaiba::Provider::OpenAI,
        Provider::Google => kaiba::Provider::Google,
    }
}

/// List all Teis
#[utoipa::path(
    get,
    path = "/kaiba/tei",
    responses(
        (status = 200, description = "List of all Teis", body = Vec<TeiResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn list_teis(
    State(state): State<AppState>,
) -> Result<Json<Vec<TeiResponse>>, (axum::http::StatusCode, String)> {
    let teis = state
        .tei_service
        .list_all()
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<TeiResponse> = teis
        .into_iter()
        .map(|tei| TeiResponse {
            id: tei.id,
            name: tei.name,
            provider: tei.provider,
            model_id: tei.model_id,
            is_fallback: tei.is_fallback,
            priority: tei.priority,
            config: tei.config,
            expertise: tei.expertise,
            created_at: tei.created_at,
            updated_at: tei.updated_at,
        })
        .collect();

    Ok(Json(responses))
}

/// Create new Tei
#[utoipa::path(
    post,
    path = "/kaiba/tei",
    request_body = CreateTeiRequest,
    responses(
        (status = 200, description = "Tei created", body = TeiResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn create_tei(
    State(state): State<AppState>,
    Json(payload): Json<CreateTeiRequest>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    let tei = state
        .tei_service
        .create(
            payload.name,
            to_domain_provider(payload.provider),
            payload.model_id,
            payload.is_fallback,
            payload.priority,
            payload.config,
            payload.expertise,
        )
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TeiResponse {
        id: tei.id,
        name: tei.name,
        provider: tei.provider,
        model_id: tei.model_id,
        is_fallback: tei.is_fallback,
        priority: tei.priority,
        config: tei.config,
        expertise: tei.expertise,
        created_at: tei.created_at,
        updated_at: tei.updated_at,
    }))
}

/// Get Tei by ID
#[utoipa::path(
    get,
    path = "/kaiba/tei/{id}",
    params(("id" = Uuid, Path, description = "Tei ID")),
    responses(
        (status = 200, description = "Tei found", body = TeiResponse),
        (status = 404, description = "Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn get_tei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    let tei = state
        .tei_service
        .get_by_id(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ))?;

    Ok(Json(TeiResponse {
        id: tei.id,
        name: tei.name,
        provider: tei.provider,
        model_id: tei.model_id,
        is_fallback: tei.is_fallback,
        priority: tei.priority,
        config: tei.config,
        expertise: tei.expertise,
        created_at: tei.created_at,
        updated_at: tei.updated_at,
    }))
}

/// Update Tei
#[utoipa::path(
    put,
    path = "/kaiba/tei/{id}",
    params(("id" = Uuid, Path, description = "Tei ID")),
    request_body = UpdateTeiRequest,
    responses(
        (status = 200, description = "Tei updated", body = TeiResponse),
        (status = 404, description = "Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn update_tei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTeiRequest>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    let tei = state
        .tei_service
        .update(
            id,
            payload.name,
            payload.provider.map(to_domain_provider),
            payload.model_id,
            payload.is_fallback,
            payload.priority,
            payload.config,
            payload.expertise,
        )
        .await
        .map_err(|e| match e {
            kaiba::DomainError::NotFound { .. } => {
                (axum::http::StatusCode::NOT_FOUND, "Tei not found".to_string())
            }
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        })?;

    Ok(Json(TeiResponse {
        id: tei.id,
        name: tei.name,
        provider: tei.provider,
        model_id: tei.model_id,
        is_fallback: tei.is_fallback,
        priority: tei.priority,
        config: tei.config,
        expertise: tei.expertise,
        created_at: tei.created_at,
        updated_at: tei.updated_at,
    }))
}

/// Delete Tei
#[utoipa::path(
    delete,
    path = "/kaiba/tei/{id}",
    params(("id" = Uuid, Path, description = "Tei ID")),
    responses(
        (status = 200, description = "Tei deleted"),
        (status = 404, description = "Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn delete_tei(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let deleted = state
        .tei_service
        .delete(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !deleted {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Tei deleted"
    })))
}

/// Get Tei expertise
#[utoipa::path(
    get,
    path = "/kaiba/tei/{id}/expertise",
    params(("id" = Uuid, Path, description = "Tei ID")),
    responses(
        (status = 200, description = "Tei expertise"),
        (status = 404, description = "Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn get_tei_expertise(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let expertise = state
        .tei_service
        .get_expertise(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(expertise.unwrap_or(serde_json::json!(null))))
}

/// Update Tei expertise
#[utoipa::path(
    put,
    path = "/kaiba/tei/{id}/expertise",
    params(("id" = Uuid, Path, description = "Tei ID")),
    responses(
        (status = 200, description = "Expertise updated"),
        (status = 404, description = "Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn update_tei_expertise(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(expertise): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = state
        .tei_service
        .update_expertise(id, expertise)
        .await
        .map_err(|e| match e {
            kaiba::DomainError::NotFound { .. } => {
                (axum::http::StatusCode::NOT_FOUND, "Tei not found".to_string())
            }
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        })?;

    Ok(Json(result))
}

// ============================================
// Rei-Tei Association Routes
// ============================================

/// List Teis associated with a Rei
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/teis",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "List of associated Teis", body = Vec<TeiResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn list_rei_teis(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<TeiResponse>>, (axum::http::StatusCode, String)> {
    let teis = state
        .tei_service
        .list_by_rei(rei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<TeiResponse> = teis
        .into_iter()
        .map(|tei| TeiResponse {
            id: tei.id,
            name: tei.name,
            provider: tei.provider,
            model_id: tei.model_id,
            is_fallback: tei.is_fallback,
            priority: tei.priority,
            config: tei.config,
            expertise: tei.expertise,
            created_at: tei.created_at,
            updated_at: tei.updated_at,
        })
        .collect();

    Ok(Json(responses))
}

/// Associate Tei with Rei
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/teis",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = AssociateTeiRequest,
    responses(
        (status = 200, description = "Tei associated"),
        (status = 404, description = "Rei or Tei not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn associate_tei(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<AssociateTeiRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    state
        .tei_service
        .associate(rei_id, payload.tei_id)
        .await
        .map_err(|e| match e {
            kaiba::DomainError::NotFound { entity_type, .. } => (
                axum::http::StatusCode::NOT_FOUND,
                format!("{} not found", entity_type),
            ),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        })?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Tei associated with Rei"
    })))
}

/// Disassociate Tei from Rei
#[utoipa::path(
    delete,
    path = "/kaiba/rei/{rei_id}/teis/{tei_id}",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("tei_id" = Uuid, Path, description = "Tei ID")
    ),
    responses(
        (status = 200, description = "Tei disassociated"),
        (status = 404, description = "Association not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Tei"
)]
pub async fn disassociate_tei(
    State(state): State<AppState>,
    Path((rei_id, tei_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let removed = state
        .tei_service
        .disassociate(rei_id, tei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !removed {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Association not found".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Tei disassociated from Rei"
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        // Tei CRUD
        .route("/kaiba/tei", get(list_teis).post(create_tei))
        .route(
            "/kaiba/tei/:id",
            get(get_tei).put(update_tei).delete(delete_tei),
        )
        .route(
            "/kaiba/tei/:id/expertise",
            get(get_tei_expertise).put(update_tei_expertise),
        )
        // Rei-Tei associations
        .route(
            "/kaiba/rei/:rei_id/teis",
            get(list_rei_teis).post(associate_tei),
        )
        .route("/kaiba/rei/:rei_id/teis/:tei_id", delete(disassociate_tei))
}
