//! Prompt Routes - Generate prompts for external Teis
//!
//! Instead of wrapping LLMs, Kaiba generates prompts that external
//! execution environments (Claude Code, Casting, etc.) can use directly.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::models::{
    Memory, PromptFormat, PromptQuery, PromptResponse, Rei, ReiState, ReiSummary, TagMatchMode,
};
use crate::services::SearchFilter;
use crate::AppState;

/// Generate prompt for external Tei
///
/// GET /kaiba/rei/{id}/prompt?format=casting&include_memories=true&context=...
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/prompt",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        PromptQuery
    ),
    responses(
        (status = 200, description = "Generated prompt", body = PromptResponse),
        (status = 404, description = "Rei not found"),
        (status = 400, description = "Invalid format"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Prompt"
)]
pub async fn generate_prompt(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Query(query): Query<PromptQuery>,
) -> Result<Json<PromptResponse>, (axum::http::StatusCode, String)> {
    let pool = &state.pool;

    // 1. Parse format
    let format: PromptFormat = query
        .format
        .as_deref()
        .map(|s| s.parse())
        .transpose()
        .map_err(|e: String| (axum::http::StatusCode::BAD_REQUEST, e))?
        .unwrap_or_default();

    // 2. Load Rei
    let rei = sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
        .bind(rei_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei not found".to_string(),
        ))?;

    // 3. Load Rei state
    let rei_state = sqlx::query_as::<_, ReiState>("SELECT * FROM rei_states WHERE rei_id = $1")
        .bind(rei_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Rei state not found".to_string(),
        ))?;

    // 4. RAG: Search relevant memories if requested
    let memories = if query.include_memories {
        let context = query.context.as_deref().unwrap_or(&rei.name);
        let focus_tags: Vec<String> = query
            .focus_tags
            .as_deref()
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
            .unwrap_or_default();
        search_memories_for_prompt(
            &state,
            &rei_id,
            context,
            query.memory_limit,
            focus_tags,
            query.min_importance,
        )
        .await?
    } else {
        vec![]
    };

    // 5. Generate prompt in requested format
    let system_prompt = format_prompt(&rei, &rei_state, &memories, format);

    tracing::info!(
        "Generated {} prompt for Rei {} with {} memories",
        format_name(format),
        rei.name,
        memories.len()
    );

    Ok(Json(PromptResponse {
        system_prompt,
        format: format_name(format).to_string(),
        rei: ReiSummary {
            id: rei.id,
            name: rei.name,
            role: rei.role,
            energy_level: rei_state.energy_level,
            mood: rei_state.mood,
        },
        memories_included: memories.len(),
    }))
}

// ============================================
// Formatters
// ============================================

fn format_name(format: PromptFormat) -> &'static str {
    match format {
        PromptFormat::Casting => "casting",
        PromptFormat::ClaudeCode => "claude-code",
        PromptFormat::Raw => "raw",
    }
}

/// Generate prompt in the requested format
fn format_prompt(rei: &Rei, state: &ReiState, memories: &[Memory], format: PromptFormat) -> String {
    match format {
        PromptFormat::Casting => format_casting(rei, state, memories),
        PromptFormat::ClaudeCode => format_claude_code(rei, state, memories),
        PromptFormat::Raw => format_raw(rei, state, memories),
    }
}

/// Casting CLI format (system_prompt.txt compatible)
fn format_casting(rei: &Rei, state: &ReiState, memories: &[Memory]) -> String {
    let mut prompt = format!(
        r#"YOU ARE a Persona named "{name}" who embodies the role of {role}.

Your role is to:
- Embody this persona as a helpful, knowledgeable character
- Support users in a friendly and approachable way
- Respond authentically based on your personality and memories

## Identity
- Name: {name}
- Role: {role}
- Current Mood: {mood}
- Energy Level: {energy}%
"#,
        name = rei.name,
        role = rei.role,
        mood = state.mood,
        energy = state.energy_level,
    );

    // Add personality from manifest
    if let Some(manifest) = rei.manifest.as_object() {
        if let Some(personality) = manifest.get("personality") {
            prompt.push_str(&format!("\n## Personality\n{}\n", personality));
        }
        if let Some(instructions) = manifest.get("instructions") {
            prompt.push_str(&format!("\n## Special Instructions\n{}\n", instructions));
        }
        if let Some(quirks) = manifest.get("quirks") {
            prompt.push_str(&format!("\n## Quirks & Mannerisms\n{}\n", quirks));
        }
    }

    // Add memories as context
    if !memories.is_empty() {
        prompt.push_str("\n## Your Memories (use as context)\n");
        for mem in memories {
            prompt.push_str(&format!("- [{}] {}\n", mem.memory_type, mem.content));
        }
    }

    // Add memory management instructions
    prompt.push_str(
        r#"
## Memory Management
If the `kaiba` CLI is available, you can access your memories:

**Search memories** (not all memories are in this prompt):
```bash
kaiba memory search "<query>"
```

**Save new memories**:
```bash
kaiba memory add -t <type> "<content>"
```
Types: learning, fact, expertise, reflection

Use search to recall past conversations, projects, or learnings that aren't in the initial context.
"#,
    );

    prompt
}

/// Claude Code --system-prompt format
fn format_claude_code(rei: &Rei, state: &ReiState, memories: &[Memory]) -> String {
    let mut prompt = format!(
        r#"You are {name}, {role}.

Current state: {mood} (Energy: {energy}%)
"#,
        name = rei.name,
        role = rei.role,
        mood = state.mood,
        energy = state.energy_level,
    );

    // Add manifest info
    if let Some(manifest) = rei.manifest.as_object() {
        if let Some(personality) = manifest.get("personality") {
            prompt.push_str(&format!("\nPersonality: {}\n", personality));
        }
        if let Some(instructions) = manifest.get("instructions") {
            prompt.push_str(&format!("\n{}\n", instructions));
        }
    }

    // Add memories
    if !memories.is_empty() {
        prompt.push_str("\n## Context from Memory\n");
        for mem in memories {
            prompt.push_str(&format!("- {}\n", mem.content));
        }
    }

    // Add memory management instructions
    prompt.push_str(
        r#"
## Memory
- Search: `kaiba memory search "<query>"` (not all memories are in this prompt)
- Save: `kaiba memory add -t <type> "<content>"`
Types: learning, fact, expertise, reflection
"#,
    );

    prompt
}

/// Raw format with clear sections
fn format_raw(rei: &Rei, state: &ReiState, memories: &[Memory]) -> String {
    let mut prompt = String::new();

    // Identity section
    prompt.push_str("=== IDENTITY ===\n");
    prompt.push_str(&format!("Name: {}\n", rei.name));
    prompt.push_str(&format!("Role: {}\n", rei.role));
    prompt.push_str(&format!("Mood: {}\n", state.mood));
    prompt.push_str(&format!("Energy: {}%\n", state.energy_level));

    // Manifest section
    prompt.push_str("\n=== MANIFEST ===\n");
    prompt.push_str(&serde_json::to_string_pretty(&rei.manifest).unwrap_or_default());

    // Memories section
    if !memories.is_empty() {
        prompt.push_str("\n\n=== MEMORIES ===\n");
        for mem in memories {
            prompt.push_str(&format!(
                "[{}] (importance: {:.2}) {}\n",
                mem.memory_type, mem.importance, mem.content
            ));
        }
    }

    prompt
}

// ============================================
// RAG Helper
// ============================================

/// Search memories for prompt context
async fn search_memories_for_prompt(
    state: &AppState,
    rei_id: &Uuid,
    query: &str,
    limit: Option<usize>,
    focus_tags: Vec<String>,
    min_importance: Option<f32>,
) -> Result<Vec<Memory>, (axum::http::StatusCode, String)> {
    let memory_kai = match &state.memory_kai {
        Some(kai) => kai,
        None => return Ok(vec![]),
    };

    let embedding_service = match &state.embedding {
        Some(svc) => svc,
        None => return Ok(vec![]),
    };

    // Generate query embedding
    let query_vector = embedding_service.embed(query).await.map_err(|e| {
        tracing::warn!("Failed to generate embedding for prompt RAG: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Build search filter
    let filter = SearchFilter {
        memory_type: None, // Don't filter by type in prompt context
        tags: focus_tags,
        tags_match_mode: TagMatchMode::Any, // OR match for prompt context
        min_importance,
    };

    // Search memories
    let limit = limit.unwrap_or(5);
    let memories = memory_kai
        .search_memories_with_filter(&rei_id.to_string(), query_vector, limit, filter)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to search memories for prompt: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(memories)
}

pub fn router() -> Router<AppState> {
    Router::new().route("/kaiba/rei/:rei_id/prompt", get(generate_prompt))
}
