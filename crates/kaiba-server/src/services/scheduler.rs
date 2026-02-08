//! Scheduler Service - Autonomous decision & action execution
//!
//! For each Rei:
//! 1. Regenerate energy
//! 2. Decide action (Learn, Digest, Rest)
//! 3. Execute action
//! 4. Dispatch webhooks on learning completion

use crate::adapters::{HttpWebhook, PgReiWebhookRepository};
use crate::models::{MemoryType, Rei, ReiState};
use crate::services::decision::{Action, DecisionEngine};
use crate::services::digest::{DigestResult, DigestService};
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;
use crate::services::self_learning::{LearningSession, SelfLearningService};
use crate::services::web_search::WebSearchAgent;
use kaiba::{
    DocRepository, GraphRepository, ReiWebhookRepository, TeiWebhook, WebhookEventType,
    WebhookPayload,
};
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
    // Decision engine (LLM or rule-based)
    decision_engine: Arc<dyn DecisionEngine>,
    // Webhook support
    webhook_repo: Option<Arc<PgReiWebhookRepository>>,
    http_webhook: Option<Arc<HttpWebhook>>,
    // GraphKai integration (for digest -> document -> graph flow)
    doc_store: Option<Arc<dyn DocRepository>>,
    graph_kai: Option<Arc<dyn GraphRepository>>,
}

impl AutonomousScheduler {
    /// Creates a new scheduler
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: PgPool,
        memory_kai: Arc<MemoryKai>,
        embedding: EmbeddingService,
        web_search: WebSearchAgent,
        gemini_api_key: Option<String>,
        config: Option<SchedulerConfig>,
        decision_engine: Arc<dyn DecisionEngine>,
        webhook_repo: Option<Arc<PgReiWebhookRepository>>,
        http_webhook: Option<Arc<HttpWebhook>>,
        doc_store: Option<Arc<dyn DocRepository>>,
        graph_kai: Option<Arc<dyn GraphRepository>>,
    ) -> Self {
        Self {
            pool,
            memory_kai,
            embedding,
            web_search,
            gemini_api_key,
            config: config.unwrap_or_default(),
            decision_engine,
            webhook_repo,
            http_webhook,
            doc_store,
            graph_kai,
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
        let state = sqlx::query_as::<_, ReiState>("SELECT * FROM rei_states WHERE rei_id = $1")
            .bind(rei.id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or("Rei state not found")?;

        // Count learning memories (simplified - count recent learnings)
        let memories_count = self.count_learning_memories(rei.id).await.unwrap_or(0);

        // Make decision using the configured engine (LLM or rule-based)
        let decision = self.decision_engine.decide(&state, memories_count).await;

        tracing::info!(
            "🧠 {} [{}] decides: {} ({})",
            rei.name,
            self.decision_engine.name(),
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
    async fn execute_learn(
        &self,
        rei_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

                // Dispatch webhooks for learning completion
                self.dispatch_learning_webhooks(&session).await;
            }
            Err(e) => {
                tracing::warn!("  ❌ Learning failed: {}", e);
            }
        }

        Ok(())
    }

    /// Dispatch webhooks for learning completion
    async fn dispatch_learning_webhooks(&self, session: &LearningSession) {
        let (Some(webhook_repo), Some(http_webhook)) = (&self.webhook_repo, &self.http_webhook)
        else {
            return;
        };

        // Find webhooks subscribed to LearningCompleted event
        let webhooks = match webhook_repo
            .find_by_rei_and_event(session.rei_id, &WebhookEventType::LearningCompleted)
            .await
        {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("  ⚠️  Failed to find webhooks: {}", e);
                return;
            }
        };

        if webhooks.is_empty() {
            return;
        }

        // Build payload from LearningSession
        let payload = WebhookPayload::new(
            WebhookEventType::LearningCompleted,
            session.rei_id,
            serde_json::json!({
                "rei_name": session.rei_name,
                "queries_generated": session.queries_generated,
                "searches_completed": session.searches_completed,
                "memories_stored": session.memories_stored,
                "errors": session.errors,
            }),
        );

        // Deliver to each webhook
        for webhook in webhooks {
            tracing::info!("  📤 Dispatching webhook: {}", webhook.name);

            match http_webhook.deliver_with_retry(&webhook, &payload).await {
                Ok(delivery) => {
                    // Save delivery record
                    if let Err(e) = webhook_repo.save_delivery(&delivery).await {
                        tracing::warn!("  ⚠️  Failed to save delivery record: {}", e);
                    }

                    if delivery.status == kaiba::DeliveryStatus::Success {
                        tracing::info!("  ✅ Webhook delivered successfully");
                    } else {
                        tracing::warn!(
                            "  ❌ Webhook delivery failed: {:?}",
                            delivery.response_body
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("  ❌ Webhook delivery error: {}", e);
                }
            }
        }
    }

    /// Dispatch webhooks for digest completion
    async fn dispatch_digest_webhooks(&self, result: &DigestResult) {
        let (Some(webhook_repo), Some(http_webhook)) = (&self.webhook_repo, &self.http_webhook)
        else {
            return;
        };

        // Find webhooks subscribed to DigestCompleted event
        let webhooks = match webhook_repo
            .find_by_rei_and_event(result.rei_id, &WebhookEventType::DigestCompleted)
            .await
        {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("  ⚠️  Failed to find webhooks: {}", e);
                return;
            }
        };

        if webhooks.is_empty() {
            return;
        }

        // Build payload from DigestResult
        let payload = WebhookPayload::new(
            WebhookEventType::DigestCompleted,
            result.rei_id,
            serde_json::json!({
                "memories_processed": result.memories_processed,
                "expertise_created": result.expertise_created,
                "summary": result.summary,
            }),
        );

        // Deliver to each webhook
        for webhook in webhooks {
            tracing::info!("  📤 Dispatching digest webhook: {}", webhook.name);

            match http_webhook.deliver_with_retry(&webhook, &payload).await {
                Ok(delivery) => {
                    // Save delivery record
                    if let Err(e) = webhook_repo.save_delivery(&delivery).await {
                        tracing::warn!("  ⚠️  Failed to save delivery record: {}", e);
                    }

                    if delivery.status == kaiba::DeliveryStatus::Success {
                        tracing::info!("  ✅ Digest webhook delivered successfully");
                    } else {
                        tracing::warn!(
                            "  ❌ Digest webhook delivery failed: {:?}",
                            delivery.response_body
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("  ❌ Digest webhook delivery error: {}", e);
                }
            }
        }
    }

    /// Execute digest action
    async fn execute_digest(
        &self,
        rei_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut service = DigestService::new(
            self.pool.clone(),
            self.memory_kai.clone(),
            self.embedding.clone(),
            self.gemini_api_key.clone(),
        );

        // Add DocStore and GraphKai for document -> graph integration
        if let Some(doc_store) = &self.doc_store {
            service = service.with_doc_store(doc_store.clone());
        }
        if let Some(graph_kai) = &self.graph_kai {
            service = service.with_graph_kai(graph_kai.clone());
        }

        match service.digest(rei_id).await {
            Ok(result) => {
                tracing::info!(
                    "  📝 Digested: {} memories -> expertise (doc={:?}, graph_nodes={})",
                    result.memories_processed,
                    result.document_id,
                    result.graph_nodes_created
                );

                // Dispatch webhooks for digest completion (only if expertise was created)
                if result.expertise_created {
                    self.dispatch_digest_webhooks(&result).await;
                }
            }
            Err(e) => {
                tracing::warn!("  ❌ Digest failed: {}", e);
            }
        }

        // Reduce energy for digest
        sqlx::query(
            "UPDATE rei_states SET energy_level = GREATEST(0, energy_level - 20) WHERE rei_id = $1",
        )
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
#[allow(clippy::too_many_arguments)]
pub fn maybe_start_scheduler(
    pool: PgPool,
    memory_kai: Option<Arc<MemoryKai>>,
    embedding: Option<EmbeddingService>,
    web_search: Option<WebSearchAgent>,
    gemini_api_key: Option<String>,
    interval_secs: Option<u64>,
    decision_engine: Arc<dyn DecisionEngine>,
    webhook_repo: Option<Arc<PgReiWebhookRepository>>,
    http_webhook: Option<Arc<HttpWebhook>>,
    doc_store: Option<Arc<dyn DocRepository>>,
    graph_kai: Option<Arc<dyn GraphRepository>>,
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
        decision_engine,
        webhook_repo,
        http_webhook,
        doc_store,
        graph_kai,
    );

    Some(scheduler.start())
}
