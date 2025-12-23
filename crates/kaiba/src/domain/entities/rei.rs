//! Rei (霊) - Persistent Persona Identity
//!
//! Pure domain entity without infrastructure dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Rei - Core persona identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rei {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub manifest: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rei State - Current energy, mood, resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReiState {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub token_budget: i32,
    pub tokens_used: i32,
    pub energy_level: i32,
    pub mood: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    /// Energy regeneration per hour (0 = disabled)
    pub energy_regen_per_hour: i32,
    /// Last time Digest was completed (for filtering already-digested memories)
    pub last_digest_at: Option<DateTime<Utc>>,
    /// Last time Learn was completed (for dashboard)
    pub last_learn_at: Option<DateTime<Utc>>,
}

impl Rei {
    /// Create a new Rei with generated ID and timestamps
    pub fn new(
        name: String,
        role: String,
        avatar_url: Option<String>,
        manifest: Option<serde_json::Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            role,
            avatar_url,
            manifest: manifest.unwrap_or(serde_json::json!({})),
            created_at: now,
            updated_at: now,
        }
    }
}

impl ReiState {
    /// Create default state for a Rei
    pub fn new_for_rei(rei_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            rei_id,
            token_budget: 100000,
            tokens_used: 0,
            energy_level: 100,
            mood: "neutral".to_string(),
            last_active_at: None,
            updated_at: Utc::now(),
            energy_regen_per_hour: 10,
            last_digest_at: None,
            last_learn_at: None,
        }
    }

    /// Default state values (for fallback)
    pub fn default_values() -> Self {
        Self {
            id: Uuid::nil(),
            rei_id: Uuid::nil(),
            token_budget: 100000,
            tokens_used: 0,
            energy_level: 100,
            mood: "neutral".to_string(),
            last_active_at: None,
            updated_at: Utc::now(),
            energy_regen_per_hour: 10,
            last_digest_at: None,
            last_learn_at: None,
        }
    }
}
