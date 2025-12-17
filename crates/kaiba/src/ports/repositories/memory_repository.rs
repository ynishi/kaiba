//! Memory Repository Port
//!
//! Abstract interface for Memory persistence operations.
//! Note: Memory is stored in vector database (Qdrant), not PostgreSQL.

use async_trait::async_trait;

use crate::domain::{errors::DomainError, Memory, MemoryType, TagMatchMode};

/// Search filter for memory queries
#[derive(Debug, Default, Clone)]
pub struct MemorySearchFilter {
    /// Filter by memory type
    pub memory_type: Option<MemoryType>,
    /// Filter by tags
    pub tags: Vec<String>,
    /// Tag matching mode
    pub tags_match_mode: TagMatchMode,
    /// Minimum importance score
    pub min_importance: Option<f32>,
}

/// Repository interface for Memory entities
#[async_trait]
pub trait MemoryRepository: Send + Sync {
    /// Add a memory with its embedding vector
    async fn add(
        &self,
        rei_id: &str,
        memory: Memory,
        embedding: Vec<f32>,
    ) -> Result<(), DomainError>;

    /// Search memories by semantic similarity
    async fn search(
        &self,
        rei_id: &str,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<Memory>, DomainError>;

    /// Search memories with filters
    async fn search_with_filter(
        &self,
        rei_id: &str,
        query_vector: Vec<f32>,
        limit: usize,
        filter: MemorySearchFilter,
    ) -> Result<Vec<Memory>, DomainError>;
}
