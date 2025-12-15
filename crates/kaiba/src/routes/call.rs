//! Call Routes - LLM Invocation

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::AppState;
use crate::models::{
    CallRequest, CallResponse, CallLog, MemoryReference,
    Rei, ReiState, Tei,
};

/// Select Tei based on Rei's energy level
fn select_tei<'a>(energy_level: i32, teis: &'a [Tei]) -> Option<&'a Tei> {
    if teis.is_empty() {
        return None;
    }

    if energy_level < 20 {
        // Tired mode: use fallback
        teis.iter()
            .find(|t| t.is_fallback)
            .or_else(|| teis.iter().max_by_key(|t| t.priority))
    } else if energy_level < 50 {
        // Low energy: use mid-tier (priority >= 1)
        teis.iter()
            .filter(|t| t.priority >= 1)
            .min_by_key(|t| t.priority)
            .or_else(|| teis.iter().min_by_key(|t| t.priority))
    } else {
        // Full energy: use best (lowest priority number)
        teis.iter().min_by_key(|t| t.priority)
    }
}

/// Call LLM with Rei context
async fn call_llm(
    State(pool): State<PgPool>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<CallRequest>,
) -> Result<Json<CallResponse>, (axum::http::StatusCode, String)> {
    // 1. Load Rei
    let rei = sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
        .bind(rei_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei not found".to_string()))?;

    // 2. Load Rei state
    let state = sqlx::query_as::<_, ReiState>(
        "SELECT * FROM rei_states WHERE rei_id = $1"
    )
    .bind(rei_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((axum::http::StatusCode::NOT_FOUND, "Rei state not found".to_string()))?;

    // 3. Load requested Teis
    let teis = if payload.tei_ids.is_empty() {
        // If no Teis specified, use all associated Teis
        sqlx::query_as::<_, Tei>(
            r#"
            SELECT t.* FROM teis t
            INNER JOIN rei_teis rt ON t.id = rt.tei_id
            WHERE rt.rei_id = $1
            ORDER BY t.priority
            "#,
        )
        .bind(rei_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        // Load specific Teis
        let mut teis = Vec::new();
        for tei_id in &payload.tei_ids {
            if let Some(tei) = sqlx::query_as::<_, Tei>("SELECT * FROM teis WHERE id = $1")
                .bind(tei_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            {
                teis.push(tei);
            }
        }
        teis
    };

    if teis.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "No Teis available for this Rei".to_string(),
        ));
    }

    // 4. Select Tei based on energy
    let selected_tei = select_tei(state.energy_level, &teis)
        .ok_or((axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to select Tei".to_string()))?;

    tracing::info!(
        "Call for Rei {} using Tei {} ({}) - Energy: {}",
        rei.name, selected_tei.name, selected_tei.model_id, state.energy_level
    );

    // 5. TODO: Combine expertise from all Teis
    // let combined_expertise = combine_expertise(&teis);

    // 6. TODO: Search relevant memories from Qdrant (if requested)
    let memories_included: Vec<MemoryReference> = vec![];

    // 7. TODO: Build prompt with Rei identity, expertise, memories, and message
    // let prompt = build_prompt(&rei, &combined_expertise, &memories, &payload.message);

    // 8. TODO: Call LLM via llm-toolkit
    // For now, return mock response
    let response_text = format!(
        "[Mock Response from {} via {}]\n\nReceived: {}\n\nThis is a placeholder response. LLM integration pending.",
        rei.name,
        selected_tei.model_id,
        payload.message
    );
    let tokens_consumed = 100; // Mock

    // 9. Update Rei state (consume tokens, update last_active)
    sqlx::query(
        r#"
        UPDATE rei_states
        SET tokens_used = tokens_used + $2, last_active_at = NOW()
        WHERE rei_id = $1
        "#,
    )
    .bind(rei_id)
    .bind(tokens_consumed)
    .execute(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 10. Log the call
    sqlx::query(
        r#"
        INSERT INTO call_logs (rei_id, tei_id, message, response, tokens_consumed, context)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(rei_id)
    .bind(selected_tei.id)
    .bind(&payload.message)
    .bind(&response_text)
    .bind(tokens_consumed)
    .bind(serde_json::to_value(&payload.context).ok())
    .execute(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CallResponse {
        response: response_text,
        tei_used: selected_tei.id,
        tokens_consumed,
        memories_included,
    }))
}

/// Get call history for a Rei
async fn get_call_history(
    State(pool): State<PgPool>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<CallLog>>, (axum::http::StatusCode, String)> {
    let logs = sqlx::query_as::<_, CallLog>(
        "SELECT * FROM call_logs WHERE rei_id = $1 ORDER BY created_at DESC LIMIT 100"
    )
    .bind(rei_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(logs))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/call", post(call_llm))
        .route("/kaiba/rei/:rei_id/calls", axum::routing::get(get_call_history))
}
