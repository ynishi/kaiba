//! Webhook DTOs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use kaiba::{DeliveryStatus, WebhookEventType};

/// Request to create a new webhook
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWebhookRequest {
    /// Human-readable name
    pub name: String,
    /// Target URL for webhook delivery
    pub url: String,
    /// Secret for HMAC-SHA256 signature (optional)
    pub secret: Option<String>,
    /// Event types to subscribe to (defaults to "all")
    #[serde(default)]
    pub events: Option<Vec<String>>,
    /// Custom headers to include
    #[serde(default)]
    pub headers: Option<serde_json::Value>,
    /// Maximum retry attempts (default: 3)
    pub max_retries: Option<i32>,
    /// Timeout in milliseconds (default: 30000)
    pub timeout_ms: Option<i32>,
    /// Payload format transformation (e.g., "github_issue")
    pub payload_format: Option<String>,
}

/// Request to update a webhook
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub enabled: Option<bool>,
    pub events: Option<Vec<String>>,
    pub headers: Option<serde_json::Value>,
    pub max_retries: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub payload_format: Option<String>,
}

/// Webhook response
#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookResponse {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub events: Vec<String>,
    pub max_retries: i32,
    pub timeout_ms: i32,
    pub payload_format: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Webhook delivery response
#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookDeliveryResponse {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event: String,
    pub status: String,
    pub status_code: Option<i32>,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Request to trigger a test webhook
#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerWebhookRequest {
    /// Event type to simulate
    pub event: Option<String>,
    /// Custom data to include in payload
    pub data: Option<serde_json::Value>,
}

impl WebhookResponse {
    pub fn from_domain(webhook: kaiba::ReiWebhook) -> Self {
        Self {
            id: webhook.id,
            rei_id: webhook.rei_id,
            name: webhook.name,
            url: webhook.url,
            enabled: webhook.enabled,
            events: webhook.events.iter().map(|e| e.to_string()).collect(),
            max_retries: webhook.max_retries,
            timeout_ms: webhook.timeout_ms,
            payload_format: webhook.payload_format,
            created_at: webhook.created_at,
            updated_at: webhook.updated_at,
        }
    }
}

impl WebhookDeliveryResponse {
    pub fn from_domain(delivery: kaiba::WebhookDelivery) -> Self {
        Self {
            id: delivery.id,
            webhook_id: delivery.webhook_id,
            event: delivery.payload.event.to_string(),
            status: match delivery.status {
                DeliveryStatus::Pending => "pending",
                DeliveryStatus::Success => "success",
                DeliveryStatus::Failed => "failed",
                DeliveryStatus::Retrying => "retrying",
            }
            .to_string(),
            status_code: delivery.status_code,
            attempts: delivery.attempts,
            created_at: delivery.created_at,
            completed_at: delivery.completed_at,
        }
    }
}

/// Parse event type strings to domain types
pub fn parse_event_types(events: Option<Vec<String>>) -> Vec<WebhookEventType> {
    events
        .map(|es| {
            es.into_iter()
                .map(|e| match e.as_str() {
                    "response_completed" => WebhookEventType::ResponseCompleted,
                    "state_changed" => WebhookEventType::StateChanged,
                    "memory_added" => WebhookEventType::MemoryAdded,
                    "search_completed" => WebhookEventType::SearchCompleted,
                    "learning_completed" => WebhookEventType::LearningCompleted,
                    "all" => WebhookEventType::All,
                    s if s.starts_with("custom:") => {
                        WebhookEventType::Custom(s.strip_prefix("custom:").unwrap().to_string())
                    }
                    s => WebhookEventType::Custom(s.to_string()),
                })
                .collect()
        })
        .unwrap_or_else(|| vec![WebhookEventType::All])
}
