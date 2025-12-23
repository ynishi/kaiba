//! Dashboard DTOs - Status overview for a Rei

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Dashboard response - comprehensive Rei status overview
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardResponse {
    pub rei: DashboardReiInfo,
    pub state: DashboardState,
    pub activity: DashboardActivity,
    pub stats: DashboardStats,
    pub webhooks: DashboardWebhooks,
}

/// Basic Rei information for dashboard
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardReiInfo {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
}

/// Current state summary
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardState {
    pub energy_level: i32,
    pub mood: String,
    pub tokens_used: i32,
    pub token_budget: i32,
    pub energy_regen_per_hour: i32,
}

/// Activity timestamps
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardActivity {
    pub last_active_at: Option<DateTime<Utc>>,
    pub last_learn_at: Option<DateTime<Utc>>,
    pub last_digest_at: Option<DateTime<Utc>>,
}

/// Statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardStats {
    pub memory_count: u64,
    pub tei_count: i64,
}

/// Webhook delivery status
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardWebhooks {
    pub webhook_count: i64,
    pub last_delivery_at: Option<DateTime<Utc>>,
    pub recent_failures: i64,
}
