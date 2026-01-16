//! Document - Source of Truth for GraphKai
//!
//! Raw Markdown storage that serves as the source for:
//! - Qdrant chunk embeddings (derived)
//! - Neo4j knowledge graph (derived)
//!
//! Documents can be re-processed to rebuild derived data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Document - Raw Markdown source for knowledge graph construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier
    pub id: Uuid,
    /// The Rei this document belongs to
    pub rei_id: Uuid,
    /// Document title
    pub title: String,
    /// Raw Markdown content (Source of Truth)
    pub raw_content: String,
    /// Original file path (for Git/file sync)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// SHA256 checksum for change detection
    pub checksum: String,
    /// Additional metadata (frontmatter, tags, etc.)
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// When this document was created
    pub created_at: DateTime<Utc>,
    /// When this document was last updated
    pub updated_at: DateTime<Utc>,
}

impl Document {
    /// Create a new document with generated ID, checksum, and timestamps
    pub fn new(
        rei_id: Uuid,
        title: String,
        raw_content: String,
        source_path: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        let checksum = Self::compute_checksum(&raw_content);
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            rei_id,
            title,
            raw_content,
            source_path,
            checksum,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Compute SHA256 checksum of content
    pub fn compute_checksum(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if content has changed by comparing checksums
    pub fn content_changed(&self, new_content: &str) -> bool {
        self.checksum != Self::compute_checksum(new_content)
    }

    /// Update content and recalculate checksum
    pub fn update_content(&mut self, new_content: String) {
        self.raw_content = new_content;
        self.checksum = Self::compute_checksum(&self.raw_content);
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_consistency() {
        let content = "# Hello\n\nThis is **bold** text.";
        let checksum1 = Document::compute_checksum(content);
        let checksum2 = Document::compute_checksum(content);
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_changes_with_content() {
        let content1 = "# Hello";
        let content2 = "# Hello World";
        let checksum1 = Document::compute_checksum(content1);
        let checksum2 = Document::compute_checksum(content2);
        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_content_changed_detection() {
        let doc = Document::new(
            Uuid::new_v4(),
            "Test".to_string(),
            "Original content".to_string(),
            None,
            None,
        );

        assert!(!doc.content_changed("Original content"));
        assert!(doc.content_changed("Modified content"));
    }
}
