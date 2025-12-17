//! Tei Repository Port
//!
//! Abstract interface for Tei persistence operations.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{errors::DomainError, ReiTei, Tei};

/// Repository interface for Tei entities
#[async_trait]
pub trait TeiRepository: Send + Sync {
    /// Find a Tei by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tei>, DomainError>;

    /// Find all Teis
    async fn find_all(&self) -> Result<Vec<Tei>, DomainError>;

    /// Save a Tei (insert or update)
    async fn save(&self, tei: &Tei) -> Result<Tei, DomainError>;

    /// Delete a Tei by ID
    async fn delete(&self, id: Uuid) -> Result<bool, DomainError>;

    /// Find all Teis associated with a Rei
    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<Tei>, DomainError>;

    /// Associate a Tei with a Rei
    async fn associate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<ReiTei, DomainError>;

    /// Disassociate a Tei from a Rei
    async fn disassociate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<bool, DomainError>;

    /// Check if Rei exists
    async fn rei_exists(&self, rei_id: Uuid) -> Result<bool, DomainError>;

    /// Check if Tei exists
    async fn tei_exists(&self, tei_id: Uuid) -> Result<bool, DomainError>;
}
