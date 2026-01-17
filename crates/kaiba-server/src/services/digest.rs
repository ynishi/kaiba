//! Digest Service - Consolidate and summarize memories
//!
//! Takes recent learning memories and creates a consolidated expertise.
//! After digesting, saves the expertise as a Document for GraphKai integration.

use crate::models::{Memory, MemoryType};
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::{MemoryKai, SearchFilter};
use chrono::{DateTime, Utc};
use kaiba::{DocRepository, Document, EmphasisParser, GraphBuilder, GraphRepository};
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
    /// Number of expertise entries created
    pub expertises_count: usize,
    pub summary: String,
    /// Document ID if saved to DocStore
    pub document_id: Option<Uuid>,
    /// Graph nodes created from the expertise
    pub graph_nodes_created: usize,
}

/// Parsed expertise entry from LLM output
#[derive(Debug, Clone)]
struct ParsedExpertise {
    /// The main content text
    content: String,
    /// Optional hierarchical category (e.g., "Rust > Concurrency > Async")
    topic_path: Option<String>,
}

/// Parse LLM output into multiple expertise entries
/// Format: content + optional "topic_path: X > Y > Z" + "=====" separator
fn parse_expertise_output(raw_output: &str) -> Vec<ParsedExpertise> {
    let mut expertises = Vec::new();

    // Split by separator
    for chunk in raw_output.split("=====") {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        // Extract topic_path if present
        let (content, topic_path) = extract_topic_path(chunk);

        // Skip if content is too short (likely malformed)
        if content.split_whitespace().count() < 20 {
            continue;
        }

        expertises.push(ParsedExpertise {
            content,
            topic_path,
        });
    }

    expertises
}

/// Extract topic_path from chunk if present
/// Returns (content_without_topic_path, Option<topic_path>)
fn extract_topic_path(chunk: &str) -> (String, Option<String>) {
    // Look for "topic_path:" line (case insensitive)
    let lines: Vec<&str> = chunk.lines().collect();
    let mut content_lines = Vec::new();
    let mut topic_path = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("topic_path:") {
            // Extract the path after the colon
            let path = trimmed.split_once(':').map(|x| x.1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            topic_path = path;
        } else {
            content_lines.push(line);
        }
    }

    let content = content_lines.join("\n").trim().to_string();
    (content, topic_path)
}

/// Digest service for consolidating memories
pub struct DigestService {
    pool: PgPool,
    memory_kai: Arc<MemoryKai>,
    embedding: EmbeddingService,
    client: Client,
    gemini_api_key: Option<String>,
    /// Optional DocStore for saving expertise as documents
    doc_store: Option<Arc<dyn DocRepository>>,
    /// Optional GraphKai for building knowledge graph
    graph_kai: Option<Arc<dyn GraphRepository>>,
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
            doc_store: None,
            graph_kai: None,
        }
    }

    /// Set DocStore for saving expertise as documents
    pub fn with_doc_store(mut self, doc_store: Arc<dyn DocRepository>) -> Self {
        self.doc_store = Some(doc_store);
        self
    }

    /// Set GraphKai for building knowledge graph from expertise
    pub fn with_graph_kai(mut self, graph_kai: Arc<dyn GraphRepository>) -> Self {
        self.graph_kai = Some(graph_kai);
        self
    }

    /// Digest recent learning memories for a Rei
    pub async fn digest(&self, rei_id: Uuid) -> Result<DigestResult, DigestError> {
        // 0. Get last_digest_at to filter already-digested memories
        let last_digest_at = self.get_last_digest_at(rei_id).await?;

        // 1. Get recent learning memories (not yet digested)
        let memories = self.get_learning_memories(rei_id, last_digest_at).await?;

        if memories.is_empty() {
            return Ok(DigestResult {
                rei_id,
                memories_processed: 0,
                expertise_created: false,
                expertises_count: 0,
                summary: "No memories to digest".to_string(),
                document_id: None,
                graph_nodes_created: 0,
            });
        }

        // 2. Generate digest output (multiple expertises separated by =====)
        let raw_output = self.generate_summary(&memories).await?;

        // 3. Parse into multiple expertise entries
        let parsed_expertises = parse_expertise_output(&raw_output);

        if parsed_expertises.is_empty() {
            // Fallback: treat entire output as single expertise
            tracing::warn!(
                "No expertises parsed from LLM output, using raw output as single entry"
            );
            let fallback = vec![ParsedExpertise {
                content: raw_output.clone(),
                topic_path: None,
            }];
            return self
                .store_expertises(rei_id, &memories, &fallback, &raw_output)
                .await;
        }

        // 4. Store each expertise as separate Memory
        self.store_expertises(rei_id, &memories, &parsed_expertises, &raw_output)
            .await
    }

    /// Store multiple expertise entries as separate Memories
    async fn store_expertises(
        &self,
        rei_id: Uuid,
        source_memories: &[Memory],
        expertises: &[ParsedExpertise],
        raw_output: &str,
    ) -> Result<DigestResult, DigestError> {
        let mut stored_count = 0;

        for (i, expertise) in expertises.iter().enumerate() {
            let memory_id = Uuid::new_v4();

            // Generate embedding for this expertise
            let vector = self
                .embedding
                .embed(&expertise.content)
                .await
                .map_err(|e| DigestError::EmbeddingFailed(e.to_string()))?;

            let memory = Memory {
                id: memory_id.to_string(),
                rei_id: rei_id.to_string(),
                content: expertise.content.clone(),
                memory_type: MemoryType::Expertise,
                importance: 0.9,
                tags: vec!["digest".to_string(), "auto_generated".to_string()],
                topic_path: expertise.topic_path.clone(),
                metadata: None,
                created_at: chrono::Utc::now(),
            };

            match self
                .memory_kai
                .add_memory(&rei_id.to_string(), memory, vector)
                .await
            {
                Ok(_) => {
                    stored_count += 1;
                    tracing::debug!(
                        "📝 Stored expertise {}/{}: topic_path={:?}",
                        i + 1,
                        expertises.len(),
                        expertise.topic_path
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to store expertise {}: {}", i + 1, e);
                }
            }
        }

        // 5. Save combined output as Document for GraphKai integration
        let (document_id, graph_nodes_created) = self
            .save_as_document_and_build_graph(rei_id, raw_output)
            .await;

        // 6. Update last_digest_at in state
        self.update_digest_timestamp(rei_id).await?;

        tracing::info!(
            "📝 Digest completed for Rei {}: {} memories -> {} expertises, doc={:?}, graph_nodes={}",
            rei_id,
            source_memories.len(),
            stored_count,
            document_id,
            graph_nodes_created
        );

        Ok(DigestResult {
            rei_id,
            memories_processed: source_memories.len(),
            expertise_created: stored_count > 0,
            expertises_count: stored_count,
            summary: raw_output.to_string(),
            document_id,
            graph_nodes_created,
        })
    }

    /// Save expertise as Document and build knowledge graph
    /// Returns (document_id, graph_nodes_created)
    async fn save_as_document_and_build_graph(
        &self,
        rei_id: Uuid,
        content: &str,
    ) -> (Option<Uuid>, usize) {
        let doc_store = match &self.doc_store {
            Some(ds) => ds,
            None => {
                tracing::debug!("DocStore not available, skipping document save");
                return (None, 0);
            }
        };

        // Create document from expertise
        let doc_id = Uuid::new_v4();
        let title = format!(
            "Expertise Digest - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        );
        let source_path = format!("digest/{}/{}.md", rei_id, doc_id);

        let document = Document::new(
            rei_id,
            title.clone(),
            content.to_string(),
            Some(source_path),
            None, // metadata
        );

        // Save to DocStore
        if let Err(e) = doc_store.save(&document).await {
            tracing::warn!("Failed to save expertise as document: {}", e);
            return (None, 0);
        }

        tracing::info!("📄 Saved expertise as document: {} ({})", title, doc_id);

        // Build graph if GraphKai available
        let graph_nodes = match &self.graph_kai {
            Some(graph_kai) => {
                self.build_graph_from_document(rei_id, &document, graph_kai.clone())
                    .await
            }
            None => {
                tracing::debug!("GraphKai not available, skipping graph build");
                0
            }
        };

        (Some(document.id), graph_nodes)
    }

    /// Build knowledge graph from a single document
    async fn build_graph_from_document(
        &self,
        rei_id: Uuid,
        document: &Document,
        graph_kai: Arc<dyn GraphRepository>,
    ) -> usize {
        let emphasis_parser = EmphasisParser::new();
        let builder = GraphBuilder::new(Default::default());

        // Parse emphasis from document content
        let parse_result = emphasis_parser.parse(document.id, &document.raw_content);

        if parse_result.nodes.is_empty() {
            tracing::debug!("No emphasis nodes found in expertise document");
            return 0;
        }

        // Build graph nodes and edges
        let build_result =
            builder.build_from_emphasis(rei_id, document.id, &document.title, &parse_result.nodes);

        let mut nodes_created = 0;

        // Upsert document node
        if let Some(doc_node) = &build_result.doc_node {
            if let Err(e) = graph_kai.upsert_node(doc_node).await {
                tracing::warn!("Failed to upsert doc node: {}", e);
            }
        }

        // Upsert concept nodes
        if !build_result.nodes.is_empty() {
            match graph_kai.upsert_nodes(&build_result.nodes).await {
                Ok(result) => {
                    nodes_created = result.created + result.updated;
                    tracing::info!("🔗 Created {} graph nodes from expertise", nodes_created);
                }
                Err(e) => {
                    tracing::warn!("Failed to upsert concept nodes: {}", e);
                }
            }
        }

        // Upsert extraction edges
        if !build_result.extraction_edges.is_empty() {
            if let Err(e) = graph_kai.upsert_edges(&build_result.extraction_edges).await {
                tracing::warn!("Failed to upsert extraction edges: {}", e);
            }
        }

        // Upsert co-occurrence edges
        if !build_result.co_occurrence_edges.is_empty() {
            if let Err(e) = graph_kai
                .upsert_edges(&build_result.co_occurrence_edges)
                .await
            {
                tracing::warn!("Failed to upsert co-occurrence edges: {}", e);
            }
        }

        nodes_created
    }

    /// Get last_digest_at from rei_states
    async fn get_last_digest_at(&self, rei_id: Uuid) -> Result<Option<DateTime<Utc>>, DigestError> {
        let result: Option<(Option<DateTime<Utc>>,)> =
            sqlx::query_as("SELECT last_digest_at FROM rei_states WHERE rei_id = $1")
                .bind(rei_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| DigestError::DatabaseError(e.to_string()))?;

        Ok(result.and_then(|(ts,)| ts))
    }

    /// Get recent learning memories (created after last_digest_at)
    async fn get_learning_memories(
        &self,
        rei_id: Uuid,
        last_digest_at: Option<DateTime<Utc>>,
    ) -> Result<Vec<Memory>, DigestError> {
        // Search for learning type memories
        // We use a generic query to get recent learnings
        let query_vector = self
            .embedding
            .embed("recent learnings and discoveries")
            .await
            .map_err(|e| DigestError::EmbeddingFailed(e.to_string()))?;

        // Build filter: Learning type + created after last_digest_at
        let filter = SearchFilter {
            memory_type: Some(MemoryType::Learning),
            created_after: last_digest_at,
            ..Default::default()
        };

        let memories = self
            .memory_kai
            .search_memories_with_filter(&rei_id.to_string(), query_vector, 20, filter)
            .await
            .map_err(|e| DigestError::SearchFailed(e.to_string()))?;

        Ok(memories)
    }

    /// Generate summary using Gemini
    async fn generate_summary(&self, memories: &[Memory]) -> Result<String, DigestError> {
        let api_key = self.gemini_api_key.as_ref().ok_or(DigestError::NoApiKey)?;

        // Build content from memories
        let memory_content: String = memories
            .iter()
            .enumerate()
            .map(|(i, m)| format!("### Memory {}\n{}\n", i + 1, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"You are a knowledge synthesizer. Analyze the following learning memories and create multiple focused expertise entries.

## Output Format Rules (CRITICAL)

Generate 3-5 separate expertise entries. Each entry should be:
- Focused on ONE specific topic/concept (400-500 tokens)
- Self-contained and understandable on its own
- Written in the same language as the memories

## Entry Format

Each expertise entry MUST follow this exact format:

```
[expertise content here - focused on one topic, ~400-500 tokens]

topic_path: Category > Subcategory > Topic
=====
```

The separator `=====` MUST appear after each entry (including the last one).
The `topic_path:` line is optional but recommended for categorization.

## Formatting Rules for Content

1. **Important concepts** MUST be wrapped in **bold** (double asterisks)
   → These become high-weight nodes in the knowledge graph
2. Technical terms should be wrapped in `code` (backticks)
3. *Supplementary concepts* can be in *italic*

## Example Output

**Rust**'s **async/await** was stabilized in version 1.39. The key abstraction is the `Future` trait, which represents a value that may not be available yet. Unlike traditional threading, async in Rust is *zero-cost* - it compiles down to state machines without runtime overhead.

topic_path: Programming > Rust > Concurrency
=====

The **tokio** runtime is the most popular choice for async Rust applications. It provides a multi-threaded scheduler, I/O primitives, and utilities for building network applications. Key features include `spawn` for task creation and `select!` macro for handling multiple futures.

topic_path: Programming > Rust > Runtime
=====

## Learning Memories:
{}

## Your Task:
Create 3-5 focused expertise entries from the memories above. Each entry should cover ONE specific topic deeply rather than covering everything shallowly."#,
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
        sqlx::query(
            "UPDATE rei_states SET last_digest_at = NOW(), last_active_at = NOW() WHERE rei_id = $1",
        )
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
