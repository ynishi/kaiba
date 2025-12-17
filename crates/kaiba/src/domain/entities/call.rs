//! Call - LLM Invocation Record
//!
//! Pure domain entity without infrastructure dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Call - Record of an LLM invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub tei_id: Uuid,
    pub prompt: String,
    pub response: Option<String>,
    pub tokens_used: Option<i32>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Call {
    /// Create a new call record
    pub fn new(rei_id: Uuid, tei_id: Uuid, prompt: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            rei_id,
            tei_id,
            prompt,
            response: None,
            tokens_used: None,
            status: "pending".to_string(),
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark call as completed with response
    pub fn complete(&mut self, response: String, tokens_used: i32) {
        self.response = Some(response);
        self.tokens_used = Some(tokens_used);
        self.status = "completed".to_string();
        self.completed_at = Some(Utc::now());
    }

    /// Mark call as failed
    pub fn fail(&mut self, error: String) {
        self.response = Some(error);
        self.status = "failed".to_string();
        self.completed_at = Some(Utc::now());
    }
}
