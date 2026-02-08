//! Memory - Long-term storage in Qdrant

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::services::HybridStrategy;

/// Memory type
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    #[default]
    Conversation,
    Learning,
    Fact,
    Expertise,
    Reflection,
}

/// Tag match mode for search filtering
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TagMatchMode {
    /// Match any of the specified tags (OR)
    #[default]
    Any,
    /// Match all of the specified tags (AND)
    All,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "conversation"),
            MemoryType::Learning => write!(f, "learning"),
            MemoryType::Fact => write!(f, "fact"),
            MemoryType::Expertise => write!(f, "expertise"),
            MemoryType::Reflection => write!(f, "reflection"),
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
    /// Custom tags for flexible categorization (e.g., ["code_knowledge", "rust", "orcs"])
    #[serde(default)]
    pub tags: Vec<String>,
    /// Hierarchical category path (e.g., "Rust > Concurrency > Async")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic_path: Option<String>,
    /// Extensible metadata for project-specific data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
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
    /// Custom tags for flexible categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Extensible metadata for project-specific data
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Search memories request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchMemoriesRequest {
    /// Query string for semantic search
    pub query: String,
    /// Maximum number of results (default: 10)
    pub limit: Option<usize>,
    /// Search strategy (default: auto)
    /// Options: auto, parallel, graph_first, rag_first, multi_hop,
    ///          single_rag, single_db, single_graph
    #[serde(default)]
    pub strategy: Option<HybridStrategy>,
    /// Multiple strategies to run and merge (overrides `strategy` when non-empty)
    /// Example: ["single_rag", "single_db"] runs RAG + DB and merges results
    #[serde(default)]
    pub strategies: Vec<HybridStrategy>,
    /// Filter by memory type (AND condition)
    pub memory_type: Option<MemoryType>,
    /// Filter by tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Tag matching mode: "any" (OR) or "all" (AND), default: any
    #[serde(default)]
    pub tags_match_mode: TagMatchMode,
    /// Minimum importance score (0.0 - 1.0)
    pub min_importance: Option<f32>,
    /// Context weights for boosting/excluding topics
    /// - weight > 0: boost (1.0 = full boost)
    /// - weight = 0: exclude
    ///
    /// Example: {"Rust": 1.0, "Finance": 0}
    #[serde(default)]
    pub context: HashMap<String, f32>,
}

/// Memory response
#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
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
            tags: mem.tags,
            topic_path: mem.topic_path,
            metadata: mem.metadata,
            similarity: None,
            created_at: mem.created_at,
        }
    }
}
