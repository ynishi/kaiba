//! Rei Repository Port
//!
//! Abstract interface for Rei persistence operations.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{errors::DomainError, Rei, ReiState};

/// Repository interface for Rei entities
#[async_trait]
pub trait ReiRepository: Send + Sync {
    /// Find a Rei by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Rei>, DomainError>;

    /// Find all Reis
    async fn find_all(&self) -> Result<Vec<Rei>, DomainError>;

    /// Save a Rei (insert or update)
    async fn save(&self, rei: &Rei) -> Result<Rei, DomainError>;

    /// Delete a Rei by ID
    async fn delete(&self, id: Uuid) -> Result<bool, DomainError>;

    /// Find state for a Rei
    async fn find_state(&self, rei_id: Uuid) -> Result<Option<ReiState>, DomainError>;

    /// Save Rei state
    async fn save_state(&self, state: &ReiState) -> Result<ReiState, DomainError>;

    /// Create initial state for a new Rei
    async fn create_state(&self, rei_id: Uuid) -> Result<ReiState, DomainError>;
}
