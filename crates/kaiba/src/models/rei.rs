//! Rei (霊) - Persistent Persona Identity

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Rei - Core persona identity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReiState {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub token_budget: i32,
    pub tokens_used: i32,
    pub energy_level: i32,
    pub mood: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

// ============================================
// Request/Response DTOs
// ============================================

/// Create Rei request
#[derive(Debug, Deserialize)]
pub struct CreateReiRequest {
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub manifest: Option<serde_json::Value>,
}

/// Update Rei request
#[derive(Debug, Deserialize)]
pub struct UpdateReiRequest {
    pub name: Option<String>,
    pub role: Option<String>,
    pub avatar_url: Option<String>,
    pub manifest: Option<serde_json::Value>,
}

/// Rei response with state
#[derive(Debug, Serialize)]
pub struct ReiResponse {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub manifest: serde_json::Value,
    pub state: ReiStateResponse,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rei state response
#[derive(Debug, Serialize)]
pub struct ReiStateResponse {
    pub energy_level: i32,
    pub mood: String,
    pub token_budget: i32,
    pub tokens_used: i32,
    pub last_active_at: Option<DateTime<Utc>>,
}

/// Update Rei state request
#[derive(Debug, Deserialize)]
pub struct UpdateReiStateRequest {
    pub energy_level: Option<i32>,
    pub mood: Option<String>,
    pub token_budget: Option<i32>,
    pub tokens_used: Option<i32>,
}

impl From<ReiState> for ReiStateResponse {
    fn from(state: ReiState) -> Self {
        Self {
            energy_level: state.energy_level,
            mood: state.mood,
            token_budget: state.token_budget,
            tokens_used: state.tokens_used,
            last_active_at: state.last_active_at,
        }
    }
}
