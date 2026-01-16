//! Infrastructure Adapters
//!
//! Implementations of domain ports for external systems.

pub mod formatters;
pub mod in_memory;
pub mod neo4j;
pub mod postgres;
pub mod webhook;

// Re-exports
pub use neo4j::Neo4jGraphRepository;
pub use postgres::{PgDocRepository, PgReiRepository, PgReiWebhookRepository, PgTeiRepository};
pub use webhook::HttpWebhook;
