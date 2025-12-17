//! Memory - Long-term storage for Rei
//!
//! Pure domain entity without infrastructure dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_objects::MemoryType;

/// Memory - A piece of stored knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier for the memory
    pub id: String,
    /// The Rei this memory belongs to
    pub rei_id: String,
    /// The content/text of the memory
    pub content: String,
    /// Type of memory (episodic, semantic, reflection, etc.)
    pub memory_type: MemoryType,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// When this memory was created
    pub created_at: DateTime<Utc>,
}

impl Memory {
    /// Create a new memory with generated ID and timestamp
    pub fn new(
        rei_id: String,
        content: String,
        memory_type: MemoryType,
        importance: f32,
        tags: Vec<String>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            rei_id,
            content,
            memory_type,
            importance,
            tags,
            metadata,
            created_at: Utc::now(),
        }
    }
}
