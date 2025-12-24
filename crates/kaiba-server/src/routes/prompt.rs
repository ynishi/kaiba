//! Prompt Routes - Generate prompts for external Teis
//!
//! Instead of wrapping LLMs, Kaiba generates prompts that external
//! execution environments (Claude Code, Casting, etc.) can use directly.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use llm_toolkit::ToPrompt;
use serde::Serialize;
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
// Prompt DTOs - Type-safe prompt generation
// ============================================

/// Rei's identity information
#[derive(Serialize, ToPrompt)]
#[prompt(template = r#"Name: {{ name }}
Role: {{ role }}
Mood: {{ mood }}
Energy: {{ energy_level }}%"#)]
struct ReiIdentityDto {
    name: String,
    role: String,
    mood: String,
    energy_level: i32,
}

impl ReiIdentityDto {
    fn from_rei(rei: &Rei, state: &ReiState) -> Self {
        Self {
            name: rei.name.clone(),
            role: rei.role.clone(),
            mood: state.mood.clone(),
            energy_level: state.energy_level,
        }
    }
}

/// Rei's manifest information (personality, instructions, quirks)
#[derive(Serialize, ToPrompt)]
struct ReiManifestDto {
    personality: Option<String>,
    instructions: Option<String>,
    quirks: Option<String>,
}

impl ReiManifestDto {
    fn from_rei(rei: &Rei) -> Self {
        let manifest = rei.manifest.as_object();
        Self {
            personality: manifest
                .and_then(|m| m.get("personality"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            instructions: manifest
                .and_then(|m| m.get("instructions"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            quirks: manifest
                .and_then(|m| m.get("quirks"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }

    #[allow(dead_code)] // Used in template strings
    fn is_empty(&self) -> bool {
        self.personality.is_none() && self.instructions.is_none() && self.quirks.is_none()
    }
}

/// Single memory entry
#[derive(Serialize, ToPrompt)]
#[prompt(template = "[{{ memory_type }}] {{ content }} (created: {{ created_at }}, importance: {{ importance }})")]
struct MemoryDto {
    memory_type: String,
    content: String,
    created_at: String,
    importance: f32,
}

impl From<&Memory> for MemoryDto {
    fn from(mem: &Memory) -> Self {
        Self {
            memory_type: mem.memory_type.to_string(),
            content: mem.content.clone(),
            created_at: mem.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            importance: mem.importance,
        }
    }
}

/// Casting format prompt (system_prompt.txt compatible)
#[derive(Serialize, ToPrompt)]
#[prompt(template = r#"YOU ARE a Persona named "{{ rei_name }}" who embodies the role of {{ rei_role }}.

Your role is to:
- Embody this persona as a helpful, knowledgeable character
- Support users in a friendly and approachable way
- Respond authentically based on your personality and memories

## Identity
- Name: {{ rei_name }}
- Role: {{ rei_role }}
- Mood: {{ mood }}
- Energy: {{ energy_level }}%
{% if personality %}

## Personality
{{ personality }}{% endif %}
{% if instructions %}

## Special Instructions
{{ instructions }}{% endif %}
{% if quirks %}

## Quirks & Mannerisms
{{ quirks }}{% endif %}
{% if has_memories %}

## Your Memories
{% for mem in memories %}
- {{ mem }}
{% endfor %}{% endif %}

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

Use search to recall past conversations, projects, or learnings that aren't in the initial context."#)]
struct CastingPromptDto {
    rei_name: String,
    rei_role: String,
    mood: String,
    energy_level: i32,
    personality: Option<String>,
    instructions: Option<String>,
    quirks: Option<String>,
    memories: Vec<String>,
    has_memories: bool,
}

/// Claude Code --system-prompt format
#[derive(Serialize, ToPrompt)]
#[prompt(template = r#"You are {{ rei_name }}, {{ rei_role }}.

Current state: {{ mood }} (Energy: {{ energy_level }}%)
{% if personality %}

Personality: {{ personality }}{% endif %}
{% if instructions %}

{{ instructions }}{% endif %}
{% if has_memories %}

## Context from Memory
{% for mem in memories %}
- {{ mem }}
{% endfor %}{% endif %}

## Memory
- Search: `kaiba memory search "<query>"` (not all memories are in this prompt)
- Save: `kaiba memory add -t <type> "<content>"`
Types: learning, fact, expertise, reflection"#)]
struct ClaudeCodePromptDto {
    rei_name: String,
    rei_role: String,
    mood: String,
    energy_level: i32,
    personality: Option<String>,
    instructions: Option<String>,
    memories: Vec<String>,
    has_memories: bool,
}

/// Raw format with clear sections
#[derive(Serialize, ToPrompt)]
#[prompt(template = r#"=== IDENTITY ===
Name: {{ rei_name }}
Role: {{ rei_role }}
Mood: {{ mood }}
Energy: {{ energy_level }}%

=== MANIFEST ===
{{ manifest_json }}
{% if has_memories %}

=== MEMORIES ===
{% for mem in memories %}
{{ mem }}
{% endfor %}{% endif %}"#)]
struct RawPromptDto {
    rei_name: String,
    rei_role: String,
    mood: String,
    energy_level: i32,
    manifest_json: String,
    memories: Vec<String>,
    has_memories: bool,
}

/// LLM Call system prompt (used in /kaiba/rei/{id}/call endpoint)
#[derive(Serialize, ToPrompt)]
#[prompt(template = r#"You are {{ rei_name }}, {{ rei_role }}.
{% if personality %}

Personality: {{ personality }}{% endif %}
{% if instructions %}

Instructions: {{ instructions }}{% endif %}
{% if has_memories %}

## Relevant Memories
Use the following memories as context for your response:

{% for mem in memories %}
- {{ mem }}
{% endfor %}{% endif %}"#)]
pub(crate) struct CallPromptDto {
    rei_name: String,
    rei_role: String,
    personality: Option<String>,
    instructions: Option<String>,
    memories: Vec<String>,
    has_memories: bool,
}

impl CallPromptDto {
    pub(crate) fn new(rei: &Rei, memories: &[Memory]) -> Self {
        let manifest = ReiManifestDto::from_rei(rei);
        let memory_strs: Vec<String> = memories.iter().map(|m| MemoryDto::from(m).to_prompt()).collect();
        let has_memories = !memories.is_empty();

        Self {
            rei_name: rei.name.clone(),
            rei_role: rei.role.clone(),
            personality: manifest.personality,
            instructions: manifest.instructions,
            memories: memory_strs,
            has_memories,
        }
    }
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

/// Generate prompt in the requested format using ToPrompt DTOs
fn format_prompt(rei: &Rei, state: &ReiState, memories: &[Memory], format: PromptFormat) -> String {
    let manifest = ReiManifestDto::from_rei(rei);
    let memory_strs: Vec<String> = memories.iter().map(|m| MemoryDto::from(m).to_prompt()).collect();
    let has_memories = !memories.is_empty();

    match format {
        PromptFormat::Casting => {
            let dto = CastingPromptDto {
                rei_name: rei.name.clone(),
                rei_role: rei.role.clone(),
                mood: state.mood.clone(),
                energy_level: state.energy_level,
                personality: manifest.personality,
                instructions: manifest.instructions,
                quirks: manifest.quirks,
                memories: memory_strs,
                has_memories,
            };
            dto.to_prompt()
        }
        PromptFormat::ClaudeCode => {
            let dto = ClaudeCodePromptDto {
                rei_name: rei.name.clone(),
                rei_role: rei.role.clone(),
                mood: state.mood.clone(),
                energy_level: state.energy_level,
                personality: manifest.personality,
                instructions: manifest.instructions,
                memories: memory_strs,
                has_memories,
            };
            dto.to_prompt()
        }
        PromptFormat::Raw => {
            let manifest_json = serde_json::to_string_pretty(&rei.manifest).unwrap_or_default();
            let dto = RawPromptDto {
                rei_name: rei.name.clone(),
                rei_role: rei.role.clone(),
                mood: state.mood.clone(),
                energy_level: state.energy_level,
                manifest_json,
                memories: memory_strs,
                has_memories,
            };
            dto.to_prompt()
        }
    }
}

/// DEPRECATED: Casting CLI format (system_prompt.txt compatible)
/// Replaced by ToPrompt template (rei_casting.jinja)
#[allow(dead_code)]
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
        prompt.push_str("\n## Your Memories\n");
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

/// DEPRECATED: Claude Code --system-prompt format
/// Replaced by ToPrompt template (rei_claude_code.jinja)
#[allow(dead_code)]
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
            prompt.push_str(&format!(
                "- [{}] {} (created: {}, importance: {:.2})\n",
                mem.memory_type,
                mem.content,
                mem.created_at.format("%Y-%m-%d %H:%M UTC"),
                mem.importance
            ));
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

/// DEPRECATED: Raw format with clear sections
/// Replaced by ToPrompt template (rei_raw.jinja)
#[allow(dead_code)]
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
                "[{}] (created: {}, importance: {:.2}) {}\n",
                mem.memory_type,
                mem.created_at.format("%Y-%m-%d %H:%M UTC"),
                mem.importance,
                mem.content
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
        ..Default::default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use llm_toolkit::ToPrompt;
    use serde_json::json;

    fn sample_rei() -> Rei {
        Rei {
            id: Uuid::new_v4(),
            name: "TestRei".to_string(),
            role: "Test Assistant".to_string(),
            avatar_url: None,
            manifest: json!({
                "personality": "Friendly and helpful",
                "instructions": "Always be supportive",
                "quirks": "Uses emojis"
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_rei_state() -> ReiState {
        ReiState {
            id: Uuid::new_v4(),
            rei_id: Uuid::new_v4(),
            token_budget: 1000,
            tokens_used: 0,
            energy_level: 80,
            mood: "cheerful".to_string(),
            last_active_at: Some(Utc::now()),
            updated_at: Utc::now(),
            energy_regen_per_hour: 10,
            last_digest_at: None,
            last_learn_at: None,
        }
    }

    fn sample_memory() -> Memory {
        Memory {
            id: "test_memory".to_string(),
            rei_id: "test_rei".to_string(),
            content: "This is a test memory".to_string(),
            memory_type: crate::models::MemoryType::Learning,
            importance: 0.8,
            tags: vec!["test".to_string()],
            metadata: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_rei_identity_dto_to_prompt() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let dto = ReiIdentityDto::from_rei(&rei, &state);

        let prompt = dto.to_prompt();

        assert!(prompt.contains("Name: TestRei"));
        assert!(prompt.contains("Role: Test Assistant"));
        assert!(prompt.contains("Mood: cheerful"));
        assert!(prompt.contains("Energy: 80%"));
    }

    #[test]
    fn test_memory_dto_to_prompt() {
        let memory = sample_memory();
        let dto = MemoryDto::from(&memory);

        let prompt = dto.to_prompt();

        assert!(prompt.contains("[learning]"));
        assert!(prompt.contains("This is a test memory"));
        assert!(prompt.contains("importance: 0.8"));
    }

    #[test]
    fn test_casting_prompt_dto() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let manifest = ReiManifestDto::from_rei(&rei);
        let memory_strs: Vec<String> = memories.iter().map(|m| MemoryDto::from(m).to_prompt()).collect();

        let dto = CastingPromptDto {
            rei_name: rei.name.clone(),
            rei_role: rei.role.clone(),
            mood: state.mood.clone(),
            energy_level: state.energy_level,
            personality: manifest.personality,
            instructions: manifest.instructions,
            quirks: manifest.quirks,
            memories: memory_strs,
            has_memories: true,
        };

        let prompt = dto.to_prompt();

        // Check core structure
        assert!(prompt.contains("YOU ARE a Persona named \"TestRei\""));
        assert!(prompt.contains("Test Assistant"));

        // Check identity section
        assert!(prompt.contains("## Identity"));

        // Check manifest sections
        assert!(prompt.contains("## Personality"));
        assert!(prompt.contains("Friendly and helpful"));
        assert!(prompt.contains("## Special Instructions"));
        assert!(prompt.contains("Always be supportive"));
        assert!(prompt.contains("## Quirks & Mannerisms"));
        assert!(prompt.contains("Uses emojis"));

        // Check memories section
        assert!(prompt.contains("## Your Memories"));
        assert!(prompt.contains("This is a test memory"));

        // Check memory management instructions
        assert!(prompt.contains("## Memory Management"));
        assert!(prompt.contains("kaiba memory search"));
    }

    #[test]
    fn test_claude_code_prompt_dto() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let manifest = ReiManifestDto::from_rei(&rei);
        let memory_strs: Vec<String> = memories.iter().map(|m| MemoryDto::from(m).to_prompt()).collect();

        let dto = ClaudeCodePromptDto {
            rei_name: rei.name.clone(),
            rei_role: rei.role.clone(),
            mood: state.mood.clone(),
            energy_level: state.energy_level,
            personality: manifest.personality,
            instructions: manifest.instructions,
            memories: memory_strs,
            has_memories: true,
        };

        let prompt = dto.to_prompt();

        // Check core structure
        assert!(prompt.contains("You are TestRei, Test Assistant"));
        assert!(prompt.contains("Current state: cheerful (Energy: 80%)"));

        // Check personality
        assert!(prompt.contains("Personality: Friendly and helpful"));

        // Check instructions
        assert!(prompt.contains("Always be supportive"));

        // Check memories
        assert!(prompt.contains("## Context from Memory"));
        assert!(prompt.contains("This is a test memory"));

        // Check memory commands
        assert!(prompt.contains("## Memory"));
        assert!(prompt.contains("kaiba memory search"));
    }

    #[test]
    fn test_raw_prompt_dto() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let manifest_json = serde_json::to_string_pretty(&rei.manifest).unwrap();
        let memory_strs: Vec<String> = memories.iter().map(|m| MemoryDto::from(m).to_prompt()).collect();

        let dto = RawPromptDto {
            rei_name: rei.name.clone(),
            rei_role: rei.role.clone(),
            mood: state.mood.clone(),
            energy_level: state.energy_level,
            manifest_json,
            memories: memory_strs,
            has_memories: true,
        };

        let prompt = dto.to_prompt();

        // Check sections
        assert!(prompt.contains("=== IDENTITY ==="));
        assert!(prompt.contains("=== MANIFEST ==="));
        assert!(prompt.contains("=== MEMORIES ==="));

        // Check identity
        assert!(prompt.contains("TestRei"));
        assert!(prompt.contains("cheerful"));

        // Check manifest JSON
        assert!(prompt.contains("personality"));
        assert!(prompt.contains("Friendly and helpful"));

        // Check memories
        assert!(prompt.contains("This is a test memory"));
    }

    #[test]
    fn test_call_prompt_dto() {
        let rei = sample_rei();
        let memories = vec![sample_memory()];

        let dto = CallPromptDto::new(&rei, &memories);
        let prompt = dto.to_prompt();

        // Check core structure
        assert!(prompt.contains("You are TestRei, Test Assistant"));

        // Check personality
        assert!(prompt.contains("Personality: Friendly and helpful"));

        // Check instructions
        assert!(prompt.contains("Instructions: Always be supportive"));

        // Check memories
        assert!(prompt.contains("## Relevant Memories"));
        assert!(prompt.contains("This is a test memory"));
    }

    #[test]
    fn test_format_prompt_casting() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let prompt = format_prompt(&rei, &state, &memories, PromptFormat::Casting);

        assert!(prompt.contains("YOU ARE a Persona"));
        assert!(prompt.contains("TestRei"));
        assert!(prompt.contains("## Memory Management"));
    }

    #[test]
    fn test_format_prompt_claude_code() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let prompt = format_prompt(&rei, &state, &memories, PromptFormat::ClaudeCode);

        assert!(prompt.contains("You are TestRei"));
        assert!(prompt.contains("Current state: cheerful"));
    }

    #[test]
    fn test_format_prompt_raw() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories = vec![sample_memory()];

        let prompt = format_prompt(&rei, &state, &memories, PromptFormat::Raw);

        assert!(prompt.contains("=== IDENTITY ==="));
        assert!(prompt.contains("=== MANIFEST ==="));
    }

    #[test]
    fn test_empty_memories() {
        let rei = sample_rei();
        let state = sample_rei_state();
        let memories: Vec<Memory> = vec![];

        let prompt = format_prompt(&rei, &state, &memories, PromptFormat::Casting);

        // Should not contain memories section when empty
        assert!(!prompt.contains("## Your Memories\n-"));
    }

    #[test]
    fn test_empty_manifest() {
        let mut rei = sample_rei();
        rei.manifest = json!({});
        let state = sample_rei_state();
        let memories: Vec<Memory> = vec![];

        let prompt = format_prompt(&rei, &state, &memories, PromptFormat::Casting);

        // Should still generate valid prompt without manifest sections
        assert!(prompt.contains("YOU ARE a Persona"));
        assert!(!prompt.contains("## Personality"));
    }
}
