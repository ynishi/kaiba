//! Platform Integration Port
//!
//! Abstract interface for integrating with messaging platforms
//! such as Discord, Slack, Teams, etc.
//!
//! Implementations of this trait should live in separate crates
//! (e.g., kaiba-integration-discord, kaiba-integration-slack).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::entities::{Message, Rei};
use crate::domain::errors::DomainError;

/// Platform integration interface
///
/// This trait abstracts the communication with external messaging platforms.
/// Each platform (Discord, Slack, etc.) should have its own implementation
/// in a separate crate.
///
/// # Example
///
/// ```rust,ignore
/// use kaiba::ports::TeiIntegration;
///
/// struct DiscordIntegration { /* ... */ }
///
/// #[async_trait]
/// impl TeiIntegration for DiscordIntegration {
///     async fn read_messages(&self, rei: &Rei) -> Result<Vec<Message>, DomainError> {
///         // Fetch messages from Discord
///     }
///     // ...
/// }
/// ```
#[async_trait]
pub trait TeiIntegration: Send + Sync {
    /// Read messages from the platform
    ///
    /// Fetches recent messages from the channel/conversation
    /// associated with the given Rei.
    async fn read_messages(&self, rei: &Rei) -> Result<Vec<Message>, DomainError>;

    /// Post a message to the platform
    ///
    /// Sends a message to the channel/conversation
    /// associated with the given Rei.
    async fn post_message(&self, rei: &Rei, content: &str) -> Result<(), DomainError>;

    /// Get the integration name (e.g., "discord", "slack")
    fn name(&self) -> &str;

    /// Handle incoming webhook events
    ///
    /// Parses and processes platform-specific webhook payloads.
    /// Returns `None` if the event doesn't require action.
    async fn handle_webhook(
        &self,
        _payload: &[u8],
    ) -> Result<Option<IntegrationEvent>, DomainError> {
        Ok(None)
    }

    /// Check if the integration is connected and healthy
    async fn health_check(&self) -> Result<bool, DomainError> {
        Ok(true)
    }
}

/// Events received from integration platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IntegrationEvent {
    /// A new message was received
    MessageReceived {
        channel_id: String,
        user_id: String,
        user_name: String,
        content: String,
        /// Platform-specific metadata
        #[serde(default)]
        metadata: serde_json::Value,
    },

    /// The Rei was mentioned in a message
    MentionReceived {
        rei_id: Uuid,
        channel_id: String,
        user_id: String,
        user_name: String,
        content: String,
    },

    /// A reaction was added to a message
    ReactionAdded {
        message_id: String,
        channel_id: String,
        user_id: String,
        emoji: String,
    },

    /// A direct message was received
    DirectMessage {
        user_id: String,
        user_name: String,
        content: String,
    },

    /// A slash command was invoked
    SlashCommand {
        command: String,
        user_id: String,
        channel_id: String,
        /// Command arguments
        args: Vec<String>,
    },
}

impl IntegrationEvent {
    /// Get the channel ID if available
    pub fn channel_id(&self) -> Option<&str> {
        match self {
            Self::MessageReceived { channel_id, .. } => Some(channel_id),
            Self::MentionReceived { channel_id, .. } => Some(channel_id),
            Self::ReactionAdded { channel_id, .. } => Some(channel_id),
            Self::SlashCommand { channel_id, .. } => Some(channel_id),
            Self::DirectMessage { .. } => None,
        }
    }

    /// Get the user ID if available
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::MessageReceived { user_id, .. } => Some(user_id),
            Self::MentionReceived { user_id, .. } => Some(user_id),
            Self::ReactionAdded { user_id, .. } => Some(user_id),
            Self::DirectMessage { user_id, .. } => Some(user_id),
            Self::SlashCommand { user_id, .. } => Some(user_id),
        }
    }

    /// Get the message content if available
    pub fn content(&self) -> Option<&str> {
        match self {
            Self::MessageReceived { content, .. } => Some(content),
            Self::MentionReceived { content, .. } => Some(content),
            Self::DirectMessage { content, .. } => Some(content),
            _ => None,
        }
    }
}

/// Configuration for platform integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Platform name
    pub platform: String,
    /// Channel/conversation ID to monitor
    pub channel_id: Option<String>,
    /// Whether to respond to mentions
    pub respond_to_mentions: bool,
    /// Whether to respond to direct messages
    pub respond_to_dms: bool,
    /// Platform-specific settings
    #[serde(default)]
    pub settings: serde_json::Value,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            platform: String::new(),
            channel_id: None,
            respond_to_mentions: true,
            respond_to_dms: true,
            settings: serde_json::Value::Null,
        }
    }
}
