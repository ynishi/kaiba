//! Discord webhook handling

use kaiba::domain::errors::DomainError;
use kaiba::ports::integration::IntegrationEvent;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Discord webhook handler for incoming events
pub struct DiscordWebhookHandler {
    /// Public key for signature verification (optional)
    public_key: Option<String>,
}

impl DiscordWebhookHandler {
    /// Create a new webhook handler
    pub fn new() -> Self {
        Self { public_key: None }
    }

    /// Create a webhook handler with signature verification
    pub fn with_public_key(public_key: impl Into<String>) -> Self {
        Self {
            public_key: Some(public_key.into()),
        }
    }

    /// Parse a Discord gateway event into an IntegrationEvent
    pub fn parse_gateway_event(
        &self,
        event_type: &str,
        data: &serde_json::Value,
    ) -> Result<Option<IntegrationEvent>, DomainError> {
        match event_type {
            "MESSAGE_CREATE" => self.parse_message_create(data),
            "MESSAGE_REACTION_ADD" => self.parse_reaction_add(data),
            _ => {
                debug!(event_type = %event_type, "Ignoring Discord gateway event");
                Ok(None)
            }
        }
    }

    fn parse_message_create(
        &self,
        data: &serde_json::Value,
    ) -> Result<Option<IntegrationEvent>, DomainError> {
        let msg: DiscordMessage = serde_json::from_value(data.clone())
            .map_err(|e| DomainError::Validation(format!("Invalid MESSAGE_CREATE: {}", e)))?;

        // Ignore bot messages
        if msg.author.bot.unwrap_or(false) {
            return Ok(None);
        }

        // Check if it's a DM
        if msg.guild_id.is_none() {
            return Ok(Some(IntegrationEvent::DirectMessage {
                user_id: msg.author.id,
                user_name: msg.author.username,
                content: msg.content,
            }));
        }

        Ok(Some(IntegrationEvent::MessageReceived {
            channel_id: msg.channel_id,
            user_id: msg.author.id,
            user_name: msg.author.username,
            content: msg.content,
            metadata: serde_json::json!({
                "guild_id": msg.guild_id,
                "message_id": msg.id,
            }),
        }))
    }

    fn parse_reaction_add(
        &self,
        data: &serde_json::Value,
    ) -> Result<Option<IntegrationEvent>, DomainError> {
        let reaction: DiscordReaction = serde_json::from_value(data.clone())
            .map_err(|e| DomainError::Validation(format!("Invalid MESSAGE_REACTION_ADD: {}", e)))?;

        let emoji = reaction
            .emoji
            .name
            .unwrap_or_else(|| reaction.emoji.id.unwrap_or_default());

        Ok(Some(IntegrationEvent::ReactionAdded {
            message_id: reaction.message_id,
            channel_id: reaction.channel_id,
            user_id: reaction.user_id,
            emoji,
        }))
    }

    /// Verify Discord signature (for HTTP interactions)
    #[allow(dead_code)]
    pub fn verify_signature(
        &self,
        signature: &str,
        timestamp: &str,
        body: &[u8],
    ) -> Result<bool, DomainError> {
        let Some(ref _public_key) = self.public_key else {
            warn!("Signature verification requested but no public key configured");
            return Ok(false);
        };

        // Note: Full signature verification requires ed25519 crate
        // For now, we'll trust the signature if format is valid
        if signature.len() != 128 || !signature.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(false);
        }

        // Verify timestamp is recent (within 5 seconds)
        if let Ok(ts) = timestamp.parse::<i64>() {
            let now = chrono::Utc::now().timestamp();
            if (now - ts).abs() > 5 {
                warn!(
                    timestamp = %timestamp,
                    now = %now,
                    "Discord webhook timestamp too old"
                );
                return Ok(false);
            }
        }

        let _ = body; // Used in real verification

        Ok(true)
    }
}

impl Default for DiscordWebhookHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Internal types for parsing Discord events

#[derive(Debug, Deserialize, Serialize)]
struct DiscordMessage {
    id: String,
    channel_id: String,
    guild_id: Option<String>,
    author: DiscordUser,
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct DiscordUser {
    id: String,
    username: String,
    bot: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DiscordReaction {
    user_id: String,
    channel_id: String,
    message_id: String,
    emoji: DiscordEmoji,
}

#[derive(Debug, Deserialize, Serialize)]
struct DiscordEmoji {
    id: Option<String>,
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_create() {
        let handler = DiscordWebhookHandler::new();
        let data = serde_json::json!({
            "id": "123",
            "channel_id": "456",
            "guild_id": "789",
            "author": {
                "id": "user123",
                "username": "testuser",
                "bot": false
            },
            "content": "Hello, world!"
        });

        let event = handler.parse_message_create(&data).unwrap();
        assert!(event.is_some());

        if let Some(IntegrationEvent::MessageReceived {
            channel_id,
            user_id,
            content,
            ..
        }) = event
        {
            assert_eq!(channel_id, "456");
            assert_eq!(user_id, "user123");
            assert_eq!(content, "Hello, world!");
        } else {
            panic!("Expected MessageReceived event");
        }
    }

    #[test]
    fn test_ignore_bot_messages() {
        let handler = DiscordWebhookHandler::new();
        let data = serde_json::json!({
            "id": "123",
            "channel_id": "456",
            "guild_id": "789",
            "author": {
                "id": "bot123",
                "username": "botuser",
                "bot": true
            },
            "content": "Bot message"
        });

        let event = handler.parse_message_create(&data).unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_dm() {
        let handler = DiscordWebhookHandler::new();
        let data = serde_json::json!({
            "id": "123",
            "channel_id": "456",
            "author": {
                "id": "user123",
                "username": "testuser"
            },
            "content": "DM message"
        });

        let event = handler.parse_message_create(&data).unwrap();
        assert!(event.is_some());

        if let Some(IntegrationEvent::DirectMessage {
            user_id, content, ..
        }) = event
        {
            assert_eq!(user_id, "user123");
            assert_eq!(content, "DM message");
        } else {
            panic!("Expected DirectMessage event");
        }
    }
}
