//! Call - LLM Invocation Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Task health status (from llm-toolkit)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskHealth {
    #[default]
    OnTrack,
    AtRisk,
    OffTrack,
}

/// Call log entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CallLog {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub tei_id: Uuid,
    pub message: String,
    pub response: String,
    pub tokens_consumed: i32,
    pub context: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// ============================================
// Request/Response DTOs
// ============================================

/// Call context for LLM invocation
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CallContext {
    pub task_type: Option<String>,
    pub task_health: Option<TaskHealth>,
    #[serde(default)]
    pub include_memories: bool,
    pub memory_limit: Option<usize>,
}

/// Call request
#[derive(Debug, Deserialize)]
pub struct CallRequest {
    pub tei_ids: Vec<Uuid>,
    pub message: String,
    pub context: Option<CallContext>,
}

/// Memory reference in response
#[derive(Debug, Serialize)]
pub struct MemoryReference {
    pub id: String,
    pub similarity: f32,
}

/// Call response
#[derive(Debug, Serialize)]
pub struct CallResponse {
    pub response: String,
    pub tei_used: Uuid,
    pub tokens_consumed: i32,
    pub memories_included: Vec<MemoryReference>,
}
