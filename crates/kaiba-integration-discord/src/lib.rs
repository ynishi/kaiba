//! Discord Integration for Kaiba
//!
//! This crate provides Discord platform integration for the Kaiba AI persona system.
//!
//! # Usage
//!
//! ```rust,ignore
//! use kaiba_integration_discord::{DiscordIntegration, DiscordConfig};
//!
//! let config = DiscordConfig::new("your-bot-token");
//! let integration = DiscordIntegration::new(config).await?;
//! ```

mod client;
mod config;
mod integration;
mod webhook;

pub use client::DiscordClient;
pub use config::DiscordConfig;
pub use integration::DiscordIntegration;
pub use webhook::DiscordWebhookHandler;
