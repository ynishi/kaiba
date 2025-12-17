//! Tei Application Service (Use Case)
//!
//! Orchestrates domain operations for Tei management.

use std::sync::Arc;
use uuid::Uuid;

use kaiba::{DomainError, Provider, ReiTei, Tei, TeiRepository};

/// Application service for Tei operations
pub struct TeiService<R: TeiRepository> {
    repo: Arc<R>,
}

impl<R: TeiRepository> TeiService<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// Get all Teis
    pub async fn list_all(&self) -> Result<Vec<Tei>, DomainError> {
        self.repo.find_all().await
    }

    /// Get a Tei by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Tei>, DomainError> {
        self.repo.find_by_id(id).await
    }

    /// Create a new Tei
    pub async fn create(
        &self,
        name: String,
        provider: Provider,
        model_id: String,
        is_fallback: bool,
        priority: i32,
        config: Option<serde_json::Value>,
        expertise: Option<serde_json::Value>,
    ) -> Result<Tei, DomainError> {
        let tei = Tei::new(
            name,
            provider,
            model_id,
            is_fallback,
            priority,
            config,
            expertise,
        );
        let saved = self.repo.save(&tei).await?;

        tracing::info!(
            "Created Tei: {} ({}) - {}",
            saved.name,
            saved.id,
            saved.model_id
        );

        Ok(saved)
    }

    /// Update a Tei
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        provider: Option<Provider>,
        model_id: Option<String>,
        is_fallback: Option<bool>,
        priority: Option<i32>,
        config: Option<serde_json::Value>,
        expertise: Option<serde_json::Value>,
    ) -> Result<Tei, DomainError> {
        let current = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Tei", id))?;

        let updated = Tei {
            id: current.id,
            name: name.unwrap_or(current.name),
            provider: provider.map(|p| p.to_string()).unwrap_or(current.provider),
            model_id: model_id.unwrap_or(current.model_id),
            is_fallback: is_fallback.unwrap_or(current.is_fallback),
            priority: priority.unwrap_or(current.priority),
            config: config.unwrap_or(current.config),
            expertise: expertise.or(current.expertise),
            created_at: current.created_at,
            updated_at: chrono::Utc::now(),
        };

        self.repo.save(&updated).await
    }

    /// Delete a Tei
    pub async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let deleted = self.repo.delete(id).await?;
        if deleted {
            tracing::info!("Deleted Tei: {}", id);
        }
        Ok(deleted)
    }

    /// Get Tei expertise
    pub async fn get_expertise(&self, id: Uuid) -> Result<Option<serde_json::Value>, DomainError> {
        let tei = self.repo.find_by_id(id).await?;
        Ok(tei.and_then(|t| t.expertise))
    }

    /// Update Tei expertise
    pub async fn update_expertise(
        &self,
        id: Uuid,
        expertise: serde_json::Value,
    ) -> Result<serde_json::Value, DomainError> {
        let current = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Tei", id))?;

        let updated = Tei {
            expertise: Some(expertise.clone()),
            updated_at: chrono::Utc::now(),
            ..current
        };

        self.repo.save(&updated).await?;
        Ok(expertise)
    }

    /// Get Teis associated with a Rei
    pub async fn list_by_rei(&self, rei_id: Uuid) -> Result<Vec<Tei>, DomainError> {
        self.repo.find_by_rei(rei_id).await
    }

    /// Associate a Tei with a Rei
    pub async fn associate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<ReiTei, DomainError> {
        // Verify Rei exists
        if !self.repo.rei_exists(rei_id).await? {
            return Err(DomainError::not_found("Rei", rei_id));
        }

        // Verify Tei exists
        if !self.repo.tei_exists(tei_id).await? {
            return Err(DomainError::not_found("Tei", tei_id));
        }

        let association = self.repo.associate(rei_id, tei_id).await?;
        tracing::info!("Associated Tei {} with Rei {}", tei_id, rei_id);

        Ok(association)
    }

    /// Disassociate a Tei from a Rei
    pub async fn disassociate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<bool, DomainError> {
        let removed = self.repo.disassociate(rei_id, tei_id).await?;
        if removed {
            tracing::info!("Disassociated Tei {} from Rei {}", tei_id, rei_id);
        }
        Ok(removed)
    }
}
