//! Memory - Long-term storage in Qdrant

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Memory type
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    #[default]
    Conversation,
    Learning,
    Fact,
    Expertise,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "conversation"),
            MemoryType::Learning => write!(f, "learning"),
            MemoryType::Fact => write!(f, "fact"),
            MemoryType::Expertise => write!(f, "expertise"),
        }
    }
}

/// Memory entry (stored in Qdrant)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Memory {
    pub id: String,
    pub rei_id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
}

// ============================================
// Request/Response DTOs
// ============================================

/// Create memory request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMemoryRequest {
    pub content: String,
    #[serde(default)]
    pub memory_type: MemoryType,
    pub importance: Option<f32>,
}

/// Search memories request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchMemoriesRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub memory_type: Option<MemoryType>,
}

/// Memory response
#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub similarity: Option<f32>,
    pub created_at: DateTime<Utc>,
}

impl From<Memory> for MemoryResponse {
    fn from(mem: Memory) -> Self {
        Self {
            id: mem.id,
            content: mem.content,
            memory_type: mem.memory_type,
            importance: mem.importance,
            similarity: None,
            created_at: mem.created_at,
        }
    }
}
