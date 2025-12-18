//! HTTP Webhook Implementation
//!
//! Delivers webhooks to external endpoints using reqwest.

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use kaiba::{
    DeliveryStatus, DomainError, ReiWebhook, TeiWebhook, WebhookDelivery, WebhookDeliveryConfig,
    WebhookPayload,
};

/// HTTP implementation of TeiWebhook
pub struct HttpWebhook {
    client: Client,
    config: WebhookDeliveryConfig,
}

impl HttpWebhook {
    pub fn new() -> Self {
        Self::with_config(WebhookDeliveryConfig::default())
    }

    pub fn with_config(config: WebhookDeliveryConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(&config.user_agent)
            .build()
            .expect("Failed to build HTTP client");

        Self { client, config }
    }
}

impl Default for HttpWebhook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TeiWebhook for HttpWebhook {
    async fn deliver(
        &self,
        webhook: &ReiWebhook,
        payload: &WebhookPayload,
    ) -> Result<WebhookDelivery, DomainError> {
        let mut delivery = WebhookDelivery::new(webhook.id, payload.clone());

        // Serialize payload
        let body = serde_json::to_vec(payload).map_err(|e| {
            DomainError::ExternalService(format!("Failed to serialize payload: {e}"))
        })?;

        // Build request
        let mut request = self
            .client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .timeout(Duration::from_millis(webhook.timeout_ms as u64));

        // Add signature if secret is configured
        if let Some(secret) = &webhook.secret {
            let signature = self.sign_payload(secret, &body);
            request = request.header("X-Kaiba-Signature", signature);
        }

        // Add custom headers
        if let Some(headers) = webhook.headers.as_object() {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        // Send request
        let response = request.body(body).send().await;

        match response {
            Ok(resp) => {
                let status_code = resp.status().as_u16() as i32;
                let response_body = resp.text().await.ok();

                if (200..300).contains(&status_code) {
                    delivery = delivery.success(status_code, response_body);
                } else {
                    delivery = delivery.failed(
                        Some(status_code),
                        response_body.unwrap_or_else(|| "No response body".to_string()),
                    );
                }
            }
            Err(e) => {
                delivery = delivery.failed(None, e.to_string());
            }
        }

        Ok(delivery)
    }

    async fn deliver_with_retry(
        &self,
        webhook: &ReiWebhook,
        payload: &WebhookPayload,
    ) -> Result<WebhookDelivery, DomainError> {
        let mut delivery = WebhookDelivery::new(webhook.id, payload.clone());
        let mut delay = self.config.retry_base_delay_ms;

        for attempt in 0..=webhook.max_retries {
            if attempt > 0 {
                // Wait before retry
                tokio::time::sleep(Duration::from_millis(delay)).await;
                delay = (delay * 2).min(self.config.retry_max_delay_ms);
                delivery = delivery.retry();
            }

            let result = self.deliver(webhook, payload).await?;

            if result.status == DeliveryStatus::Success {
                return Ok(result);
            }

            // Update delivery with latest attempt info
            delivery.status_code = result.status_code;
            delivery.response_body = result.response_body;
            delivery.attempts = result.attempts;
        }

        // All retries exhausted
        delivery.status = DeliveryStatus::Failed;
        delivery.completed_at = Some(chrono::Utc::now());
        Ok(delivery)
    }

    async fn verify_endpoint(&self, url: &str) -> Result<bool, DomainError> {
        let response = self
            .client
            .head(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() == 405),
            Err(_) => Ok(false),
        }
    }

    fn sign_payload(&self, secret: &str, payload: &[u8]) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload);
        let result = mac.finalize();
        let bytes = result.into_bytes();

        // Return hex-encoded signature with sha256= prefix
        format!("sha256={}", hex::encode(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_payload() {
        let webhook = HttpWebhook::new();
        let signature = webhook.sign_payload("test-secret", b"test payload");

        assert!(signature.starts_with("sha256="));
        assert_eq!(signature.len(), 7 + 64); // "sha256=" + 64 hex chars
    }
}
