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

#[derive(Debug, Serialize)]
pub struct CreateMemoryRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct SearchMemoriesRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
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
        let resp = self.client
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

        let reis: Vec<ReiResponse> = resp.json().await
            .context("Failed to parse response")?;

        Ok(reis)
    }

    /// Get a specific Rei
    pub async fn get_rei(&self, rei_id: &str) -> Result<ReiResponse> {
        let url = format!("{}/kaiba/rei/{}", self.base_url, rei_id);
        let resp = self.client
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

        let rei: ReiResponse = resp.json().await
            .context("Failed to parse response")?;

        Ok(rei)
    }

    /// Add a memory
    pub async fn add_memory(
        &self,
        rei_id: &str,
        content: &str,
        memory_type: Option<&str>,
        importance: Option<f32>,
    ) -> Result<MemoryResponse> {
        let url = format!("{}/kaiba/rei/{}/memories", self.base_url, rei_id);

        let request = CreateMemoryRequest {
            content: content.to_string(),
            memory_type: memory_type.map(|s| s.to_string()),
            importance,
        };

        let resp = self.client
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

        let memory: MemoryResponse = resp.json().await
            .context("Failed to parse response")?;

        Ok(memory)
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

        let resp = self.client
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

        let memories: Vec<MemoryResponse> = resp.json().await
            .context("Failed to parse response")?;

        Ok(memories)
    }
}
