//! Document Repository Port
//!
//! Abstract interface for Document persistence operations.
//! Documents are the Source of Truth for GraphKai.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{errors::DomainError, Document};

/// Repository interface for Document entities
#[async_trait]
pub trait DocRepository: Send + Sync {
    /// Find a document by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Document>, DomainError>;

    /// Find a document by checksum (for deduplication)
    async fn find_by_checksum(
        &self,
        rei_id: Uuid,
        checksum: &str,
    ) -> Result<Option<Document>, DomainError>;

    /// Find all documents for a Rei
    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<Document>, DomainError>;

    /// Find documents modified since a given timestamp (for incremental sync)
    async fn find_modified_since(
        &self,
        rei_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<Document>, DomainError>;

    /// Save a document (insert or update based on checksum)
    async fn save(&self, doc: &Document) -> Result<Document, DomainError>;

    /// Save multiple documents in a batch (transactional)
    async fn save_batch(&self, docs: &[Document]) -> Result<Vec<DocumentSaveResult>, DomainError>;

    /// Delete a document by ID
    async fn delete(&self, id: Uuid) -> Result<bool, DomainError>;

    /// Delete multiple documents by IDs (transactional)
    async fn delete_batch(&self, ids: &[Uuid]) -> Result<DeleteBatchResult, DomainError>;

    /// Count documents for a Rei
    async fn count_by_rei(&self, rei_id: Uuid) -> Result<usize, DomainError>;
}

/// Result of saving a single document in a batch operation
#[derive(Debug, Clone)]
pub struct DocumentSaveResult {
    /// The saved document
    pub document: Document,
    /// Whether the document was newly created or updated
    pub status: SaveStatus,
}

/// Status of a document save operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStatus {
    /// Document was newly created
    Created,
    /// Document was updated (content changed)
    Updated,
    /// Document was unchanged (same checksum)
    Unchanged,
}

/// Result of a batch delete operation
#[derive(Debug, Clone)]
pub struct DeleteBatchResult {
    /// Number of documents deleted
    pub deleted: usize,
    /// IDs that were not found
    pub not_found: Vec<Uuid>,
}
