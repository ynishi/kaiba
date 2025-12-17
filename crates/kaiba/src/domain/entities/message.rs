//! Message Entity
//!
//! Represents a message received from or sent to an integration platform.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message from an integration platform (Discord, Slack, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Platform-specific message ID
    pub id: String,
    /// Channel or conversation ID
    pub channel_id: String,
    /// Author's platform-specific ID
    pub author_id: String,
    /// Author's display name
    pub author_name: String,
    /// Message content
    pub content: String,
    /// When the message was created
    pub timestamp: DateTime<Utc>,
    /// Platform name ("discord", "slack", etc.)
    pub platform: String,
    /// Platform-specific metadata (attachments, embeds, etc.)
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Message {
    /// Create a new message
    pub fn new(
        id: impl Into<String>,
        channel_id: impl Into<String>,
        author_id: impl Into<String>,
        author_name: impl Into<String>,
        content: impl Into<String>,
        platform: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            channel_id: channel_id.into(),
            author_id: author_id.into(),
            author_name: author_name.into(),
            content: content.into(),
            timestamp: Utc::now(),
            platform: platform.into(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Add metadata to the message
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }
}
