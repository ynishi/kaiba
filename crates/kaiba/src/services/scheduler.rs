//! Scheduler Service - Autonomous decision & action execution
//!
//! For each Rei:
//! 1. Regenerate energy
//! 2. Decide action (Learn, Digest, Rest)
//! 3. Execute action

use crate::models::{Rei, ReiState, MemoryType};
use crate::services::decision::{Action, DecisionMaker};
use crate::services::digest::DigestService;
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;
use crate::services::self_learning::SelfLearningService;
use crate::services::web_search::WebSearchAgent;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use uuid::Uuid;

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Interval between cycles
    pub interval: Duration,
    /// Enable/disable scheduler
    pub enabled: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600), // 1 hour
            enabled: true,
        }
    }
}

/// Autonomous scheduler with decision-making
pub struct AutonomousScheduler {
    pool: PgPool,
    memory_kai: Arc<MemoryKai>,
    embedding: EmbeddingService,
    web_search: WebSearchAgent,
    gemini_api_key: Option<String>,
    config: SchedulerConfig,
}

impl AutonomousScheduler {
    /// Creates a new scheduler
    pub fn new(
        pool: PgPool,
        memory_kai: Arc<MemoryKai>,
        embedding: EmbeddingService,
        web_search: WebSearchAgent,
        gemini_api_key: Option<String>,
        config: Option<SchedulerConfig>,
    ) -> Self {
        Self {
            pool,
            memory_kai,
            embedding,
            web_search,
            gemini_api_key,
            config: config.unwrap_or_default(),
        }
    }

    /// Start the scheduler (runs in background)
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Run the scheduler loop
    async fn run(self) {
        if !self.config.enabled {
            tracing::info!("📅 Autonomous scheduler disabled");
            return;
        }

        tracing::info!(
            "📅 Autonomous scheduler started (interval: {:?})",
            self.config.interval
        );

        let mut ticker = interval(self.config.interval);

        // Skip the first immediate tick
        ticker.tick().await;

        loop {
            ticker.tick().await;
            tracing::info!("🔄 Scheduler: Starting autonomous cycle...");

            // 1. Regenerate energy for all Reis
            match self.regenerate_all_energy().await {
                Ok(count) => tracing::info!("⚡ Regenerated energy for {} Reis", count),
                Err(e) => tracing::warn!("⚠️  Energy regeneration failed: {}", e),
            }

            // 2. Get all Reis and process each
            let reis = match self.get_all_reis().await {
                Ok(reis) => reis,
                Err(e) => {
                    tracing::error!("Failed to get Reis: {}", e);
                    continue;
                }
            };

            for rei in reis {
                if let Err(e) = self.process_rei(&rei).await {
                    tracing::warn!("⚠️  Failed to process Rei {}: {}", rei.name, e);
                }
            }

            tracing::info!("🔄 Scheduler: Autonomous cycle completed");
        }
    }

    /// Process a single Rei - decide and execute action
    async fn process_rei(&self, rei: &Rei) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get Rei state
        let state = sqlx::query_as::<_, ReiState>(
            "SELECT * FROM rei_states WHERE rei_id = $1"
        )
        .bind(rei.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or("Rei state not found")?;

        // Count learning memories (simplified - count recent learnings)
        let memories_count = self.count_learning_memories(rei.id).await.unwrap_or(0);

        // Make decision
        let decision_maker = DecisionMaker::new(None);
        let decision = decision_maker.decide(&state, memories_count);

        tracing::info!(
            "🧠 {} decides: {} ({})",
            rei.name,
            decision.action,
            decision.reason
        );

        // Execute action
        match decision.action {
            Action::Learn => {
                self.execute_learn(rei.id).await?;
            }
            Action::Digest => {
                self.execute_digest(rei.id).await?;
            }
            Action::Rest => {
                tracing::info!("  😴 {} is resting", rei.name);
            }
        }

        Ok(())
    }

    /// Execute learning action
    async fn execute_learn(&self, rei_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let service = SelfLearningService::new(
            self.pool.clone(),
            self.memory_kai.clone(),
            self.embedding.clone(),
            self.web_search.clone(),
            None,
        );

        match service.learn(rei_id).await {
            Ok(session) => {
                tracing::info!(
                    "  🔍 Learned: {} queries, {} memories stored",
                    session.queries_generated.len(),
                    session.memories_stored
                );
            }
            Err(e) => {
                tracing::warn!("  ❌ Learning failed: {}", e);
            }
        }

        Ok(())
    }

    /// Execute digest action
    async fn execute_digest(&self, rei_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let service = DigestService::new(
            self.pool.clone(),
            self.memory_kai.clone(),
            self.embedding.clone(),
            self.gemini_api_key.clone(),
        );

        match service.digest(rei_id).await {
            Ok(result) => {
                tracing::info!(
                    "  📝 Digested: {} memories -> expertise",
                    result.memories_processed
                );
            }
            Err(e) => {
                tracing::warn!("  ❌ Digest failed: {}", e);
            }
        }

        // Reduce energy for digest
        sqlx::query("UPDATE rei_states SET energy_level = GREATEST(0, energy_level - 20) WHERE rei_id = $1")
            .bind(rei_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Count learning memories for a Rei
    async fn count_learning_memories(&self, rei_id: Uuid) -> Result<usize, String> {
        // Search for learning memories
        let query_vector = self
            .embedding
            .embed("learning")
            .await
            .map_err(|e| format!("Embedding failed: {}", e))?;

        let memories = self
            .memory_kai
            .search_memories(&rei_id.to_string(), query_vector, 20)
            .await
            .map_err(|e| format!("Search failed: {}", e))?;

        let count = memories
            .iter()
            .filter(|m| matches!(m.memory_type, MemoryType::Learning))
            .count();

        Ok(count)
    }

    /// Get all Reis
    async fn get_all_reis(&self) -> Result<Vec<Rei>, Box<dyn std::error::Error + Send + Sync>> {
        let reis = sqlx::query_as::<_, Rei>("SELECT * FROM reis")
            .fetch_all(&self.pool)
            .await?;
        Ok(reis)
    }

    /// Regenerate energy for all Reis
    async fn regenerate_all_energy(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let result = sqlx::query(
            r#"
            UPDATE rei_states
            SET energy_level = LEAST(100, energy_level + energy_regen_per_hour)
            WHERE energy_regen_per_hour > 0
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

/// Start scheduler if all required services are available
pub fn maybe_start_scheduler(
    pool: PgPool,
    memory_kai: Option<Arc<MemoryKai>>,
    embedding: Option<EmbeddingService>,
    web_search: Option<WebSearchAgent>,
    gemini_api_key: Option<String>,
    interval_secs: Option<u64>,
) -> Option<tokio::task::JoinHandle<()>> {
    let memory_kai = memory_kai?;
    let embedding = embedding?;
    let web_search = web_search?;

    let config = SchedulerConfig {
        interval: Duration::from_secs(interval_secs.unwrap_or(3600)),
        enabled: true,
    };

    let scheduler = AutonomousScheduler::new(
        pool,
        memory_kai,
        embedding,
        web_search,
        gemini_api_key,
        Some(config),
    );

    Some(scheduler.start())
}
