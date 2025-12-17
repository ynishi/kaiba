//! Discord API client wrapper

use serenity::http::Http;
use serenity::model::channel::Message as SerenityMessage;
use serenity::model::id::ChannelId;
use std::sync::Arc;
use tracing::{debug, error};

use crate::config::DiscordConfig;

/// Discord API client
pub struct DiscordClient {
    http: Arc<Http>,
    #[allow(dead_code)]
    config: DiscordConfig,
}

impl DiscordClient {
    /// Create a new Discord client
    pub fn new(config: DiscordConfig) -> Self {
        let http = Arc::new(Http::new(&config.token));
        Self { http, config }
    }

    /// Get recent messages from a channel
    pub async fn get_messages(
        &self,
        channel_id: u64,
        limit: u8,
    ) -> Result<Vec<SerenityMessage>, serenity::Error> {
        let channel = ChannelId::new(channel_id);
        debug!(channel_id = %channel_id, limit = %limit, "Fetching messages from Discord");

        let messages = channel
            .messages(
                &self.http,
                serenity::builder::GetMessages::new().limit(limit),
            )
            .await?;

        Ok(messages)
    }

    /// Send a message to a channel
    pub async fn send_message(
        &self,
        channel_id: u64,
        content: &str,
    ) -> Result<SerenityMessage, serenity::Error> {
        let channel = ChannelId::new(channel_id);
        debug!(channel_id = %channel_id, content_len = %content.len(), "Sending message to Discord");

        let message = channel
            .say(&self.http, content)
            .await
            .inspect_err(|e| error!(error = %e, "Failed to send Discord message"))?;

        Ok(message)
    }

    /// Reply to a message
    pub async fn reply(
        &self,
        channel_id: u64,
        message_id: u64,
        content: &str,
    ) -> Result<SerenityMessage, serenity::Error> {
        let channel = ChannelId::new(channel_id);
        debug!(
            channel_id = %channel_id,
            message_id = %message_id,
            "Replying to Discord message"
        );

        let message = channel
            .send_message(
                &self.http,
                serenity::builder::CreateMessage::new()
                    .content(content)
                    .reference_message((channel, serenity::model::id::MessageId::new(message_id))),
            )
            .await?;

        Ok(message)
    }

    /// Get the underlying HTTP client for advanced operations
    pub fn http(&self) -> &Arc<Http> {
        &self.http
    }
}
