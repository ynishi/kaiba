//! Tei (体) Routes - Execution Interface Management

use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AssociateTeiRequest, CreateTeiRequest, Tei, TeiResponse, UpdateTeiRequest};
use crate::AppState;

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
    State(pool): State<PgPool>,
) -> Result<Json<Vec<TeiResponse>>, (axum::http::StatusCode, String)> {
    let teis = sqlx::query_as::<_, Tei>("SELECT * FROM teis ORDER BY priority, created_at DESC")
        .fetch_all(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(teis.into_iter().map(TeiResponse::from).collect()))
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
    State(pool): State<PgPool>,
    Json(payload): Json<CreateTeiRequest>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    let config = payload.config.unwrap_or(serde_json::json!({}));

    let tei = sqlx::query_as::<_, Tei>(
        r#"
        INSERT INTO teis (name, provider, model_id, is_fallback, priority, config, expertise)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(&payload.name)
    .bind(payload.provider.to_string())
    .bind(&payload.model_id)
    .bind(payload.is_fallback)
    .bind(payload.priority)
    .bind(&config)
    .bind(&payload.expertise)
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Created Tei: {} ({}) - {}", tei.name, tei.id, tei.model_id);

    Ok(Json(tei.into()))
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
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    let tei = sqlx::query_as::<_, Tei>("SELECT * FROM teis WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ))?;

    Ok(Json(tei.into()))
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
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTeiRequest>,
) -> Result<Json<TeiResponse>, (axum::http::StatusCode, String)> {
    // Get current Tei
    let current = sqlx::query_as::<_, Tei>("SELECT * FROM teis WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ))?;

    let tei = sqlx::query_as::<_, Tei>(
        r#"
        UPDATE teis
        SET name = $2, provider = $3, model_id = $4, is_fallback = $5,
            priority = $6, config = $7, expertise = $8
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.name.unwrap_or(current.name))
    .bind(
        payload
            .provider
            .map(|p| p.to_string())
            .unwrap_or(current.provider),
    )
    .bind(payload.model_id.unwrap_or(current.model_id))
    .bind(payload.is_fallback.unwrap_or(current.is_fallback))
    .bind(payload.priority.unwrap_or(current.priority))
    .bind(payload.config.unwrap_or(current.config))
    .bind(payload.expertise.or(current.expertise))
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tei.into()))
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
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = sqlx::query("DELETE FROM teis WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ));
    }

    tracing::info!("Deleted Tei: {}", id);

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
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let tei = sqlx::query_as::<_, Tei>("SELECT * FROM teis WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ))?;

    Ok(Json(tei.expertise.unwrap_or(serde_json::json!(null))))
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
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(expertise): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = sqlx::query("UPDATE teis SET expertise = $2 WHERE id = $1")
        .bind(id)
        .bind(&expertise)
        .execute(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ));
    }

    Ok(Json(expertise))
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
    State(pool): State<PgPool>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<TeiResponse>>, (axum::http::StatusCode, String)> {
    let teis = sqlx::query_as::<_, Tei>(
        r#"
        SELECT t.* FROM teis t
        INNER JOIN rei_teis rt ON t.id = rt.tei_id
        WHERE rt.rei_id = $1
        ORDER BY t.priority, t.created_at DESC
        "#,
    )
    .bind(rei_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(teis.into_iter().map(TeiResponse::from).collect()))
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
    State(pool): State<PgPool>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<AssociateTeiRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // Verify Rei exists
    let rei_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM reis WHERE id = $1)")
            .bind(rei_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !rei_exists {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ));
    }

    // Verify Tei exists
    let tei_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM teis WHERE id = $1)")
            .bind(payload.tei_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !tei_exists {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Tei not found".to_string(),
        ));
    }

    // Create association (ignore if already exists)
    sqlx::query(
        r#"
        INSERT INTO rei_teis (rei_id, tei_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(rei_id)
    .bind(payload.tei_id)
    .execute(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Associated Tei {} with Rei {}", payload.tei_id, rei_id);

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
    State(pool): State<PgPool>,
    Path((rei_id, tei_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = sqlx::query("DELETE FROM rei_teis WHERE rei_id = $1 AND tei_id = $2")
        .bind(rei_id)
        .bind(tei_id)
        .execute(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
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
