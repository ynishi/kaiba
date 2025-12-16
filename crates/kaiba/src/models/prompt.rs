//! Prompt Generation Models
//!
//! Support for generating prompts in various formats for external Teis.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Prompt output format
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PromptFormat {
    /// Casting CLI format (system_prompt.txt compatible)
    Casting,
    /// Claude Code --system-prompt format
    ClaudeCode,
    /// Raw format with all components separated
    #[default]
    Raw,
}

impl std::str::FromStr for PromptFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "casting" => Ok(PromptFormat::Casting),
            "claude-code" | "claudecode" | "claude" => Ok(PromptFormat::ClaudeCode),
            "raw" => Ok(PromptFormat::Raw),
            _ => Err(format!("Unknown format: {}. Valid: casting, claude-code, raw", s)),
        }
    }
}

/// Query parameters for prompt endpoint
#[derive(Debug, Deserialize, IntoParams)]
pub struct PromptQuery {
    /// Output format (default: raw)
    #[serde(default)]
    pub format: Option<String>,
    /// Include memories via RAG (default: true)
    #[serde(default = "default_true")]
    pub include_memories: bool,
    /// Memory limit for RAG (default: 5)
    pub memory_limit: Option<usize>,
    /// Optional context/query for memory search
    pub context: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Prompt response - raw format with all components
#[derive(Debug, Serialize, ToSchema)]
pub struct PromptResponse {
    /// Generated system prompt (formatted according to requested format)
    pub system_prompt: String,
    /// Format used
    pub format: String,
    /// Rei summary
    pub rei: ReiSummary,
    /// Number of memories included
    pub memories_included: usize,
}

/// Rei summary for prompt response
#[derive(Debug, Serialize, ToSchema)]
pub struct ReiSummary {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub energy_level: i32,
    pub mood: String,
}
