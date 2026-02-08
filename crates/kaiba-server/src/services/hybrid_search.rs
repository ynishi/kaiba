//! HybridSearchService - Unified RAG + Graph search
//!
//! Combines MemoryKai (Qdrant vector search) with GraphKai (Neo4j graph traversal)
//! to provide dense knowledge retrieval.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

/// Context weight for boosting/excluding topics
/// - weight > 0: boost (1.0 = full boost)
/// - weight = 0: exclude
pub type ContextWeights = HashMap<String, f32>;

use kaiba::{DocRepository, DomainError, GraphNode, GraphRepository};

use crate::adapters::Neo4jGraphRepository;
use crate::models::Memory;
use crate::services::embedding::EmbeddingService;
use crate::services::qdrant::MemoryKai;

/// Error type for HybridSearch operations
#[derive(Debug, Error)]
pub enum HybridSearchError {
    #[error("Embedding generation failed: {0}")]
    Embedding(String),

    #[error("RAG search failed: {0}")]
    RagSearch(String),

    #[error("Graph search failed: {0}")]
    GraphSearch(#[from] DomainError),

    #[error("DB search failed: {0}")]
    DbSearch(String),
}

/// Search strategy for hybrid queries
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HybridStrategy {
    // === 複合戦略 ===
    /// Graph traversal first, then RAG supplement
    GraphFirst,
    /// RAG search first, then graph expansion
    RagFirst,
    /// Execute all (RAG + DB + Graph) in parallel and merge
    Parallel,
    /// Multi-hop iterative: RAG → Graph → RAG (configurable depth)
    MultiHop,
    /// Automatically determine based on query
    #[default]
    Auto,

    // === 単体戦略 ===
    /// RAG (Qdrant) only
    SingleRag,
    /// DB full-text (PostgreSQL) only
    SingleDb,
    /// Graph (Neo4j) only
    SingleGraph,
}

/// Strategy set for combining multiple strategies (run in parallel)
#[derive(Debug, Clone, Default)]
pub struct StrategySet {
    pub strategies: HashSet<HybridStrategy>,
    /// Hop depth for MultiHop strategy (default: 2)
    pub hop_depth: u32,
}

impl StrategySet {
    /// Create a set with a single strategy
    pub fn single(strategy: HybridStrategy) -> Self {
        let mut strategies = HashSet::new();
        strategies.insert(strategy);
        Self {
            strategies,
            hop_depth: 2,
        }
    }

    /// Create a set with multiple strategies
    pub fn multiple(strategies: impl IntoIterator<Item = HybridStrategy>) -> Self {
        Self {
            strategies: strategies.into_iter().collect(),
            hop_depth: 2,
        }
    }

    /// Set hop depth for MultiHop
    pub fn with_hop_depth(mut self, depth: u32) -> Self {
        self.hop_depth = depth;
        self
    }

    /// Check if a strategy is in the set
    pub fn contains(&self, strategy: HybridStrategy) -> bool {
        self.strategies.contains(&strategy)
    }

    /// Check if set is empty (defaults to Auto)
    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }
}

/// Memory with similarity score
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f32,
}

/// Result from hybrid search with source attribution and scores
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    /// Combined memories with scores from both sources
    pub memories: Vec<ScoredMemory>,
    /// Memory IDs that came from RAG (Qdrant)
    pub rag_sources: Vec<String>,
    /// Memory IDs that came from Graph (Neo4j)
    pub graph_sources: Vec<String>,
    /// Memory IDs that came from DB full-text search (PostgreSQL)
    pub db_sources: Vec<String>,
    /// Strategy that was actually used
    pub strategy_used: HybridStrategy,
}

/// Configuration for a single hybrid search request
#[derive(Debug, Clone, Default)]
pub struct HybridSearchConfig {
    /// Search strategy to use
    pub strategy: HybridStrategy,
    /// Maximum RAG results
    pub rag_limit: usize,
    /// Graph traversal depth
    pub graph_depth: u32,
    /// Minimum similarity for graph nodes
    pub min_similarity: f32,
    /// Context weights for boosting/excluding topics
    /// - weight > 0: boost (1.0 = full boost)
    /// - weight = 0: exclude
    pub context: ContextWeights,
}

impl HybridSearchConfig {
    /// Create default config with specified limits
    pub fn new() -> Self {
        Self {
            strategy: HybridStrategy::Auto,
            rag_limit: 5,
            graph_depth: 2,
            min_similarity: 0.7,
            context: HashMap::new(),
        }
    }

    /// Add context weight
    pub fn with_context(mut self, context: ContextWeights) -> Self {
        self.context = context;
        self
    }
}

/// Unified search service combining RAG, Graph, and DB full-text search
pub struct HybridSearchService {
    memory_kai: Arc<MemoryKai>,
    graph_kai: Arc<Neo4jGraphRepository>,
    doc_store: Option<Arc<dyn DocRepository>>,
    embedding: EmbeddingService,
}

impl HybridSearchService {
    /// Create a new HybridSearchService
    pub fn new(
        memory_kai: Arc<MemoryKai>,
        graph_kai: Arc<Neo4jGraphRepository>,
        doc_store: Option<Arc<dyn DocRepository>>,
        embedding: EmbeddingService,
    ) -> Self {
        Self {
            memory_kai,
            graph_kai,
            doc_store,
            embedding,
        }
    }

    /// Perform hybrid search with multiple strategies, merge and deduplicate results.
    ///
    /// Each strategy runs sequentially (true parallelism requires `Arc<Self>` + spawn;
    /// individual strategies already use internal concurrency via `tokio::join!`).
    pub async fn search_with_strategies(
        &self,
        rei_id: &Uuid,
        query: &str,
        strategy_set: &StrategySet,
        base_config: HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        if strategy_set.is_empty() {
            return self.search(rei_id, query, base_config).await;
        }

        let strategies: Vec<HybridStrategy> = strategy_set.strategies.iter().copied().collect();
        if strategies.len() == 1 {
            let mut config = base_config;
            config.strategy = strategies[0];
            return self.search(rei_id, query, config).await;
        }

        tracing::info!(
            "StrategySet: running {} strategies: {:?}",
            strategies.len(),
            strategies
        );

        let mut merged_memories: HashMap<String, ScoredMemory> = HashMap::new();
        let mut all_rag_sources = Vec::new();
        let mut all_graph_sources = Vec::new();
        let mut all_db_sources = Vec::new();

        for &strategy in &strategies {
            let mut config = base_config.clone();
            config.strategy = strategy;
            if strategy == HybridStrategy::MultiHop {
                config.graph_depth = strategy_set.hop_depth;
            }

            let result = self.search(rei_id, query, config).await?;
            all_rag_sources.extend(result.rag_sources);
            all_graph_sources.extend(result.graph_sources);
            all_db_sources.extend(result.db_sources);

            for scored in result.memories {
                let id = scored.memory.id.clone();
                match merged_memories.get(&id) {
                    Some(existing) if existing.score >= scored.score => {}
                    _ => {
                        merged_memories.insert(id, scored);
                    }
                }
            }
        }

        let mut memories: Vec<ScoredMemory> = merged_memories.into_values().collect();
        memories.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        memories.truncate(base_config.rag_limit);

        Ok(HybridSearchResult {
            memories,
            rag_sources: all_rag_sources,
            graph_sources: all_graph_sources,
            db_sources: all_db_sources,
            strategy_used: HybridStrategy::Parallel,
        })
    }

    /// Perform hybrid search
    pub async fn search(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        // Determine actual strategy to use
        let strategy = match config.strategy {
            HybridStrategy::Auto => classify_query(query),
            other => other,
        };

        tracing::info!(
            "HybridSearch: query='{}', strategy={:?}, context_keys={:?}",
            query,
            strategy,
            config.context.keys().collect::<Vec<_>>()
        );

        let result = match strategy {
            // 複合戦略
            HybridStrategy::GraphFirst => self.search_graph_first(rei_id, query, &config).await,
            HybridStrategy::RagFirst => self.search_rag_first(rei_id, query, &config).await,
            HybridStrategy::Parallel => self.search_parallel(rei_id, query, &config).await,
            HybridStrategy::MultiHop => self.search_multi_hop(rei_id, query, &config).await,
            HybridStrategy::Auto => {
                // Should not reach here, but fallback to parallel
                self.search_parallel(rei_id, query, &config).await
            }
            // 単体戦略
            HybridStrategy::SingleRag => self.search_single_rag(rei_id, query, &config).await,
            HybridStrategy::SingleDb => self.search_single_db(rei_id, query, &config).await,
            HybridStrategy::SingleGraph => self.search_single_graph(rei_id, query, &config).await,
        }?;

        // Apply context weights (boost/exclude)
        Ok(self.apply_context(result, &config.context))
    }

    /// GraphFirst: Search graph, then supplement with RAG
    async fn search_graph_first(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let mut rag_sources = Vec::new();
        let mut graph_sources = Vec::new();
        let mut memories_map: HashMap<String, ScoredMemory> = HashMap::new();

        // 1. Search graph nodes by text
        let graph_nodes = self
            .graph_kai
            .find_nodes_by_text(*rei_id, query, None, config.rag_limit)
            .await?;

        tracing::info!(
            "GraphFirst: Found {} nodes from text search",
            graph_nodes.len()
        );

        // 2. Expand neighbors for found nodes
        let mut all_nodes: Vec<GraphNode> = graph_nodes.clone();
        for node in &graph_nodes {
            let neighbors = self
                .graph_kai
                .get_neighbors(node.id, config.graph_depth)
                .await?;
            all_nodes.extend(neighbors);
        }

        // 3. Convert graph nodes to pseudo-memories with weight as score
        for node in &all_nodes {
            let scored = self.node_to_scored_memory(rei_id, node);
            graph_sources.push(scored.memory.id.clone());
            memories_map.insert(scored.memory.id.clone(), scored);
        }

        // 4. Supplement with RAG if we need more results
        let remaining = config.rag_limit.saturating_sub(memories_map.len());
        if remaining > 0 {
            let query_vector = self
                .embedding
                .embed(query)
                .await
                .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;
            let rei_id_str = rei_id.to_string();
            let rag_results = self
                .memory_kai
                .search_memories_with_scores(&rei_id_str, query_vector, remaining)
                .await
                .map_err(|e| HybridSearchError::RagSearch(e.to_string()))?;

            for (memory, score) in rag_results {
                if !memories_map.contains_key(&memory.id) {
                    rag_sources.push(memory.id.clone());
                    memories_map.insert(memory.id.clone(), ScoredMemory { memory, score });
                }
            }
        }

        Ok(HybridSearchResult {
            memories: memories_map.into_values().collect(),
            rag_sources,
            graph_sources,
            db_sources: Vec::new(),
            strategy_used: HybridStrategy::GraphFirst,
        })
    }

    /// RagFirst: Search RAG, then expand with graph
    async fn search_rag_first(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let mut rag_sources = Vec::new();
        let mut graph_sources = Vec::new();
        let mut memories_map: HashMap<String, ScoredMemory> = HashMap::new();

        // 1. Search RAG first with scores
        let query_vector = self
            .embedding
            .embed(query)
            .await
            .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;
        let rei_id_str = rei_id.to_string();
        let rag_results = self
            .memory_kai
            .search_memories_with_scores(&rei_id_str, query_vector, config.rag_limit)
            .await
            .map_err(|e| HybridSearchError::RagSearch(e.to_string()))?;

        tracing::info!("RagFirst: Found {} memories from RAG", rag_results.len());

        for (memory, score) in rag_results {
            rag_sources.push(memory.id.clone());
            memories_map.insert(memory.id.clone(), ScoredMemory { memory, score });
        }

        // 2. For each RAG result, find related graph nodes
        let memory_ids: Vec<String> = memories_map.keys().cloned().collect();
        for id in memory_ids {
            let memory = &memories_map[&id].memory;
            // Extract key terms from memory content (simplified: first few words)
            let key_terms: Vec<&str> = memory.content.split_whitespace().take(5).collect();
            let search_term = key_terms.join(" ");

            if !search_term.is_empty() {
                let graph_nodes = self
                    .graph_kai
                    .find_nodes_by_text(*rei_id, &search_term, None, 3)
                    .await?;

                // Get neighbors of found nodes
                for node in &graph_nodes {
                    let neighbors = self.graph_kai.get_neighbors(node.id, 1).await?;
                    for neighbor in neighbors {
                        let scored = self.node_to_scored_memory(rei_id, &neighbor);
                        if !memories_map.contains_key(&scored.memory.id) {
                            graph_sources.push(scored.memory.id.clone());
                            memories_map.insert(scored.memory.id.clone(), scored);
                        }
                    }
                }
            }
        }

        // 3. Limit total results
        let memories: Vec<ScoredMemory> = memories_map
            .into_values()
            .take(config.rag_limit * 2) // Allow some expansion
            .collect();

        Ok(HybridSearchResult {
            memories,
            rag_sources,
            graph_sources,
            db_sources: Vec::new(),
            strategy_used: HybridStrategy::RagFirst,
        })
    }

    /// Parallel: Execute RAG, Graph, and DB searches and merge
    async fn search_parallel(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let mut rag_sources = Vec::new();
        let mut graph_sources = Vec::new();
        let mut db_sources = Vec::new();
        let mut memories_map: HashMap<String, ScoredMemory> = HashMap::new();

        // Generate embedding once
        let query_vector = self
            .embedding
            .embed(query)
            .await
            .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;

        // Prepare rei_id string for RAG search
        let rei_id_str = rei_id.to_string();

        // Execute all searches in parallel (RAG + Graph + DB)
        let rag_future = self.memory_kai.search_memories_with_scores(
            &rei_id_str,
            query_vector,
            config.rag_limit,
        );

        let graph_future =
            self.graph_kai
                .find_nodes_by_text(*rei_id, query, None, config.rag_limit);

        // DB full-text search (if doc_store is available)
        let db_future = async {
            if let Some(doc_store) = &self.doc_store {
                doc_store
                    .search_fulltext(*rei_id, query, config.rag_limit)
                    .await
            } else {
                Ok(vec![])
            }
        };

        let (rag_result, graph_result, db_result) =
            tokio::join!(rag_future, graph_future, db_future);

        // Process RAG results with actual scores
        if let Ok(rag_results) = rag_result {
            tracing::info!("Parallel: Found {} memories from RAG", rag_results.len());
            for (memory, score) in rag_results {
                rag_sources.push(memory.id.clone());
                memories_map.insert(memory.id.clone(), ScoredMemory { memory, score });
            }
        }

        // Process Graph results
        if let Ok(graph_nodes) = graph_result {
            tracing::info!("Parallel: Found {} nodes from Graph", graph_nodes.len());
            for node in graph_nodes {
                let scored = self.node_to_scored_memory(rei_id, &node);
                if !memories_map.contains_key(&scored.memory.id) {
                    graph_sources.push(scored.memory.id.clone());
                    memories_map.insert(scored.memory.id.clone(), scored);
                }
            }
        }

        // Process DB full-text search results
        if let Ok(documents) = db_result {
            tracing::info!("Parallel: Found {} documents from DB", documents.len());
            for doc in documents {
                let memory_id = format!("doc:{}", doc.id);
                if !memories_map.contains_key(&memory_id) {
                    db_sources.push(memory_id.clone());
                    memories_map.insert(
                        memory_id.clone(),
                        ScoredMemory {
                            memory: Memory {
                                id: memory_id,
                                rei_id: rei_id.to_string(),
                                content: doc.raw_content,
                                memory_type: crate::models::MemoryType::Fact,
                                importance: 0.7, // Default importance for DB results
                                tags: vec!["source:db".to_string(), "document".to_string()],
                                topic_path: None,
                                created_at: doc.created_at,
                                metadata: Some(serde_json::json!({
                                    "doc_id": doc.id.to_string(),
                                    "title": doc.title,
                                    "source_path": doc.source_path,
                                })),
                            },
                            score: 0.7, // DB full-text doesn't provide similarity score
                        },
                    );
                }
            }
        }

        Ok(HybridSearchResult {
            memories: memories_map.into_values().collect(),
            rag_sources,
            graph_sources,
            db_sources,
            strategy_used: HybridStrategy::Parallel,
        })
    }

    // ========== 単体戦略 ==========

    /// SingleRag: RAG (Qdrant) search only
    async fn search_single_rag(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let query_vector = self
            .embedding
            .embed(query)
            .await
            .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;

        let rei_id_str = rei_id.to_string();
        let rag_results = self
            .memory_kai
            .search_memories_with_scores(&rei_id_str, query_vector, config.rag_limit)
            .await
            .map_err(|e| HybridSearchError::RagSearch(e.to_string()))?;

        tracing::info!("SingleRag: Found {} memories", rag_results.len());

        let rag_sources: Vec<String> = rag_results.iter().map(|(m, _)| m.id.clone()).collect();
        let memories: Vec<ScoredMemory> = rag_results
            .into_iter()
            .map(|(memory, score)| ScoredMemory { memory, score })
            .collect();

        Ok(HybridSearchResult {
            memories,
            rag_sources,
            graph_sources: Vec::new(),
            db_sources: Vec::new(),
            strategy_used: HybridStrategy::SingleRag,
        })
    }

    /// SingleDb: DB full-text (PostgreSQL) search only
    async fn search_single_db(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let doc_store = match &self.doc_store {
            Some(ds) => ds,
            None => {
                return Ok(HybridSearchResult {
                    memories: Vec::new(),
                    rag_sources: Vec::new(),
                    graph_sources: Vec::new(),
                    db_sources: Vec::new(),
                    strategy_used: HybridStrategy::SingleDb,
                });
            }
        };

        let documents = doc_store
            .search_fulltext(*rei_id, query, config.rag_limit)
            .await
            .map_err(|e| HybridSearchError::DbSearch(e.to_string()))?;

        tracing::info!("SingleDb: Found {} documents", documents.len());

        let mut db_sources = Vec::new();
        let mut memories = Vec::new();

        for doc in documents {
            let memory_id = format!("doc:{}", doc.id);
            db_sources.push(memory_id.clone());
            memories.push(ScoredMemory {
                memory: Memory {
                    id: memory_id,
                    rei_id: rei_id.to_string(),
                    content: doc.raw_content,
                    memory_type: crate::models::MemoryType::Fact,
                    importance: 0.7,
                    tags: vec!["source:db".to_string(), "document".to_string()],
                    topic_path: None,
                    created_at: doc.created_at,
                    metadata: Some(serde_json::json!({
                        "doc_id": doc.id.to_string(),
                        "title": doc.title,
                        "source_path": doc.source_path,
                    })),
                },
                score: 0.7,
            });
        }

        Ok(HybridSearchResult {
            memories,
            rag_sources: Vec::new(),
            graph_sources: Vec::new(),
            db_sources,
            strategy_used: HybridStrategy::SingleDb,
        })
    }

    /// SingleGraph: Graph (Neo4j) search only
    async fn search_single_graph(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let graph_nodes = self
            .graph_kai
            .find_nodes_by_text(*rei_id, query, None, config.rag_limit)
            .await?;

        tracing::info!("SingleGraph: Found {} nodes", graph_nodes.len());

        let mut graph_sources = Vec::new();
        let mut memories = Vec::new();

        for node in graph_nodes {
            let scored = self.node_to_scored_memory(rei_id, &node);
            graph_sources.push(scored.memory.id.clone());
            memories.push(scored);
        }

        Ok(HybridSearchResult {
            memories,
            rag_sources: Vec::new(),
            graph_sources,
            db_sources: Vec::new(),
            strategy_used: HybridStrategy::SingleGraph,
        })
    }

    // ========== MultiHop戦略 ==========

    /// MultiHop: RAG → Graph expansion → RAG (iterative)
    async fn search_multi_hop(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let mut all_memories: HashMap<String, ScoredMemory> = HashMap::new();
        let mut rag_sources = Vec::new();
        let mut graph_sources = Vec::new();

        // HOP 1: Initial RAG search
        let query_vector = self
            .embedding
            .embed(query)
            .await
            .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;

        let rei_id_str = rei_id.to_string();
        let initial_results = self
            .memory_kai
            .search_memories_with_scores(&rei_id_str, query_vector, config.rag_limit)
            .await
            .map_err(|e| HybridSearchError::RagSearch(e.to_string()))?;

        tracing::info!(
            "MultiHop HOP1: Found {} initial memories",
            initial_results.len()
        );

        for (memory, score) in &initial_results {
            rag_sources.push(memory.id.clone());
            all_memories.insert(
                memory.id.clone(),
                ScoredMemory {
                    memory: memory.clone(),
                    score: *score,
                },
            );
        }

        // HOP 2: Extract keywords from initial results
        let keywords = self.extract_keywords_from_memories(&initial_results);
        tracing::info!("MultiHop HOP2: Extracted keywords: {:?}", keywords);

        if keywords.is_empty() {
            return Ok(HybridSearchResult {
                memories: all_memories.into_values().collect(),
                rag_sources,
                graph_sources,
                db_sources: Vec::new(),
                strategy_used: HybridStrategy::MultiHop,
            });
        }

        // HOP 3: Graph expansion using keywords
        let mut expanded_keywords: HashSet<String> = keywords.iter().cloned().collect();

        for keyword in &keywords {
            let graph_nodes = self
                .graph_kai
                .find_nodes_by_text(*rei_id, keyword, None, 5)
                .await?;

            for node in &graph_nodes {
                // Get neighbors for expansion
                let neighbors = self
                    .graph_kai
                    .get_neighbors(node.id, config.graph_depth)
                    .await?;
                for neighbor in neighbors {
                    expanded_keywords.insert(neighbor.text.clone());

                    // Add graph nodes to results
                    let scored = self.node_to_scored_memory(rei_id, &neighbor);
                    if !all_memories.contains_key(&scored.memory.id) {
                        graph_sources.push(scored.memory.id.clone());
                        all_memories.insert(scored.memory.id.clone(), scored);
                    }
                }
            }
        }

        tracing::info!(
            "MultiHop HOP3: Expanded to {} keywords",
            expanded_keywords.len()
        );

        // HOP 4: Second RAG search with expanded keywords
        let expanded_query = expanded_keywords
            .into_iter()
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");

        if !expanded_query.is_empty() {
            let expanded_vector = self
                .embedding
                .embed(&expanded_query)
                .await
                .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;

            let expanded_results = self
                .memory_kai
                .search_memories_with_scores(&rei_id_str, expanded_vector, config.rag_limit)
                .await
                .map_err(|e| HybridSearchError::RagSearch(e.to_string()))?;

            tracing::info!(
                "MultiHop HOP4: Found {} expanded memories",
                expanded_results.len()
            );

            for (memory, score) in expanded_results {
                if !all_memories.contains_key(&memory.id) {
                    rag_sources.push(memory.id.clone());
                    // Slightly lower score for expanded results
                    all_memories.insert(
                        memory.id.clone(),
                        ScoredMemory {
                            memory,
                            score: score * 0.9,
                        },
                    );
                }
            }
        }

        Ok(HybridSearchResult {
            memories: all_memories.into_values().collect(),
            rag_sources,
            graph_sources,
            db_sources: Vec::new(),
            strategy_used: HybridStrategy::MultiHop,
        })
    }

    /// Extract keywords from memory results (for MultiHop)
    fn extract_keywords_from_memories(&self, memories: &[(Memory, f32)]) -> Vec<String> {
        let mut keywords = Vec::new();

        for (memory, _score) in memories.iter().take(3) {
            // Extract from tags
            for tag in &memory.tags {
                if !tag.starts_with("source:") && !tag.starts_with("node_type:") {
                    keywords.push(tag.clone());
                }
            }

            // Extract first few significant words from content
            let words: Vec<&str> = memory
                .content
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .take(5)
                .collect();
            keywords.extend(words.into_iter().map(String::from));
        }

        // Deduplicate
        keywords.sort();
        keywords.dedup();
        keywords.truncate(10);

        keywords
    }

    /// Convert a GraphNode to a ScoredMemory (uses weight as score)
    fn node_to_scored_memory(&self, rei_id: &Uuid, node: &GraphNode) -> ScoredMemory {
        ScoredMemory {
            memory: Memory {
                id: format!("graph:{}", node.id),
                rei_id: rei_id.to_string(),
                content: node.text.clone(),
                memory_type: crate::models::MemoryType::Fact,
                importance: node.weight,
                tags: vec![
                    format!("node_type:{}", node.node_type),
                    "source:graph".to_string(),
                ],
                topic_path: None,
                created_at: chrono::Utc::now(),
                metadata: Some(node.metadata.clone()),
            },
            score: node.weight, // Use node weight as similarity score
        }
    }

    /// Apply context weights to search results (Post Re-ranking)
    /// - weight > 0: boost score
    /// - weight = 0: exclude
    ///
    /// Matches against: topic_path (highest priority), tags, content
    fn apply_context(
        &self,
        mut result: HybridSearchResult,
        context: &ContextWeights,
    ) -> HybridSearchResult {
        if context.is_empty() {
            return result;
        }

        // Separate exclude topics (weight = 0) and boost topics (weight > 0)
        let exclude_topics: Vec<&str> = context
            .iter()
            .filter(|(_, &w)| w == 0.0)
            .map(|(k, _)| k.as_str())
            .collect();

        let boost_topics: Vec<(&str, f32)> = context
            .iter()
            .filter(|(_, &w)| w > 0.0)
            .map(|(k, &w)| (k.as_str(), w))
            .collect();

        // Filter and boost memories
        result.memories = result
            .memories
            .into_iter()
            .filter(|scored| {
                // Exclude if topic_path, tags, or content contains any exclude topic
                let topic_lower = scored
                    .memory
                    .topic_path
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase();
                let tags_lower: Vec<String> = scored
                    .memory
                    .tags
                    .iter()
                    .map(|t| t.to_lowercase())
                    .collect();
                let content_lower = scored.memory.content.to_lowercase();

                !exclude_topics.iter().any(|topic| {
                    let topic_lc = topic.to_lowercase();
                    topic_lower.contains(&topic_lc)
                        || tags_lower.iter().any(|t| t.contains(&topic_lc))
                        || content_lower.contains(&topic_lc)
                })
            })
            .map(|mut scored| {
                // Boost score based on matching topics
                // Priority: topic_path (1.5x) > tags (1.2x) > content (1.0x)
                let topic_lower = scored
                    .memory
                    .topic_path
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase();
                let tags_lower: Vec<String> = scored
                    .memory
                    .tags
                    .iter()
                    .map(|t| t.to_lowercase())
                    .collect();
                let content_lower = scored.memory.content.to_lowercase();

                let mut total_boost = 0.0;
                let mut match_count = 0;

                for (topic, weight) in &boost_topics {
                    let topic_lc = topic.to_lowercase();

                    // topic_path match (highest priority - 1.5x multiplier)
                    if topic_lower.contains(&topic_lc) {
                        total_boost += weight * 1.5;
                        match_count += 1;
                    }
                    // tag match (medium priority - 1.2x multiplier)
                    else if tags_lower.iter().any(|t| t.contains(&topic_lc)) {
                        total_boost += weight * 1.2;
                        match_count += 1;
                    }
                    // content match (base priority - 1.0x multiplier)
                    else if content_lower.contains(&topic_lc) {
                        total_boost += weight;
                        match_count += 1;
                    }
                }

                // Apply boost: multiply score by (1 + average_boost)
                if match_count > 0 {
                    let avg_boost = total_boost / match_count as f32;
                    scored.score *= 1.0 + avg_boost;
                }

                scored
            })
            .collect();

        // Re-sort by score (descending)
        result.memories.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tracing::info!(
            "Post Re-ranking: {} memories (exclude: {:?}, boost: {:?})",
            result.memories.len(),
            exclude_topics,
            boost_topics.iter().map(|(t, _)| t).collect::<Vec<_>>()
        );

        result
    }
}

/// Check if query contains Japanese characters
fn contains_japanese(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{309F}' | // Hiragana
            '\u{30A0}'..='\u{30FF}' | // Katakana
            '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        )
    })
}

/// Standalone query classification function for testing
pub fn classify_query(query: &str) -> HybridStrategy {
    let query_lower = query.to_lowercase();

    // Check for keywords FIRST (before length checks) to handle Japanese properly
    // 1. Questions about relationships → GraphFirst
    let relationship_keywords = [
        "関係",
        "つながり",
        "関連",
        "relate",
        "connect",
        "between",
        "link",
    ];
    if relationship_keywords
        .iter()
        .any(|k| query_lower.contains(k))
    {
        return HybridStrategy::GraphFirst;
    }

    // 2. Specific concept lookup → GraphFirst
    if query.contains('"') || query.contains('`') {
        return HybridStrategy::GraphFirst;
    }

    // 3. Broad/exploratory queries → RagFirst
    let exploratory_keywords = [
        "について",
        "説明",
        "教えて",
        "explain",
        "tell me",
        "describe",
        "overview",
    ];
    if exploratory_keywords.iter().any(|k| query_lower.contains(k)) {
        return HybridStrategy::RagFirst;
    }

    // 4. Length-based classification (use char count for Japanese)
    let effective_length = if contains_japanese(query) {
        query.chars().count() / 3 // Approximate: 3 Japanese chars ≈ 1 English word
    } else {
        query_lower.split_whitespace().count()
    };

    // Short, specific queries → GraphFirst
    if effective_length <= 3 {
        return HybridStrategy::GraphFirst;
    }

    // Long queries → RagFirst
    if effective_length > 6 {
        HybridStrategy::RagFirst
    } else {
        HybridStrategy::Parallel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_query_short() {
        assert_eq!(classify_query("GraphRAG"), HybridStrategy::GraphFirst);
    }

    #[test]
    fn test_classify_query_relationship() {
        assert_eq!(
            classify_query("How does GraphRAG relate to knowledge graphs?"),
            HybridStrategy::GraphFirst
        );
    }

    #[test]
    fn test_classify_query_exploratory() {
        assert_eq!(
            classify_query("Tell me about the architecture of the system"),
            HybridStrategy::RagFirst
        );
    }

    #[test]
    fn test_classify_query_quoted() {
        assert_eq!(
            classify_query("Find information about \"EmphasisNode\""),
            HybridStrategy::GraphFirst
        );
    }

    #[test]
    fn test_classify_query_long() {
        let long_query = "I want to understand how the entire system works together including all the components and their interactions";
        assert_eq!(classify_query(long_query), HybridStrategy::RagFirst);
    }

    #[test]
    fn test_classify_query_japanese_relationship() {
        assert_eq!(
            classify_query("GraphKaiとMemoryKaiの関係は？"),
            HybridStrategy::GraphFirst
        );
    }

    #[test]
    fn test_classify_query_japanese_exploratory() {
        assert_eq!(
            classify_query("システムのアーキテクチャについて説明して"),
            HybridStrategy::RagFirst
        );
    }

    #[test]
    fn test_classify_query_medium_length() {
        // 4-6 words defaults to Parallel
        assert_eq!(
            classify_query("How does the system work?"),
            HybridStrategy::Parallel
        );
    }
}
