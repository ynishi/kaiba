//! Kaiba API Client

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[allow(dead_code)]
    pub avatar_url: Option<String>,
    pub state: ReiStateResponse,
}

#[derive(Debug, Deserialize)]
pub struct ReiStateResponse {
    pub energy_level: i32,
    #[allow(dead_code)]
    pub mood: String,
}

#[derive(Debug, Deserialize)]
pub struct MemoryResponse {
    #[allow(dead_code)]
    pub id: String,
    pub content: String,
    pub memory_type: String,
    #[allow(dead_code)]
    pub importance: f32,
    pub similarity: Option<f32>,
    pub created_at: DateTime<Utc>,
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
    #[allow(dead_code)]
    pub id: uuid::Uuid,
    pub name: String,
    pub role: String,
    pub energy_level: i32,
    #[allow(dead_code)]
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
    /// Search strategy (auto, parallel, graph_first, rag_first, multi_hop, single_rag, single_db, single_graph)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// Context weights for boosting/excluding topics
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, f32>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookResponse {
    pub id: Uuid,
    #[allow(dead_code)]
    pub rei_id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub events: Vec<String>,
    #[allow(dead_code)]
    pub max_retries: i32,
    #[allow(dead_code)]
    pub timeout_ms: i32,
    pub payload_format: Option<String>,
    #[allow(dead_code)]
    pub created_at: String,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub webhook_id: Uuid,
    pub event: String,
    pub status: String,
    pub status_code: Option<i32>,
    pub attempts: i32,
    pub created_at: String,
    #[allow(dead_code)]
    pub completed_at: Option<String>,
}

// ============================================
// Document API Types
// ============================================

#[derive(Debug, Serialize)]
pub struct DocumentInput {
    pub title: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct IngestDocumentsRequest {
    pub documents: Vec<DocumentInput>,
}

#[derive(Debug, Serialize)]
pub struct DeleteDocumentsRequest {
    pub doc_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub source_path: Option<String>,
    #[allow(dead_code)]
    pub checksum: String,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    #[allow(dead_code)]
    pub rei_id: Uuid,
    pub title: String,
    pub raw_content: String,
    pub source_path: Option<String>,
    #[allow(dead_code)]
    pub checksum: String,
    #[allow(dead_code)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentSaveResult {
    pub doc_id: Uuid,
    pub title: String,
    pub status: String,
    #[allow(dead_code)]
    pub emphasis: Option<EmphasisStats>,
    #[allow(dead_code)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmphasisStats {
    #[allow(dead_code)]
    pub bold: usize,
    #[allow(dead_code)]
    pub italic: usize,
    #[allow(dead_code)]
    pub bold_italic: usize,
    #[allow(dead_code)]
    pub code: usize,
}

#[derive(Debug, Deserialize)]
pub struct IngestSummary {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    #[allow(dead_code)]
    pub failed: usize,
    pub total_emphasis_nodes: usize,
}

#[derive(Debug, Deserialize)]
pub struct IngestDocumentsResponse {
    pub results: Vec<DocumentSaveResult>,
    pub summary: IngestSummary,
}

#[derive(Debug, Deserialize)]
pub struct DeleteDocumentsResponse {
    pub deleted: usize,
    pub not_found: Vec<Uuid>,
}

// ============================================
// Graph API Types
// ============================================

#[derive(Debug, Serialize)]
pub struct RebuildGraphRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub clear_existing: bool,
}

#[derive(Debug, Serialize)]
pub struct IncrementalRebuildRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RebuildGraphResponse {
    pub documents_processed: usize,
    pub nodes_created: usize,
    pub edges_created: usize,
    pub nodes_skipped: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct IncrementalRebuildResponse {
    pub documents_found: usize,
    pub documents_processed: usize,
    pub nodes_created: usize,
    pub edges_created: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GraphStatsResponse {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_type: HashMap<String, usize>,
    pub edges_by_type: HashMap<String, usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GraphNodeSummary {
    pub id: Uuid,
    pub text: String,
    pub node_type: String,
    pub weight: f32,
    pub source_doc_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GraphEdgeSummary {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: String,
    pub strength: f32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeNeighborsResponse {
    pub node: GraphNodeSummary,
    pub neighbors: Vec<GraphNodeSummary>,
    pub edges: Vec<GraphEdgeSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GraphExportResponse {
    pub nodes: Vec<GraphNodeSummary>,
    pub edges: Vec<GraphEdgeSummary>,
    pub stats: GraphStatsResponse,
    #[allow(dead_code)]
    pub metadata: GraphExportMetadata,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GraphExportMetadata {
    #[allow(dead_code)]
    pub rei_id: Uuid,
    #[allow(dead_code)]
    pub exported_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub format_version: String,
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

    /// Search memories with optional context weights and strategy
    pub async fn search_memories(
        &self,
        rei_id: &str,
        query: &str,
        limit: Option<usize>,
        strategy: Option<&str>,
        context: HashMap<String, f32>,
    ) -> Result<Vec<MemoryResponse>> {
        let url = format!("{}/kaiba/rei/{}/memories/search", self.base_url, rei_id);

        let request = SearchMemoriesRequest {
            query: query.to_string(),
            limit,
            strategy: strategy.map(|s| s.to_string()),
            context,
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
    #[allow(clippy::too_many_arguments)]
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

    // ============================================
    // Document API
    // ============================================

    /// List documents for a Rei
    pub async fn list_documents(&self, rei_id: &str) -> Result<Vec<DocumentSummary>> {
        let url = format!("{}/kaiba/rei/{}/documents", self.base_url, rei_id);

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

        let docs: Vec<DocumentSummary> = resp.json().await.context("Failed to parse response")?;
        Ok(docs)
    }

    /// Get a single document
    pub async fn get_document(&self, rei_id: &str, doc_id: &str) -> Result<DocumentResponse> {
        let url = format!(
            "{}/kaiba/rei/{}/documents/{}",
            self.base_url, rei_id, doc_id
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

        let doc: DocumentResponse = resp.json().await.context("Failed to parse response")?;
        Ok(doc)
    }

    /// Ingest documents (batch)
    pub async fn ingest_documents(
        &self,
        rei_id: &str,
        documents: Vec<DocumentInput>,
    ) -> Result<IngestDocumentsResponse> {
        let url = format!("{}/kaiba/rei/{}/documents", self.base_url, rei_id);

        let request = IngestDocumentsRequest { documents };

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

        let result: IngestDocumentsResponse =
            resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }

    /// Delete documents (batch)
    pub async fn delete_documents(
        &self,
        rei_id: &str,
        doc_ids: Vec<Uuid>,
    ) -> Result<DeleteDocumentsResponse> {
        let url = format!("{}/kaiba/rei/{}/documents", self.base_url, rei_id);

        let request = DeleteDocumentsRequest { doc_ids };

        let resp = self
            .client
            .delete(&url)
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

        let result: DeleteDocumentsResponse =
            resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }

    // ============================================
    // Graph API
    // ============================================

    /// Get graph statistics
    pub async fn get_graph_stats(&self, rei_id: &str) -> Result<GraphStatsResponse> {
        let url = format!("{}/kaiba/rei/{}/graph/stats", self.base_url, rei_id);

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

        let stats: GraphStatsResponse = resp.json().await.context("Failed to parse response")?;
        Ok(stats)
    }

    /// Rebuild knowledge graph
    pub async fn rebuild_graph(
        &self,
        rei_id: &str,
        doc_ids: Option<Vec<Uuid>>,
        clear_existing: bool,
    ) -> Result<RebuildGraphResponse> {
        let url = format!("{}/kaiba/rei/{}/graph/rebuild", self.base_url, rei_id);

        let request = RebuildGraphRequest {
            doc_ids,
            clear_existing,
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

        let result: RebuildGraphResponse = resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }

    /// Incremental graph rebuild
    pub async fn incremental_rebuild(
        &self,
        rei_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<IncrementalRebuildResponse> {
        let url = format!("{}/kaiba/rei/{}/graph/incremental", self.base_url, rei_id);

        let request = IncrementalRebuildRequest { since };

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

        let result: IncrementalRebuildResponse =
            resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }

    /// Export graph for visualization
    pub async fn export_graph(&self, rei_id: &str) -> Result<GraphExportResponse> {
        let url = format!("{}/kaiba/rei/{}/graph/export", self.base_url, rei_id);

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

        let result: GraphExportResponse = resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }

    /// Get node neighbors
    pub async fn get_node_neighbors(
        &self,
        rei_id: &str,
        node_id: &str,
    ) -> Result<NodeNeighborsResponse> {
        let url = format!(
            "{}/kaiba/rei/{}/graph/nodes/{}/neighbors",
            self.base_url, rei_id, node_id
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

        let result: NodeNeighborsResponse =
            resp.json().await.context("Failed to parse response")?;
        Ok(result)
    }
}
