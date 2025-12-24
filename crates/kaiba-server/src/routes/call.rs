//! Call Routes - LLM Invocation with RAG

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use uuid::Uuid;

use crate::models::{
    CallLog, CallRequest, CallResponse, Memory, MemoryReference, Rei, ReiState, Tei,
};
use crate::AppState;

/// Select Tei based on Rei's energy level
fn select_tei(energy_level: i32, teis: &[Tei]) -> Option<&Tei> {
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

/// Call LLM with Rei context and RAG
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/call",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = CallRequest,
    responses(
        (status = 200, description = "LLM call successful", body = CallResponse),
        (status = 404, description = "Rei not found"),
        (status = 400, description = "No Teis available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Call"
)]
pub async fn call_llm(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<CallRequest>,
) -> Result<Json<CallResponse>, (axum::http::StatusCode, String)> {
    let pool = &state.pool;

    // 1. Load Rei
    let rei = sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
        .bind(rei_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ))?;

    // 2. Load Rei state
    let rei_state = sqlx::query_as::<_, ReiState>("SELECT * FROM rei_states WHERE rei_id = $1")
        .bind(rei_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei state not found".to_string(),
        ))?;

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
        .fetch_all(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        // Load specific Teis
        let mut teis = Vec::new();
        for tei_id in &payload.tei_ids {
            if let Some(tei) = sqlx::query_as::<_, Tei>("SELECT * FROM teis WHERE id = $1")
                .bind(tei_id)
                .fetch_optional(pool)
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
    let selected_tei = select_tei(rei_state.energy_level, &teis).ok_or((
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to select Tei".to_string(),
    ))?;

    tracing::info!(
        "Call for Rei {} using Tei {} ({}) - Energy: {}",
        rei.name,
        selected_tei.name,
        selected_tei.model_id,
        rei_state.energy_level
    );

    // 5. RAG: Search relevant memories if requested
    let context = payload.context.unwrap_or_default();
    let (memories, memories_included) = if context.include_memories {
        search_memories_for_rag(&state, &rei_id, &payload.message, context.memory_limit).await?
    } else {
        (vec![], vec![])
    };

    // 6. Build system prompt with Rei identity and memories
    let system_prompt = build_system_prompt(&rei, &memories);

    // 7. TODO: Call LLM via llm-toolkit
    // For now, return mock response showing RAG context
    let memory_context = if memories.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n[RAG Context - {} memories retrieved]\n{}",
            memories.len(),
            memories
                .iter()
                .map(|m| format!("- {}", m.content))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let response_text = format!(
        "[Mock Response from {} via {}]{}\n\nReceived: {}\n\nSystem Prompt:\n{}\n\nThis is a placeholder response. LLM integration pending.",
        rei.name,
        selected_tei.model_id,
        memory_context,
        payload.message,
        system_prompt
    );
    let tokens_consumed = 100; // Mock

    // 8. Update Rei state (consume tokens, update last_active)
    sqlx::query(
        r#"
        UPDATE rei_states
        SET tokens_used = tokens_used + $2, last_active_at = NOW()
        WHERE rei_id = $1
        "#,
    )
    .bind(rei_id)
    .bind(tokens_consumed)
    .execute(pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 9. Log the call
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
    .bind(serde_json::to_value(&context).ok())
    .execute(pool)
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
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/calls",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "Call history", body = Vec<CallLog>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Call"
)]
pub async fn get_call_history(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<CallLog>>, (axum::http::StatusCode, String)> {
    let logs = sqlx::query_as::<_, CallLog>(
        "SELECT * FROM call_logs WHERE rei_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(rei_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(logs))
}

// ============================================
// RAG Helper Functions
// ============================================

/// Search memories for RAG context
async fn search_memories_for_rag(
    state: &AppState,
    rei_id: &Uuid,
    query: &str,
    limit: Option<usize>,
) -> Result<(Vec<Memory>, Vec<MemoryReference>), (axum::http::StatusCode, String)> {
    // Check if services are available
    let memory_kai = match &state.memory_kai {
        Some(kai) => kai,
        None => return Ok((vec![], vec![])),
    };

    let embedding_service = match &state.embedding {
        Some(svc) => svc,
        None => return Ok((vec![], vec![])),
    };

    // Generate query embedding
    let query_vector = embedding_service.embed(query).await.map_err(|e| {
        tracing::warn!("Failed to generate embedding for RAG: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Search memories
    let limit = limit.unwrap_or(5);
    let memories = memory_kai
        .search_memories(&rei_id.to_string(), query_vector, limit)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to search memories for RAG: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Build memory references (similarity scores would come from Qdrant)
    let refs: Vec<MemoryReference> = memories
        .iter()
        .map(|m| MemoryReference {
            id: m.id.clone(),
            similarity: 0.0, // TODO: Get actual similarity from Qdrant
        })
        .collect();

    tracing::info!("RAG: Retrieved {} memories for context", memories.len());

    Ok((memories, refs))
}

/// Build system prompt with Rei identity and memories
fn build_system_prompt(rei: &Rei, memories: &[Memory]) -> String {
    let mut prompt = format!("You are {}, {}.\n", rei.name, rei.role);

    // Add manifest if present
    if let Some(manifest) = rei.manifest.as_object() {
        if let Some(personality) = manifest.get("personality") {
            prompt.push_str(&format!("\nPersonality: {}\n", personality));
        }
        if let Some(instructions) = manifest.get("instructions") {
            prompt.push_str(&format!("\nInstructions: {}\n", instructions));
        }
    }

    // Add relevant memories as context
    if !memories.is_empty() {
        prompt.push_str("\n## Relevant Memories\n");
        prompt.push_str("Use the following memories as context for your response:\n\n");
        for mem in memories {
            prompt.push_str(&format!(
                "- [{}] {} (created: {}, importance: {:.2})\n",
                mem.memory_type,
                mem.content,
                mem.created_at.format("%Y-%m-%d %H:%M UTC"),
                mem.importance
            ));
        }
    }

    prompt
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/call", post(call_llm))
        .route(
            "/kaiba/rei/:rei_id/calls",
            axum::routing::get(get_call_history),
        )
}
