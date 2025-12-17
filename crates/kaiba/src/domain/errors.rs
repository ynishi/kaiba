//! Domain Errors
//!
//! Error types for domain operations.

use thiserror::Error;
use uuid::Uuid;

/// Domain layer errors
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Repository error: {0}")]
    Repository(String),

    #[error("External service error: {0}")]
    ExternalService(String),
}

impl DomainError {
    pub fn not_found<T: AsRef<str>>(entity_type: T, id: Uuid) -> Self {
        Self::NotFound {
            entity_type: entity_type.as_ref().to_string(),
            id: id.to_string(),
        }
    }

    pub fn not_found_str<T: AsRef<str>>(entity_type: T, id: &str) -> Self {
        Self::NotFound {
            entity_type: entity_type.as_ref().to_string(),
            id: id.to_string(),
        }
    }
}
