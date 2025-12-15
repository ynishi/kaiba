//! Rei (霊) Routes - Persona Identity Management

use axum::{
    extract::{Path, State},
    routing::{get, post, put, delete},
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{
    CreateReiRequest, UpdateReiRequest, UpdateReiStateRequest,
    Rei, ReiState, ReiResponse, ReiStateResponse,
};

/// List all Reis
async fn list_reis(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<ReiResponse>>, (axum::http::StatusCode, String)> {
    let reis = sqlx::query_as::<_, Rei>("SELECT * FROM reis ORDER BY created_at DESC")
        .fetch_all(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut responses = Vec::new();
    for rei in reis {
        let state = sqlx::query_as::<_, ReiState>(
            "SELECT * FROM rei_states WHERE rei_id = $1"
        )
        .bind(rei.id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let state_response = state
            .map(ReiStateResponse::from)
            .unwrap_or(ReiStateResponse {
                energy_level: 100,
                mood: "neutral".to_string(),
                token_budget: 100000,
                tokens_used: 0,
                last_active_at: None,
            });

        responses.push(ReiResponse {
            id: rei.id,
            name: rei.name,
            role: rei.role,
            avatar_url: rei.avatar_url,
            manifest: rei.manifest,
            state: state_response,
            created_at: rei.created_at,
            updated_at: rei.updated_at,
        });
    }

    Ok(Json(responses))
}

/// Create new Rei
async fn create_rei(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateReiRequest>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    let manifest = payload.manifest.unwrap_or(serde_json::json!({}));

    // Create Rei
    let rei = sqlx::query_as::<_, Rei>(
        r#"
        INSERT INTO reis (name, role, avatar_url, manifest)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.role)
    .bind(&payload.avatar_url)
    .bind(&manifest)
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Create initial state
    let state = sqlx::query_as::<_, ReiState>(
        r#"
        INSERT INTO rei_states (rei_id)
        VALUES ($1)
        RETURNING *
        "#,
    )
    .bind(rei.id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Created Rei: {} ({})", rei.name, rei.id);

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: state.into(),
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Get Rei by ID
async fn get_rei(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    let rei = sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei not found".to_string()))?;

    let state = sqlx::query_as::<_, ReiState>(
        "SELECT * FROM rei_states WHERE rei_id = $1"
    )
    .bind(rei.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let state_response = state
        .map(ReiStateResponse::from)
        .unwrap_or(ReiStateResponse {
            energy_level: 100,
            mood: "neutral".to_string(),
            token_budget: 100000,
            tokens_used: 0,
            last_active_at: None,
        });

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: state_response,
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Update Rei
async fn update_rei(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReiRequest>,
) -> Result<Json<ReiResponse>, (axum::http::StatusCode, String)> {
    // Get current Rei
    let current = sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei not found".to_string()))?;

    // Update with provided values or keep current
    let rei = sqlx::query_as::<_, Rei>(
        r#"
        UPDATE reis
        SET name = $2, role = $3, avatar_url = $4, manifest = $5
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.name.unwrap_or(current.name))
    .bind(payload.role.unwrap_or(current.role))
    .bind(payload.avatar_url.or(current.avatar_url))
    .bind(payload.manifest.unwrap_or(current.manifest))
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let state = sqlx::query_as::<_, ReiState>(
        "SELECT * FROM rei_states WHERE rei_id = $1"
    )
    .bind(rei.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let state_response = state
        .map(ReiStateResponse::from)
        .unwrap_or(ReiStateResponse {
            energy_level: 100,
            mood: "neutral".to_string(),
            token_budget: 100000,
            tokens_used: 0,
            last_active_at: None,
        });

    Ok(Json(ReiResponse {
        id: rei.id,
        name: rei.name,
        role: rei.role,
        avatar_url: rei.avatar_url,
        manifest: rei.manifest,
        state: state_response,
        created_at: rei.created_at,
        updated_at: rei.updated_at,
    }))
}

/// Delete Rei
async fn delete_rei(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = sqlx::query("DELETE FROM reis WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((axum::http::StatusCode::NOT_FOUND, "Rei not found".to_string()));
    }

    tracing::info!("Deleted Rei: {}", id);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Rei deleted"
    })))
}

/// Get Rei state
async fn get_rei_state(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReiStateResponse>, (axum::http::StatusCode, String)> {
    let state = sqlx::query_as::<_, ReiState>(
        "SELECT * FROM rei_states WHERE rei_id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei state not found".to_string()))?;

    Ok(Json(state.into()))
}

/// Update Rei state
async fn update_rei_state(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReiStateRequest>,
) -> Result<Json<ReiStateResponse>, (axum::http::StatusCode, String)> {
    // Get current state
    let current = sqlx::query_as::<_, ReiState>(
        "SELECT * FROM rei_states WHERE rei_id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei state not found".to_string()))?;

    // Update with provided values
    let state = sqlx::query_as::<_, ReiState>(
        r#"
        UPDATE rei_states
        SET energy_level = $2, mood = $3, token_budget = $4, tokens_used = $5,
            last_active_at = NOW()
        WHERE rei_id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.energy_level.unwrap_or(current.energy_level))
    .bind(payload.mood.unwrap_or(current.mood))
    .bind(payload.token_budget.unwrap_or(current.token_budget))
    .bind(payload.tokens_used.unwrap_or(current.tokens_used))
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(state.into()))
}

pub fn router() -> Router<PgPool> {
    Router::new()
        .route("/kaiba/rei", get(list_reis).post(create_rei))
        .route("/kaiba/rei/:id", get(get_rei).put(update_rei).delete(delete_rei))
        .route("/kaiba/rei/:id/state", get(get_rei_state).put(update_rei_state))
}
