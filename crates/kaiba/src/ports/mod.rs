//! Ports (Interfaces)
//!
//! Abstract interfaces that define how the domain layer
//! interacts with external systems (repositories, services, integrations).
//!
//! Implementations of these traits live in the infrastructure layer.

pub mod integration;
pub mod repositories;
pub mod services;
pub mod webhook;

// Re-exports
pub use integration::*;
pub use repositories::*;
pub use services::*;
pub use webhook::*;
