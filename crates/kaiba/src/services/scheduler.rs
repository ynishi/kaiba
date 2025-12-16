//! Scheduler Service - Autonomous learning scheduler
//!
//! Runs self-learning for all Reis at configured intervals.

use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;
use crate::services::self_learning::SelfLearningService;
use crate::services::web_search::WebSearchAgent;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Interval between learning cycles
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

/// Learning scheduler
pub struct LearningScheduler {
    pool: PgPool,
    memory_kai: Arc<MemoryKai>,
    embedding: EmbeddingService,
    web_search: WebSearchAgent,
    config: SchedulerConfig,
}

impl LearningScheduler {
    /// Creates a new scheduler
    pub fn new(
        pool: PgPool,
        memory_kai: Arc<MemoryKai>,
        embedding: EmbeddingService,
        web_search: WebSearchAgent,
        config: Option<SchedulerConfig>,
    ) -> Self {
        Self {
            pool,
            memory_kai,
            embedding,
            web_search,
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
            tracing::info!("📅 Learning scheduler disabled");
            return;
        }

        tracing::info!(
            "📅 Learning scheduler started (interval: {:?})",
            self.config.interval
        );

        let mut ticker = interval(self.config.interval);

        // Skip the first immediate tick
        ticker.tick().await;

        loop {
            ticker.tick().await;

            tracing::info!("🔄 Scheduler: Starting learning cycle...");

            let service = SelfLearningService::new(
                self.pool.clone(),
                self.memory_kai.clone(),
                self.embedding.clone(),
                self.web_search.clone(),
                None,
            );

            let results = service.learn_all().await;

            let successful = results.iter().filter(|r| r.is_ok()).count();
            let failed = results.iter().filter(|r| r.is_err()).count();

            tracing::info!(
                "🔄 Scheduler: Learning cycle completed ({} successful, {} failed)",
                successful,
                failed
            );

            // Log individual results
            for result in &results {
                match result {
                    Ok(session) => {
                        tracing::info!(
                            "  ✅ {}: {} queries, {} memories",
                            session.rei_name,
                            session.queries_generated.len(),
                            session.memories_stored
                        );
                    }
                    Err(e) => {
                        tracing::warn!("  ❌ Error: {}", e);
                    }
                }
            }
        }
    }
}

/// Start scheduler if all required services are available
pub fn maybe_start_scheduler(
    pool: PgPool,
    memory_kai: Option<Arc<MemoryKai>>,
    embedding: Option<EmbeddingService>,
    web_search: Option<WebSearchAgent>,
    interval_secs: Option<u64>,
) -> Option<tokio::task::JoinHandle<()>> {
    let memory_kai = memory_kai?;
    let embedding = embedding?;
    let web_search = web_search?;

    let config = SchedulerConfig {
        interval: Duration::from_secs(interval_secs.unwrap_or(3600)),
        enabled: true,
    };

    let scheduler = LearningScheduler::new(pool, memory_kai, embedding, web_search, Some(config));

    Some(scheduler.start())
}
