//! Trigger Routes - External kick for batch job processing
//!
//! Designed for serverless/sleeping environments where internal scheduler
//! may not run reliably. When triggered, processes all pending jobs based
//! on elapsed time since last execution.
//!
//! Features:
//! - JITTER: Random delay between Rei processing to avoid thundering herd
//! - Batch processing: Handles all Reis in one request

use axum::{extract::State, routing::post, Json, Router};
use chrono::Utc;
use serde::Serialize;
use std::time::Duration;
use utoipa::ToSchema;

use crate::models::Rei;
use crate::services::decision::{Action, DecisionMaker};
use crate::services::digest::DigestService;
use crate::services::self_learning::{LearningConfig, SelfLearningService};
use crate::AppState;

/// Jitter range in milliseconds (0-3000ms = 0-3sec)
const JITTER_MAX_MS: u64 = 3000;

/// Simple jitter using timestamp nanos (no external crate needed)
fn jitter_ms(seed: usize) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    (nanos ^ (seed as u64 * 7919)) % JITTER_MAX_MS
}

/// Trigger response
#[derive(Debug, Serialize, ToSchema)]
pub struct TriggerResponse {
    pub triggered_at: chrono::DateTime<Utc>,
    pub results: Vec<ReiTriggerResult>,
    pub summary: TriggerSummary,
}

/// Result for each Rei
#[derive(Debug, Serialize, ToSchema)]
pub struct ReiTriggerResult {
    pub rei_name: String,
    pub action: String,
    pub success: bool,
    pub details: Option<String>,
}

/// Summary of trigger execution
#[derive(Debug, Serialize, ToSchema)]
pub struct TriggerSummary {
    pub reis_processed: usize,
    pub learns_executed: usize,
    pub digests_executed: usize,
    pub rests_skipped: usize,
    pub errors: usize,
}

/// Trigger all pending jobs
#[utoipa::path(
    post,
    path = "/kaiba/trigger",
    responses(
        (status = 200, description = "Trigger completed", body = TriggerResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Trigger"
)]
pub async fn trigger_jobs(
    State(state): State<AppState>,
) -> Result<Json<TriggerResponse>, (axum::http::StatusCode, String)> {
    let triggered_at = Utc::now();
    let mut results = Vec::new();
    let mut summary = TriggerSummary {
        reis_processed: 0,
        learns_executed: 0,
        digests_executed: 0,
        rests_skipped: 0,
        errors: 0,
    };

    // Get all Reis
    let reis: Vec<Rei> = sqlx::query_as("SELECT * FROM reis")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Check required services
    let (Some(memory_kai), Some(embedding), Some(web_search)) =
        (&state.memory_kai, &state.embedding, &state.web_search)
    else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Required services not available".to_string(),
        ));
    };

    // First, regenerate energy for all Reis
    let _ = sqlx::query(
        r#"
        UPDATE rei_states
        SET energy_level = LEAST(100, energy_level + energy_regen_per_hour)
        WHERE energy_regen_per_hour > 0
        "#,
    )
    .execute(&state.pool)
    .await;

    for (idx, rei) in reis.iter().enumerate() {
        summary.reis_processed += 1;

        // Add jitter between Rei processing (skip first one)
        if idx > 0 {
            let delay = jitter_ms(idx);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        // Get Rei state
        let rei_state = match sqlx::query_as::<_, crate::models::ReiState>(
            "SELECT * FROM rei_states WHERE rei_id = $1",
        )
        .bind(rei.id)
        .fetch_optional(&state.pool)
        .await
        {
            Ok(Some(s)) => s,
            Ok(None) => {
                results.push(ReiTriggerResult {
                    rei_name: rei.name.clone(),
                    action: "Skip".to_string(),
                    success: false,
                    details: Some("No state found".to_string()),
                });
                summary.errors += 1;
                continue;
            }
            Err(e) => {
                results.push(ReiTriggerResult {
                    rei_name: rei.name.clone(),
                    action: "Skip".to_string(),
                    success: false,
                    details: Some(e.to_string()),
                });
                summary.errors += 1;
                continue;
            }
        };

        // Count learning memories for decision
        let memories_count = count_learning_memories(&state, rei.id, &rei_state).await;

        // Make decision
        let decision_maker = DecisionMaker::new(None);
        let decision = decision_maker.decide(&rei_state, memories_count);

        match decision.action {
            Action::Learn => {
                // Execute learn
                let service = SelfLearningService::new(
                    state.pool.clone(),
                    memory_kai.clone(),
                    embedding.clone(),
                    web_search.clone(),
                    Some(LearningConfig {
                        force: true, // Force even if energy is low
                        ..Default::default()
                    }),
                );

                match service.learn(rei.id).await {
                    Ok(session) => {
                        results.push(ReiTriggerResult {
                            rei_name: rei.name.clone(),
                            action: "Learn".to_string(),
                            success: true,
                            details: Some(format!(
                                "{} queries, {} memories stored",
                                session.queries_generated.len(),
                                session.memories_stored
                            )),
                        });
                        summary.learns_executed += 1;
                    }
                    Err(e) => {
                        results.push(ReiTriggerResult {
                            rei_name: rei.name.clone(),
                            action: "Learn".to_string(),
                            success: false,
                            details: Some(e.to_string()),
                        });
                        summary.errors += 1;
                    }
                }
            }
            Action::Digest => {
                // Execute digest
                let service = DigestService::new(
                    state.pool.clone(),
                    memory_kai.clone(),
                    embedding.clone(),
                    None, // Gemini API key from secrets if needed
                );

                match service.digest(rei.id).await {
                    Ok(result) => {
                        results.push(ReiTriggerResult {
                            rei_name: rei.name.clone(),
                            action: "Digest".to_string(),
                            success: true,
                            details: Some(format!(
                                "{} memories processed",
                                result.memories_processed
                            )),
                        });
                        summary.digests_executed += 1;
                    }
                    Err(e) => {
                        results.push(ReiTriggerResult {
                            rei_name: rei.name.clone(),
                            action: "Digest".to_string(),
                            success: false,
                            details: Some(e.to_string()),
                        });
                        summary.errors += 1;
                    }
                }
            }
            Action::Rest => {
                results.push(ReiTriggerResult {
                    rei_name: rei.name.clone(),
                    action: "Rest".to_string(),
                    success: true,
                    details: Some(decision.reason),
                });
                summary.rests_skipped += 1;
            }
        }
    }

    Ok(Json(TriggerResponse {
        triggered_at,
        results,
        summary,
    }))
}

/// Count learning memories for a Rei since last digest
async fn count_learning_memories(
    state: &AppState,
    rei_id: uuid::Uuid,
    rei_state: &crate::models::ReiState,
) -> usize {
    let (Some(memory_kai), Some(embedding)) = (&state.memory_kai, &state.embedding) else {
        return 0;
    };

    let query_vector = match embedding.embed("learning").await {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let memories = match memory_kai
        .search_memories(&rei_id.to_string(), query_vector, 20)
        .await
    {
        Ok(m) => m,
        Err(_) => return 0,
    };

    memories
        .iter()
        .filter(|m| matches!(m.memory_type, crate::models::MemoryType::Learning))
        .filter(|m| {
            // Only count memories created after last digest
            match rei_state.last_digest_at {
                Some(last_digest) => m.created_at > last_digest,
                None => true, // If never digested, count all
            }
        })
        .count()
}

pub fn router() -> Router<AppState> {
    Router::new().route("/kaiba/trigger", post(trigger_jobs))
}
