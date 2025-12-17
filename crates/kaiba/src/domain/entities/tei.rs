//! Tei (体) - Execution Interface with Expertise
//!
//! Pure domain entity without infrastructure dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_objects::Provider;

/// Tei - Execution interface with LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tei {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub model_id: String,
    pub is_fallback: bool,
    pub priority: i32,
    pub config: serde_json::Value,
    pub expertise: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rei-Tei association
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReiTei {
    pub rei_id: Uuid,
    pub tei_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Tei {
    /// Create a new Tei with generated ID and timestamps
    pub fn new(
        name: String,
        provider: Provider,
        model_id: String,
        is_fallback: bool,
        priority: i32,
        config: Option<serde_json::Value>,
        expertise: Option<serde_json::Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            provider: provider.to_string(),
            model_id,
            is_fallback,
            priority,
            config: config.unwrap_or(serde_json::json!({})),
            expertise,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get provider as enum
    pub fn provider_enum(&self) -> Result<Provider, String> {
        self.provider.parse()
    }
}

impl ReiTei {
    /// Create a new association
    pub fn new(rei_id: Uuid, tei_id: Uuid) -> Self {
        Self {
            rei_id,
            tei_id,
            created_at: Utc::now(),
        }
    }
}
