//! Discord configuration

use serde::{Deserialize, Serialize};

/// Configuration for Discord integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Discord bot token
    pub token: String,
    /// Application ID (for slash commands)
    pub application_id: Option<u64>,
    /// Default guild ID (for guild-specific commands)
    pub guild_id: Option<u64>,
    /// Whether to enable slash commands
    pub enable_slash_commands: bool,
    /// Whether to respond to mentions
    pub respond_to_mentions: bool,
    /// Whether to respond to DMs
    pub respond_to_dms: bool,
}

impl DiscordConfig {
    /// Create a new Discord configuration with just a token
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            application_id: None,
            guild_id: None,
            enable_slash_commands: false,
            respond_to_mentions: true,
            respond_to_dms: true,
        }
    }

    /// Set the application ID
    pub fn with_application_id(mut self, app_id: u64) -> Self {
        self.application_id = Some(app_id);
        self
    }

    /// Set the guild ID
    pub fn with_guild_id(mut self, guild_id: u64) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Enable slash commands
    pub fn with_slash_commands(mut self, enable: bool) -> Self {
        self.enable_slash_commands = enable;
        self
    }
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            application_id: None,
            guild_id: None,
            enable_slash_commands: false,
            respond_to_mentions: true,
            respond_to_dms: true,
        }
    }
}
