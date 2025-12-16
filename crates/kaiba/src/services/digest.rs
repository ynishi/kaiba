//! Digest Service - Consolidate and summarize memories
//!
//! Takes recent learning memories and creates a consolidated expertise.

use crate::models::{Memory, MemoryType};
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Digest result
#[derive(Debug, Clone, Serialize)]
pub struct DigestResult {
    pub rei_id: Uuid,
    pub memories_processed: usize,
    pub expertise_created: bool,
    pub summary: String,
}

/// Digest service for consolidating memories
pub struct DigestService {
    pool: PgPool,
    memory_kai: Arc<MemoryKai>,
    embedding: EmbeddingService,
    client: Client,
    gemini_api_key: Option<String>,
}

impl DigestService {
    pub fn new(
        pool: PgPool,
        memory_kai: Arc<MemoryKai>,
        embedding: EmbeddingService,
        gemini_api_key: Option<String>,
    ) -> Self {
        Self {
            pool,
            memory_kai,
            embedding,
            client: Client::new(),
            gemini_api_key,
        }
    }

    /// Digest recent learning memories for a Rei
    pub async fn digest(&self, rei_id: Uuid) -> Result<DigestResult, DigestError> {
        // 1. Get recent learning memories (not yet digested)
        let memories = self.get_learning_memories(rei_id).await?;

        if memories.is_empty() {
            return Ok(DigestResult {
                rei_id,
                memories_processed: 0,
                expertise_created: false,
                summary: "No memories to digest".to_string(),
            });
        }

        // 2. Generate digest summary
        let summary = self.generate_summary(&memories).await?;

        // 3. Store as Expertise memory
        let memory_id = Uuid::new_v4();
        let expertise = Memory {
            id: memory_id.to_string(),
            rei_id: rei_id.to_string(),
            content: summary.clone(),
            memory_type: MemoryType::Expertise,
            importance: 0.9, // High importance for digested knowledge
            tags: vec!["digest".to_string(), "auto_generated".to_string()],
            metadata: None,
            created_at: chrono::Utc::now(),
        };

        let vector = self
            .embedding
            .embed(&summary)
            .await
            .map_err(|e| DigestError::EmbeddingFailed(e.to_string()))?;

        self.memory_kai
            .add_memory(&rei_id.to_string(), expertise, vector)
            .await
            .map_err(|e| DigestError::StorageFailed(e.to_string()))?;

        // 4. Update last_digest_at in state
        self.update_digest_timestamp(rei_id).await?;

        tracing::info!(
            "📝 Digest completed for Rei {}: {} memories -> 1 expertise",
            rei_id,
            memories.len()
        );

        Ok(DigestResult {
            rei_id,
            memories_processed: memories.len(),
            expertise_created: true,
            summary,
        })
    }

    /// Get recent learning memories
    async fn get_learning_memories(&self, rei_id: Uuid) -> Result<Vec<Memory>, DigestError> {
        // Search for learning type memories
        // We use a generic query to get recent learnings
        let query_vector = self
            .embedding
            .embed("recent learnings and discoveries")
            .await
            .map_err(|e| DigestError::EmbeddingFailed(e.to_string()))?;

        let memories = self
            .memory_kai
            .search_memories(&rei_id.to_string(), query_vector, 10)
            .await
            .map_err(|e| DigestError::SearchFailed(e.to_string()))?;

        // Filter for learning type only
        let learning_memories: Vec<Memory> = memories
            .into_iter()
            .filter(|m| matches!(m.memory_type, MemoryType::Learning))
            .collect();

        Ok(learning_memories)
    }

    /// Generate summary using Gemini
    async fn generate_summary(&self, memories: &[Memory]) -> Result<String, DigestError> {
        let api_key = self
            .gemini_api_key
            .as_ref()
            .ok_or(DigestError::NoApiKey)?;

        // Build content from memories
        let memory_content: String = memories
            .iter()
            .enumerate()
            .map(|(i, m)| format!("### Memory {}\n{}\n", i + 1, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"You are a knowledge synthesizer. Analyze the following learning memories and create a consolidated summary that:
1. Identifies key themes and insights
2. Connects related information
3. Highlights the most important takeaways
4. Organizes knowledge for easy retrieval

## Learning Memories:
{}

## Your Task:
Create a well-structured summary (in the same language as the memories) that consolidates this knowledge into expertise. Focus on actionable insights and key facts."#,
            memory_content
        );

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            api_key
        );

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: prompt }],
            }],
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| DigestError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(DigestError::ApiError(format!("{}: {}", status, body)));
        }

        let result: GeminiResponse = response
            .json()
            .await
            .map_err(|e| DigestError::ParseError(e.to_string()))?;

        // Extract text from response
        let summary = result
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_else(|| "Failed to generate summary".to_string());

        Ok(summary)
    }

    /// Update last digest timestamp
    async fn update_digest_timestamp(&self, rei_id: Uuid) -> Result<(), DigestError> {
        sqlx::query("UPDATE rei_states SET last_active_at = NOW() WHERE rei_id = $1")
            .bind(rei_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DigestError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

// Gemini API types
#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
}

#[derive(Deserialize)]
struct GeminiContentResponse {
    parts: Vec<GeminiPart>,
}

/// Digest error types
#[derive(Debug, Clone)]
pub enum DigestError {
    NoApiKey,
    SearchFailed(String),
    EmbeddingFailed(String),
    StorageFailed(String),
    ApiError(String),
    ParseError(String),
    DatabaseError(String),
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DigestError::NoApiKey => write!(f, "No Gemini API key configured"),
            DigestError::SearchFailed(msg) => write!(f, "Memory search failed: {}", msg),
            DigestError::EmbeddingFailed(msg) => write!(f, "Embedding failed: {}", msg),
            DigestError::StorageFailed(msg) => write!(f, "Storage failed: {}", msg),
            DigestError::ApiError(msg) => write!(f, "API error: {}", msg),
            DigestError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            DigestError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for DigestError {}
