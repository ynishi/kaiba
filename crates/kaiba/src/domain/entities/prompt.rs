//! Prompt - Prompt Templates for Tei
//!
//! Pure domain entity without infrastructure dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Prompt - A prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub name: String,
    pub template: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Prompt {
    /// Create a new prompt template
    pub fn new(rei_id: Uuid, name: String, template: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            rei_id,
            name,
            template,
            description,
            created_at: now,
            updated_at: now,
        }
    }
}
