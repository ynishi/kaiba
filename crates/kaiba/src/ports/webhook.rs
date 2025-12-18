//! Webhook Delivery Port
//!
//! Abstract interface for delivering webhooks to external endpoints.
//! This port enables Rei to perform outbound actions to the external world.

use async_trait::async_trait;

use crate::domain::entities::{ReiWebhook, WebhookDelivery, WebhookPayload};
use crate::domain::errors::DomainError;

/// Webhook delivery interface
///
/// This trait abstracts the HTTP delivery mechanism for webhooks.
/// Implementations handle the actual HTTP requests, retries, and
/// signature generation.
///
/// # Example
///
/// ```rust,ignore
/// use kaiba::ports::TeiWebhook;
///
/// struct HttpWebhook { /* reqwest client */ }
///
/// #[async_trait]
/// impl TeiWebhook for HttpWebhook {
///     async fn deliver(&self, webhook: &ReiWebhook, payload: &WebhookPayload)
///         -> Result<WebhookDelivery, DomainError>
///     {
///         // Send HTTP POST with HMAC signature
///     }
/// }
/// ```
#[async_trait]
pub trait TeiWebhook: Send + Sync {
    /// Deliver a payload to a webhook endpoint
    ///
    /// Sends the payload to the webhook's URL with appropriate
    /// headers and optional HMAC-SHA256 signature.
    ///
    /// # Arguments
    /// * `webhook` - The webhook configuration
    /// * `payload` - The event payload to deliver
    ///
    /// # Returns
    /// A `WebhookDelivery` record with the delivery status
    async fn deliver(
        &self,
        webhook: &ReiWebhook,
        payload: &WebhookPayload,
    ) -> Result<WebhookDelivery, DomainError>;

    /// Deliver with automatic retry on failure
    ///
    /// Attempts delivery up to `webhook.max_retries` times
    /// with exponential backoff between attempts.
    async fn deliver_with_retry(
        &self,
        webhook: &ReiWebhook,
        payload: &WebhookPayload,
    ) -> Result<WebhookDelivery, DomainError> {
        // Default implementation: single attempt
        self.deliver(webhook, payload).await
    }

    /// Verify webhook endpoint is reachable
    ///
    /// Performs a health check on the webhook URL.
    /// Returns `true` if the endpoint responds successfully.
    async fn verify_endpoint(&self, url: &str) -> Result<bool, DomainError>;

    /// Generate HMAC-SHA256 signature for a payload
    ///
    /// Creates a signature string that can be used to verify
    /// the authenticity of webhook deliveries.
    fn sign_payload(&self, secret: &str, payload: &[u8]) -> String;
}

/// Configuration for webhook delivery behavior
#[derive(Debug, Clone)]
pub struct WebhookDeliveryConfig {
    /// Base delay for exponential backoff (milliseconds)
    pub retry_base_delay_ms: u64,
    /// Maximum delay between retries (milliseconds)
    pub retry_max_delay_ms: u64,
    /// User-Agent header value
    pub user_agent: String,
}

impl Default for WebhookDeliveryConfig {
    fn default() -> Self {
        Self {
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            user_agent: "Kaiba-Webhook/1.0".to_string(),
        }
    }
}
