//! Infrastructure Adapters
//!
//! Implementations of domain ports for external systems.

pub mod postgres;

// Re-exports
pub use postgres::{PgReiRepository, PgTeiRepository};
