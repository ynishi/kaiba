//! Embedding Service Port
//!
//! Abstract interface for text embedding generation.

use async_trait::async_trait;

use crate::domain::errors::DomainError;

/// Service interface for generating text embeddings
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate embedding vector for text
    async fn embed(&self, text: &str) -> Result<Vec<f32>, DomainError>;

    /// Generate embeddings for multiple texts
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DomainError>;
}
