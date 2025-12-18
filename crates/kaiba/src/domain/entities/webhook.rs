//! ReiWebhook - Outbound Webhook for Rei Actions
//!
//! Enables Rei to interact with the external world by sending
//! HTTP webhook requests. This is one of the three pillars of
//! Rei's autonomous capabilities:
//! 1. Knowledge acquisition (Memory + WebSearch)
//! 2. Self-reflection (ReiState)
//! 3. External actions (Webhook)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Webhook configuration for a Rei
///
/// Represents an outbound webhook endpoint that Rei can use
/// to send notifications and trigger external actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReiWebhook {
    pub id: Uuid,
    pub rei_id: Uuid,
    /// Human-readable name for this webhook
    pub name: String,
    /// Target URL for webhook delivery
    pub url: String,
    /// Secret for HMAC-SHA256 signature (optional)
    pub secret: Option<String>,
    /// Whether this webhook is active
    pub enabled: bool,
    /// Event types this webhook subscribes to
    pub events: Vec<WebhookEventType>,
    /// Custom headers to include (e.g., Authorization)
    #[serde(default)]
    pub headers: serde_json::Value,
    /// Retry configuration
    pub max_retries: i32,
    /// Timeout in milliseconds
    pub timeout_ms: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Types of events that can trigger webhooks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Rei completed a thought/response
    ResponseCompleted,
    /// Rei's state changed (mood, energy)
    StateChanged,
    /// Memory was added
    MemoryAdded,
    /// Web search was performed
    SearchCompleted,
    /// Custom event (user-defined)
    Custom(String),
    /// All events
    All,
}

/// Payload sent to webhook endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Unique ID for this delivery
    pub delivery_id: Uuid,
    /// Event type that triggered this webhook
    pub event: WebhookEventType,
    /// Rei that triggered the event
    pub rei_id: Uuid,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event-specific data
    pub data: serde_json::Value,
}

/// Result of a webhook delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub payload: WebhookPayload,
    pub status: DeliveryStatus,
    pub status_code: Option<i32>,
    pub response_body: Option<String>,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
    Retrying,
}

impl ReiWebhook {
    /// Create a new webhook with sensible defaults
    pub fn new(rei_id: Uuid, name: String, url: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            rei_id,
            name,
            url,
            secret: None,
            enabled: true,
            events: vec![WebhookEventType::All],
            headers: serde_json::json!({}),
            max_retries: 3,
            timeout_ms: 30000,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create with a signing secret for HMAC-SHA256 verification
    pub fn with_secret(mut self, secret: String) -> Self {
        self.secret = Some(secret);
        self
    }

    /// Set specific event types to subscribe to
    pub fn with_events(mut self, events: Vec<WebhookEventType>) -> Self {
        self.events = events;
        self
    }

    /// Add custom headers
    pub fn with_headers(mut self, headers: serde_json::Value) -> Self {
        self.headers = headers;
        self
    }

    /// Check if this webhook should receive a given event type
    pub fn should_receive(&self, event: &WebhookEventType) -> bool {
        if !self.enabled {
            return false;
        }
        self.events.contains(&WebhookEventType::All) || self.events.contains(event)
    }
}

impl WebhookPayload {
    /// Create a new payload for an event
    pub fn new(event: WebhookEventType, rei_id: Uuid, data: serde_json::Value) -> Self {
        Self {
            delivery_id: Uuid::new_v4(),
            event,
            rei_id,
            timestamp: Utc::now(),
            data,
        }
    }
}

impl WebhookDelivery {
    /// Create a new pending delivery
    pub fn new(webhook_id: Uuid, payload: WebhookPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            webhook_id,
            payload,
            status: DeliveryStatus::Pending,
            status_code: None,
            response_body: None,
            attempts: 0,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark as successful
    pub fn success(mut self, status_code: i32, response_body: Option<String>) -> Self {
        self.status = DeliveryStatus::Success;
        self.status_code = Some(status_code);
        self.response_body = response_body;
        self.completed_at = Some(Utc::now());
        self.attempts += 1;
        self
    }

    /// Mark as failed
    pub fn failed(mut self, status_code: Option<i32>, error: String) -> Self {
        self.status = DeliveryStatus::Failed;
        self.status_code = status_code;
        self.response_body = Some(error);
        self.completed_at = Some(Utc::now());
        self.attempts += 1;
        self
    }

    /// Mark as retrying
    pub fn retry(mut self) -> Self {
        self.status = DeliveryStatus::Retrying;
        self.attempts += 1;
        self
    }
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResponseCompleted => write!(f, "response_completed"),
            Self::StateChanged => write!(f, "state_changed"),
            Self::MemoryAdded => write!(f, "memory_added"),
            Self::SearchCompleted => write!(f, "search_completed"),
            Self::Custom(name) => write!(f, "custom:{}", name),
            Self::All => write!(f, "all"),
        }
    }
}
