//! HybridSearchService - Unified RAG + Graph search
//!
//! Combines MemoryKai (Qdrant vector search) with GraphKai (Neo4j graph traversal)
//! to provide dense knowledge retrieval.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

/// Context weight for boosting/excluding topics
/// - weight > 0: boost (1.0 = full boost)
/// - weight = 0: exclude
pub type ContextWeights = HashMap<String, f32>;

use kaiba::{DomainError, GraphNode, GraphRepository};

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
}

/// Search strategy for hybrid queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HybridStrategy {
    /// Graph traversal first, then RAG supplement
    GraphFirst,
    /// RAG search first, then graph expansion
    RagFirst,
    /// Execute both in parallel and merge
    Parallel,
    /// Automatically determine based on query
    #[default]
    Auto,
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

/// Unified search service combining RAG and Graph
pub struct HybridSearchService {
    memory_kai: Arc<MemoryKai>,
    graph_kai: Arc<Neo4jGraphRepository>,
    embedding: EmbeddingService,
}

impl HybridSearchService {
    /// Create a new HybridSearchService
    pub fn new(
        memory_kai: Arc<MemoryKai>,
        graph_kai: Arc<Neo4jGraphRepository>,
        embedding: EmbeddingService,
    ) -> Self {
        Self {
            memory_kai,
            graph_kai,
            embedding,
        }
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
            HybridStrategy::GraphFirst => self.search_graph_first(rei_id, query, &config).await,
            HybridStrategy::RagFirst => self.search_rag_first(rei_id, query, &config).await,
            HybridStrategy::Parallel => self.search_parallel(rei_id, query, &config).await,
            HybridStrategy::Auto => {
                // Should not reach here, but fallback to parallel
                self.search_parallel(rei_id, query, &config).await
            }
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
            strategy_used: HybridStrategy::RagFirst,
        })
    }

    /// Parallel: Execute both searches and merge
    async fn search_parallel(
        &self,
        rei_id: &Uuid,
        query: &str,
        config: &HybridSearchConfig,
    ) -> Result<HybridSearchResult, HybridSearchError> {
        let mut rag_sources = Vec::new();
        let mut graph_sources = Vec::new();
        let mut memories_map: HashMap<String, ScoredMemory> = HashMap::new();

        // Generate embedding once
        let query_vector = self
            .embedding
            .embed(query)
            .await
            .map_err(|e| HybridSearchError::Embedding(e.to_string()))?;

        // Prepare rei_id string for RAG search
        let rei_id_str = rei_id.to_string();

        // Execute both searches in parallel
        let (rag_result, graph_result) = tokio::join!(
            self.memory_kai.search_memories_with_scores(
                &rei_id_str,
                query_vector,
                config.rag_limit
            ),
            self.graph_kai
                .find_nodes_by_text(*rei_id, query, None, config.rag_limit)
        );

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

        Ok(HybridSearchResult {
            memories: memories_map.into_values().collect(),
            rag_sources,
            graph_sources,
            strategy_used: HybridStrategy::Parallel,
        })
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

    /// Apply context weights to search results
    /// - weight > 0: boost score
    /// - weight = 0: exclude
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
                // Exclude if content contains any exclude topic
                let content_lower = scored.memory.content.to_lowercase();
                !exclude_topics
                    .iter()
                    .any(|topic| content_lower.contains(&topic.to_lowercase()))
            })
            .map(|mut scored| {
                // Boost score based on matching topics
                let content_lower = scored.memory.content.to_lowercase();
                let mut total_boost = 0.0;
                let mut match_count = 0;

                for (topic, weight) in &boost_topics {
                    if content_lower.contains(&topic.to_lowercase()) {
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
            "Context applied: {} memories after filtering (exclude: {:?}, boost: {:?})",
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
