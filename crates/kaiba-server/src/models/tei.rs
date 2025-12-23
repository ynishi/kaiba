//! Tei (体) - Execution Interface with Expertise

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

/// LLM Provider
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Google,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::Anthropic => write!(f, "anthropic"),
            Provider::OpenAI => write!(f, "openai"),
            Provider::Google => write!(f, "google"),
        }
    }
}

impl std::str::FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(Provider::Anthropic),
            "openai" => Ok(Provider::OpenAI),
            "google" => Ok(Provider::Google),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// Tei - Execution interface with LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Tei {
    pub id: Uuid,
    pub name: String,
    pub provider: String, // Stored as string in DB
    pub model_id: String,
    pub is_fallback: bool,
    pub priority: i32,
    pub config: serde_json::Value,
    pub expertise: Option<serde_json::Value>, // Serialized Expertise
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Tei {
    /// Get provider as enum
    #[allow(dead_code)]
    pub fn provider_enum(&self) -> Result<Provider, String> {
        self.provider.parse()
    }
}

/// Rei-Tei association
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ReiTei {
    pub rei_id: Uuid,
    pub tei_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ============================================
// Request/Response DTOs
// ============================================

/// Create Tei request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTeiRequest {
    pub name: String,
    pub provider: Provider,
    pub model_id: String,
    #[serde(default)]
    pub is_fallback: bool,
    #[serde(default)]
    pub priority: i32,
    pub config: Option<serde_json::Value>,
    pub expertise: Option<serde_json::Value>,
}

/// Update Tei request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTeiRequest {
    pub name: Option<String>,
    pub provider: Option<Provider>,
    pub model_id: Option<String>,
    pub is_fallback: Option<bool>,
    pub priority: Option<i32>,
    pub config: Option<serde_json::Value>,
    pub expertise: Option<serde_json::Value>,
}

/// Tei response
#[derive(Debug, Serialize, ToSchema)]
pub struct TeiResponse {
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

impl From<Tei> for TeiResponse {
    fn from(tei: Tei) -> Self {
        Self {
            id: tei.id,
            name: tei.name,
            provider: tei.provider,
            model_id: tei.model_id,
            is_fallback: tei.is_fallback,
            priority: tei.priority,
            config: tei.config,
            expertise: tei.expertise,
            created_at: tei.created_at,
            updated_at: tei.updated_at,
        }
    }
}

/// Associate Tei to Rei request
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssociateTeiRequest {
    pub tei_id: Uuid,
}
