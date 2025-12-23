//! TeiIntegration implementation for Discord

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use kaiba::domain::entities::{Message, Rei};
use kaiba::domain::errors::DomainError;
use kaiba::ports::integration::{IntegrationEvent, TeiIntegration};
use tracing::{debug, warn};

use crate::client::DiscordClient;
use crate::config::DiscordConfig;

/// Discord integration implementing TeiIntegration trait
pub struct DiscordIntegration {
    client: DiscordClient,
    #[allow(dead_code)]
    config: DiscordConfig,
}

impl DiscordIntegration {
    /// Create a new Discord integration
    pub fn new(config: DiscordConfig) -> Self {
        let client = DiscordClient::new(config.clone());
        Self { client, config }
    }

    /// Extract Discord channel ID from Rei's manifest
    fn get_channel_id(&self, rei: &Rei) -> Result<u64, DomainError> {
        rei.manifest
            .get("discord_channel_id")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .ok_or_else(|| {
                DomainError::Validation(format!(
                    "Rei '{}' does not have discord_channel_id configured in manifest",
                    rei.name
                ))
            })
    }

    /// Convert serenity Message to domain Message
    fn convert_message(&self, msg: &serenity::model::channel::Message) -> Message {
        // Convert serenity's time::OffsetDateTime to chrono::DateTime<Utc>
        let timestamp = DateTime::<Utc>::from_timestamp(
            msg.timestamp.unix_timestamp(),
            msg.timestamp.nanosecond(),
        )
        .unwrap_or_else(Utc::now);

        Message::new(
            msg.id.to_string(),
            msg.channel_id.to_string(),
            msg.author.id.to_string(),
            msg.author.name.clone(),
            msg.content.clone(),
            "discord",
        )
        .with_timestamp(timestamp)
        .with_metadata(serde_json::json!({
            "guild_id": msg.guild_id.map(|g| g.to_string()),
            "attachments_count": msg.attachments.len(),
            "embeds_count": msg.embeds.len(),
            "is_bot": msg.author.bot,
        }))
    }
}

#[async_trait]
impl TeiIntegration for DiscordIntegration {
    async fn read_messages(&self, rei: &Rei) -> Result<Vec<Message>, DomainError> {
        let channel_id = self.get_channel_id(rei)?;
        debug!(channel_id = %channel_id, rei_name = %rei.name, "Reading messages from Discord");

        let messages = self
            .client
            .get_messages(channel_id, 50)
            .await
            .map_err(|e| DomainError::ExternalService(format!("Discord API error: {}", e)))?;

        Ok(messages.iter().map(|m| self.convert_message(m)).collect())
    }

    async fn post_message(&self, rei: &Rei, content: &str) -> Result<(), DomainError> {
        let channel_id = self.get_channel_id(rei)?;
        debug!(
            channel_id = %channel_id,
            rei_name = %rei.name,
            content_len = %content.len(),
            "Posting message to Discord"
        );

        self.client
            .send_message(channel_id, content)
            .await
            .map_err(|e| DomainError::ExternalService(format!("Discord API error: {}", e)))?;

        Ok(())
    }

    fn name(&self) -> &str {
        "discord"
    }

    async fn handle_webhook(
        &self,
        payload: &[u8],
    ) -> Result<Option<IntegrationEvent>, DomainError> {
        // Parse Discord interaction webhook
        let payload_str = std::str::from_utf8(payload)
            .map_err(|e| DomainError::Validation(format!("Invalid UTF-8 in webhook: {}", e)))?;

        let json: serde_json::Value = serde_json::from_str(payload_str)
            .map_err(|e| DomainError::Validation(format!("Invalid JSON in webhook: {}", e)))?;

        // Check interaction type
        let interaction_type = json.get("type").and_then(|t| t.as_u64()).unwrap_or(0);

        match interaction_type {
            // Ping (verification)
            1 => {
                debug!("Received Discord ping verification");
                Ok(None)
            }
            // Application Command (slash command)
            2 => {
                let data = json.get("data").ok_or_else(|| {
                    DomainError::Validation("Missing data in slash command".into())
                })?;

                let command = data
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or_default()
                    .to_string();

                let user_id = json
                    .get("member")
                    .and_then(|m| m.get("user"))
                    .or_else(|| json.get("user"))
                    .and_then(|u| u.get("id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or_default()
                    .to_string();

                let channel_id = json
                    .get("channel_id")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string();

                Ok(Some(IntegrationEvent::SlashCommand {
                    command,
                    user_id,
                    channel_id,
                    args: vec![],
                }))
            }
            // Message Component
            3 => {
                debug!("Received Discord message component interaction");
                Ok(None)
            }
            _ => {
                warn!(interaction_type = %interaction_type, "Unknown Discord interaction type");
                Ok(None)
            }
        }
    }

    async fn health_check(&self) -> Result<bool, DomainError> {
        // Try to get current user to verify connection
        match self.client.http().get_current_user().await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!(error = %e, "Discord health check failed");
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = DiscordConfig::new("test-token")
            .with_application_id(12345)
            .with_guild_id(67890)
            .with_slash_commands(true);

        assert_eq!(config.token, "test-token");
        assert_eq!(config.application_id, Some(12345));
        assert_eq!(config.guild_id, Some(67890));
        assert!(config.enable_slash_commands);
    }
}
