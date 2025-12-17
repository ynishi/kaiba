//! Ports (Interfaces)
//!
//! Abstract interfaces that define how the domain layer
//! interacts with external systems (repositories, services).
//!
//! Implementations of these traits live in the infrastructure layer.

pub mod repositories;
pub mod services;

// Re-exports
pub use repositories::*;
pub use services::*;
