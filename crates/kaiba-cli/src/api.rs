//! Kaiba API Client

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// API Client for Kaiba
pub struct KaibaClient {
    client: Client,
    base_url: String,
    api_key: String,
}

// ============================================
// API Response Types
// ============================================

#[derive(Debug, Deserialize)]
pub struct ReiResponse {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub state: ReiStateResponse,
}

#[derive(Debug, Deserialize)]
pub struct ReiStateResponse {
    pub energy_level: i32,
    pub mood: String,
}

#[derive(Debug, Deserialize)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub importance: f32,
}

#[derive(Debug, Deserialize)]
pub struct PromptResponse {
    pub system_prompt: String,
    pub format: String,
    pub rei: ReiSummary,
    pub memories_included: usize,
}

#[derive(Debug, Deserialize)]
pub struct ReiSummary {
    pub id: uuid::Uuid,
    pub name: String,
    pub role: String,
    pub energy_level: i32,
    pub mood: String,
}

#[derive(Debug, Serialize)]
pub struct CreateMemoryRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchMemoriesRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateWebhookRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookDeliveryResponse {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event: String,
    pub status: String,
    pub status_code: Option<i32>,
    pub attempts: i32,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl KaibaClient {
    /// Create a new API client
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    /// Test connection with health check
    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().is_success())
    }

    /// List all Reis
    pub async fn list_reis(&self) -> Result<Vec<ReiResponse>> {
        let url = format!("{}/kaiba/rei", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let reis: Vec<ReiResponse> = resp.json().await.context("Failed to parse response")?;

        Ok(reis)
    }

    /// Get a specific Rei
    pub async fn get_rei(&self, rei_id: &str) -> Result<ReiResponse> {
        let url = format!("{}/kaiba/rei/{}", self.base_url, rei_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let rei: ReiResponse = resp.json().await.context("Failed to parse response")?;

        Ok(rei)
    }

    /// Add a memory
    pub async fn add_memory(
        &self,
        rei_id: &str,
        content: &str,
        memory_type: Option<&str>,
        importance: Option<f32>,
        tags: &[String],
    ) -> Result<MemoryResponse> {
        let url = format!("{}/kaiba/rei/{}/memories", self.base_url, rei_id);

        let request = CreateMemoryRequest {
            content: content.to_string(),
            memory_type: memory_type.map(|s| s.to_string()),
            importance,
            tags: tags.to_vec(),
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let memory: MemoryResponse = resp.json().await.context("Failed to parse response")?;

        Ok(memory)
    }

    /// Get prompt for external Tei
    pub async fn get_prompt(
        &self,
        rei_id: &str,
        format: Option<&str>,
        include_memories: bool,
        context: Option<&str>,
    ) -> Result<PromptResponse> {
        let mut url = format!("{}/kaiba/rei/{}/prompt", self.base_url, rei_id);

        // Build query params
        let mut params = vec![];
        if let Some(f) = format {
            params.push(format!("format={}", f));
        }
        if include_memories {
            params.push("include_memories=true".to_string());
        }
        if let Some(ctx) = context {
            params.push(format!("context={}", urlencoding::encode(ctx)));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let prompt: PromptResponse = resp.json().await.context("Failed to parse response")?;

        Ok(prompt)
    }

    /// Search memories
    pub async fn search_memories(
        &self,
        rei_id: &str,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MemoryResponse>> {
        let url = format!("{}/kaiba/rei/{}/memories/search", self.base_url, rei_id);

        let request = SearchMemoriesRequest {
            query: query.to_string(),
            limit,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let memories: Vec<MemoryResponse> =
            resp.json().await.context("Failed to parse response")?;

        Ok(memories)
    }

    /// List webhooks for a Rei
    pub async fn list_webhooks(&self, rei_id: &str) -> Result<Vec<WebhookResponse>> {
        let url = format!("{}/kaiba/rei/{}/webhooks", self.base_url, rei_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let webhooks: Vec<WebhookResponse> =
            resp.json().await.context("Failed to parse response")?;

        Ok(webhooks)
    }

    /// Create a webhook
    pub async fn create_webhook(
        &self,
        rei_id: &str,
        name: &str,
        url: &str,
        events: Option<Vec<String>>,
        payload_format: Option<String>,
    ) -> Result<WebhookResponse> {
        let api_url = format!("{}/kaiba/rei/{}/webhooks", self.base_url, rei_id);

        let request = CreateWebhookRequest {
            name: name.to_string(),
            url: url.to_string(),
            secret: None,
            events,
            payload_format,
        };

        let resp = self
            .client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let webhook: WebhookResponse = resp.json().await.context("Failed to parse response")?;

        Ok(webhook)
    }

    /// Update a webhook
    pub async fn update_webhook(
        &self,
        rei_id: &str,
        webhook_id: &str,
        name: Option<String>,
        url: Option<String>,
        enabled: Option<bool>,
        events: Option<Vec<String>>,
        payload_format: Option<String>,
    ) -> Result<WebhookResponse> {
        let api_url = format!(
            "{}/kaiba/rei/{}/webhooks/{}",
            self.base_url, rei_id, webhook_id
        );

        let request = UpdateWebhookRequest {
            name,
            url,
            enabled,
            events,
            payload_format,
        };

        let resp = self
            .client
            .put(&api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let webhook: WebhookResponse = resp.json().await.context("Failed to parse response")?;

        Ok(webhook)
    }

    /// Delete a webhook
    pub async fn delete_webhook(&self, rei_id: &str, webhook_id: &str) -> Result<()> {
        let url = format!(
            "{}/kaiba/rei/{}/webhooks/{}",
            self.base_url, rei_id, webhook_id
        );

        let resp = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        Ok(())
    }

    /// Trigger a webhook (for testing)
    pub async fn trigger_webhook(
        &self,
        rei_id: &str,
        webhook_id: &str,
        event: Option<String>,
    ) -> Result<WebhookDeliveryResponse> {
        let url = format!(
            "{}/kaiba/rei/{}/webhooks/{}/trigger",
            self.base_url, rei_id, webhook_id
        );

        let payload = serde_json::json!({
            "event": event,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let delivery: WebhookDeliveryResponse =
            resp.json().await.context("Failed to parse response")?;

        Ok(delivery)
    }

    /// List webhook deliveries
    pub async fn list_deliveries(
        &self,
        rei_id: &str,
        webhook_id: &str,
    ) -> Result<Vec<WebhookDeliveryResponse>> {
        let url = format!(
            "{}/kaiba/rei/{}/webhooks/{}/deliveries",
            self.base_url, rei_id, webhook_id
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .context("Failed to connect to Kaiba API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("API error ({}): {}", status, body);
        }

        let deliveries: Vec<WebhookDeliveryResponse> =
            resp.json().await.context("Failed to parse response")?;

        Ok(deliveries)
    }
}
