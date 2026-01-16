//! Document - Source of Truth for GraphKai

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ============================================
// Request DTOs
// ============================================

/// Single document input for batch ingestion
#[derive(Debug, Deserialize, ToSchema)]
pub struct DocumentInput {
    /// Document title
    pub title: String,
    /// Raw Markdown content
    pub content: String,
    /// Original file path (for Git/file sync)
    #[serde(default)]
    pub source_path: Option<String>,
    /// Additional metadata (frontmatter, etc.)
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Batch document ingestion request
#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestDocumentsRequest {
    /// Documents to ingest (always array, even for single doc)
    pub documents: Vec<DocumentInput>,
}

/// Batch document deletion request
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteDocumentsRequest {
    /// Document IDs to delete
    pub doc_ids: Vec<Uuid>,
}

// ============================================
// Response DTOs
// ============================================

/// Status of a document save operation
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DocumentStatus {
    /// Document was newly created
    Created,
    /// Document was updated (content changed)
    Updated,
    /// Document was unchanged (same checksum)
    Unchanged,
    /// Save failed
    Failed,
}

/// Emphasis parsing statistics
#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema)]
pub struct EmphasisStats {
    /// Number of bold emphasis nodes
    pub bold: usize,
    /// Number of italic emphasis nodes
    pub italic: usize,
    /// Number of bold+italic emphasis nodes
    pub bold_italic: usize,
    /// Number of code emphasis nodes
    pub code: usize,
}

impl EmphasisStats {
    #[allow(dead_code)]
    pub fn total(&self) -> usize {
        self.bold + self.italic + self.bold_italic + self.code
    }
}

/// Result of saving a single document
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentSaveResultDto {
    /// Document ID
    pub doc_id: Uuid,
    /// Document title
    pub title: String,
    /// Save status
    pub status: DocumentStatus,
    /// Parsed emphasis statistics (only for created/updated docs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emphasis: Option<EmphasisStats>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary of batch ingestion
#[derive(Debug, Serialize, ToSchema)]
pub struct IngestSummary {
    /// Total documents processed
    pub total: usize,
    /// Documents created
    pub created: usize,
    /// Documents updated
    pub updated: usize,
    /// Documents unchanged
    pub unchanged: usize,
    /// Documents failed
    pub failed: usize,
    /// Total emphasis nodes extracted
    pub total_emphasis_nodes: usize,
}

/// Batch document ingestion response
#[derive(Debug, Serialize, ToSchema)]
pub struct IngestDocumentsResponse {
    /// Results for each document
    pub results: Vec<DocumentSaveResultDto>,
    /// Summary statistics
    pub summary: IngestSummary,
}

/// Batch document deletion response
#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteDocumentsResponse {
    /// Number of documents deleted
    pub deleted: usize,
    /// IDs that were not found
    pub not_found: Vec<Uuid>,
}

/// Document response (single document)
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub rei_id: Uuid,
    pub title: String,
    pub raw_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub checksum: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<kaiba::Document> for DocumentResponse {
    fn from(doc: kaiba::Document) -> Self {
        Self {
            id: doc.id,
            rei_id: doc.rei_id,
            title: doc.title,
            raw_content: doc.raw_content,
            source_path: doc.source_path,
            checksum: doc.checksum,
            metadata: doc.metadata,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
        }
    }
}

/// Document list response (summary, without content)
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<kaiba::Document> for DocumentSummary {
    fn from(doc: kaiba::Document) -> Self {
        Self {
            id: doc.id,
            title: doc.title,
            source_path: doc.source_path,
            checksum: doc.checksum,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
        }
    }
}
