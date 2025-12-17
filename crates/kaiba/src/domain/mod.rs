//! Domain Layer
//!
//! Pure domain logic without infrastructure dependencies.
//! Contains entities, value objects, domain services, and errors.

pub mod entities;
pub mod errors;
pub mod services;
pub mod value_objects;

// Re-exports for convenience
pub use entities::*;
pub use errors::*;
pub use value_objects::*;
