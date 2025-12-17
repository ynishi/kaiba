//! Self-Learning Service (自己活動)
//!
//! Ghost-like autonomous learning:
//! 1. Read Rei's personality/interests from manifest
//! 2. Generate search queries based on interests
//! 3. Execute WebSearch via Gemini
//! 4. Store results to MemoryKai (記憶海)

use crate::models::{Memory, MemoryType, Rei, ReiState};
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;
use crate::services::web_search::{WebSearchAgent, WebSearchResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Learning session result
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LearningSession {
    pub rei_id: Uuid,
    pub rei_name: String,
    pub queries_generated: Vec<String>,
    pub searches_completed: usize,
    pub memories_stored: usize,
    pub errors: Vec<String>,
}

/// Self-learning service configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LearningConfig {
    /// Maximum queries per session
    #[serde(default = "default_max_queries")]
    pub max_queries: usize,
    /// Minimum energy level required to learn
    #[serde(default = "default_min_energy")]
    pub min_energy: i32,
    /// Force learning even if energy is low
    #[serde(default)]
    pub force: bool,
}

fn default_max_queries() -> usize {
    3
}

fn default_min_energy() -> i32 {
    30
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            max_queries: default_max_queries(),
            min_energy: default_min_energy(),
            force: false,
        }
    }
}

/// Self-learning service for autonomous knowledge acquisition
pub struct SelfLearningService {
    pool: PgPool,
    memory_kai: Arc<MemoryKai>,
    embedding: EmbeddingService,
    web_search: WebSearchAgent,
    config: LearningConfig,
}

impl SelfLearningService {
    /// Creates a new self-learning service
    pub fn new(
        pool: PgPool,
        memory_kai: Arc<MemoryKai>,
        embedding: EmbeddingService,
        web_search: WebSearchAgent,
        config: Option<LearningConfig>,
    ) -> Self {
        Self {
            pool,
            memory_kai,
            embedding,
            web_search,
            config: config.unwrap_or_default(),
        }
    }

    /// Execute a learning session for a specific Rei
    pub async fn learn(&self, rei_id: Uuid) -> Result<LearningSession, SelfLearningError> {
        // 1. Fetch Rei and their state
        let rei = self.get_rei(rei_id).await?;
        let state = self.get_rei_state(rei_id).await?;

        // Check energy level (skip if force is enabled)
        if !self.config.force && state.energy_level < self.config.min_energy {
            return Err(SelfLearningError::InsufficientEnergy {
                current: state.energy_level,
                required: self.config.min_energy,
            });
        }

        let mut session = LearningSession {
            rei_id,
            rei_name: rei.name.clone(),
            queries_generated: Vec::new(),
            searches_completed: 0,
            memories_stored: 0,
            errors: Vec::new(),
        };

        // 2. Generate search queries from manifest
        let queries = self.generate_queries(&rei)?;
        session.queries_generated = queries.clone();

        if queries.is_empty() {
            return Err(SelfLearningError::NoInterests);
        }

        // 3. Execute searches and store results
        for query in queries.iter().take(self.config.max_queries) {
            match self.search_and_store(rei_id, query).await {
                Ok(memories_count) => {
                    session.searches_completed += 1;
                    session.memories_stored += memories_count;
                    tracing::info!(
                        "🧠 {} learned about: {} ({} memories)",
                        rei.name,
                        query,
                        memories_count
                    );
                }
                Err(e) => {
                    let error_msg = format!("Query '{}': {}", query, e);
                    tracing::warn!("⚠️  Learning error: {}", error_msg);
                    session.errors.push(error_msg);
                }
            }
        }

        // 4. Update last_active_at and reduce energy
        self.update_after_learning(rei_id, session.searches_completed)
            .await?;

        Ok(session)
    }

    /// Generate search queries from Rei's manifest
    fn generate_queries(&self, rei: &Rei) -> Result<Vec<String>, SelfLearningError> {
        let manifest = &rei.manifest;
        let mut queries = Vec::new();

        // Extract interests from manifest
        if let Some(interests) = manifest.get("interests").and_then(|v| v.as_array()) {
            for interest in interests {
                if let Some(topic) = interest.as_str() {
                    // Generate contextual query
                    let query = format!("{} latest developments 2025", topic);
                    queries.push(query);
                }
            }
        }

        // Extract learning_topics from manifest
        if let Some(topics) = manifest.get("learning_topics").and_then(|v| v.as_array()) {
            for topic in topics {
                if let Some(topic_str) = topic.as_str() {
                    queries.push(topic_str.to_string());
                }
            }
        }

        // Extract curiosities from manifest
        if let Some(curiosities) = manifest.get("curiosities").and_then(|v| v.as_array()) {
            for curiosity in curiosities {
                if let Some(q) = curiosity.as_str() {
                    queries.push(q.to_string());
                }
            }
        }

        // Fallback: use role as interest if no specific interests defined
        if queries.is_empty() {
            let role_query = format!("{} best practices 2025", rei.role);
            queries.push(role_query);
        }

        Ok(queries)
    }

    /// Execute web search and store results as memories
    async fn search_and_store(
        &self,
        rei_id: Uuid,
        query: &str,
    ) -> Result<usize, SelfLearningError> {
        // Execute web search
        let search_result = self
            .web_search
            .search(query)
            .await
            .map_err(|e| SelfLearningError::SearchFailed(e.to_string()))?;

        // Store the answer as a memory
        let memory_content = self.format_memory(&search_result);
        let vector = self
            .embedding
            .embed(&memory_content)
            .await
            .map_err(|e| SelfLearningError::EmbeddingFailed(e.to_string()))?;

        let memory_id = Uuid::new_v4();

        // Create Memory struct
        let memory = Memory {
            id: memory_id.to_string(),
            rei_id: rei_id.to_string(),
            content: memory_content,
            memory_type: MemoryType::Learning,
            importance: 0.7, // Self-learned content has moderate importance
            tags: vec!["self_learning".to_string(), "auto_generated".to_string()],
            metadata: None,
            created_at: chrono::Utc::now(),
        };

        // Use rei_id as persona_id for the collection
        self.memory_kai
            .add_memory(&rei_id.to_string(), memory, vector)
            .await
            .map_err(|e| SelfLearningError::StorageFailed(e.to_string()))?;

        // Count: 1 for the main answer
        let stored_count = 1;
        Ok(stored_count)
    }

    /// Format search response as memory content
    fn format_memory(&self, response: &WebSearchResponse) -> String {
        let mut content = format!("## Query: {}\n\n", response.query);
        content.push_str(&response.answer);

        if !response.references.is_empty() {
            content.push_str("\n\n### Sources:\n");
            for (i, reference) in response.references.iter().take(5).enumerate() {
                content.push_str(&format!(
                    "{}. [{}]({})\n",
                    i + 1,
                    reference.title,
                    reference.url
                ));
            }
        }

        content
    }

    /// Get Rei by ID
    async fn get_rei(&self, rei_id: Uuid) -> Result<Rei, SelfLearningError> {
        sqlx::query_as::<_, Rei>("SELECT * FROM reis WHERE id = $1")
            .bind(rei_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| SelfLearningError::DatabaseError(e.to_string()))?
            .ok_or(SelfLearningError::ReiNotFound(rei_id))
    }

    /// Get Rei state
    async fn get_rei_state(&self, rei_id: Uuid) -> Result<ReiState, SelfLearningError> {
        sqlx::query_as::<_, ReiState>("SELECT * FROM rei_states WHERE rei_id = $1")
            .bind(rei_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| SelfLearningError::DatabaseError(e.to_string()))?
            .ok_or(SelfLearningError::ReiNotFound(rei_id))
    }

    /// Update Rei state after learning
    async fn update_after_learning(
        &self,
        rei_id: Uuid,
        searches_completed: usize,
    ) -> Result<(), SelfLearningError> {
        // Reduce energy based on searches (10 energy per search)
        let energy_cost = (searches_completed as i32) * 10;

        sqlx::query(
            r#"
            UPDATE rei_states
            SET energy_level = GREATEST(0, energy_level - $1),
                last_active_at = NOW(),
                updated_at = NOW()
            WHERE rei_id = $2
            "#,
        )
        .bind(energy_cost)
        .bind(rei_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SelfLearningError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Execute learning for all Reis with sufficient energy
    pub async fn learn_all(&self) -> Vec<Result<LearningSession, SelfLearningError>> {
        let reis = match self.get_all_reis().await {
            Ok(reis) => reis,
            Err(e) => return vec![Err(e)],
        };

        let mut results = Vec::new();
        for rei in reis {
            let result = self.learn(rei.id).await;
            results.push(result);
        }

        results
    }

    /// Get all Reis
    async fn get_all_reis(&self) -> Result<Vec<Rei>, SelfLearningError> {
        sqlx::query_as::<_, Rei>("SELECT * FROM reis")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SelfLearningError::DatabaseError(e.to_string()))
    }
}

/// Self-learning error types
#[derive(Debug, Clone)]
pub enum SelfLearningError {
    ReiNotFound(Uuid),
    NoInterests,
    InsufficientEnergy { current: i32, required: i32 },
    SearchFailed(String),
    EmbeddingFailed(String),
    StorageFailed(String),
    DatabaseError(String),
}

impl std::fmt::Display for SelfLearningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelfLearningError::ReiNotFound(id) => write!(f, "Rei not found: {}", id),
            SelfLearningError::NoInterests => write!(f, "No interests defined in manifest"),
            SelfLearningError::InsufficientEnergy { current, required } => {
                write!(
                    f,
                    "Insufficient energy: {} (required: {})",
                    current, required
                )
            }
            SelfLearningError::SearchFailed(msg) => write!(f, "Search failed: {}", msg),
            SelfLearningError::EmbeddingFailed(msg) => write!(f, "Embedding failed: {}", msg),
            SelfLearningError::StorageFailed(msg) => write!(f, "Storage failed: {}", msg),
            SelfLearningError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for SelfLearningError {}
