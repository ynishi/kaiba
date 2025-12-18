//! Infrastructure Adapters
//!
//! Implementations of domain ports for external systems.

pub mod formatters;
pub mod postgres;
pub mod webhook;

// Re-exports
pub use postgres::{PgReiRepository, PgReiWebhookRepository, PgTeiRepository};
pub use webhook::HttpWebhook;
