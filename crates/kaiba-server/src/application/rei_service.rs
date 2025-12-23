//! Rei Application Service (Use Case)
//!
//! Orchestrates domain operations for Rei management.

use std::sync::Arc;
use uuid::Uuid;

use kaiba::{DomainError, Rei, ReiRepository, ReiState};

/// Application service for Rei operations
pub struct ReiService<R: ReiRepository> {
    repo: Arc<R>,
}

impl<R: ReiRepository> ReiService<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// Get all Reis with their states
    pub async fn list_all(&self) -> Result<Vec<(Rei, ReiState)>, DomainError> {
        let reis = self.repo.find_all().await?;
        let mut results = Vec::with_capacity(reis.len());

        for rei in reis {
            let state = self
                .repo
                .find_state(rei.id)
                .await?
                .unwrap_or_else(ReiState::default_values);
            results.push((rei, state));
        }

        Ok(results)
    }

    /// Get a Rei by ID with state
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<(Rei, ReiState)>, DomainError> {
        let rei = match self.repo.find_by_id(id).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        let state = self
            .repo
            .find_state(rei.id)
            .await?
            .unwrap_or_else(ReiState::default_values);

        Ok(Some((rei, state)))
    }

    /// Create a new Rei with initial state
    pub async fn create(
        &self,
        name: String,
        role: String,
        avatar_url: Option<String>,
        manifest: Option<serde_json::Value>,
    ) -> Result<(Rei, ReiState), DomainError> {
        let rei = Rei::new(name, role, avatar_url, manifest);
        let saved_rei = self.repo.save(&rei).await?;
        let state = self.repo.create_state(saved_rei.id).await?;

        tracing::info!("Created Rei: {} ({})", saved_rei.name, saved_rei.id);

        Ok((saved_rei, state))
    }

    /// Update a Rei
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        role: Option<String>,
        avatar_url: Option<String>,
        manifest: Option<serde_json::Value>,
    ) -> Result<(Rei, ReiState), DomainError> {
        let current = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Rei", id))?;

        let updated = Rei {
            id: current.id,
            name: name.unwrap_or(current.name),
            role: role.unwrap_or(current.role),
            avatar_url: avatar_url.or(current.avatar_url),
            manifest: manifest.unwrap_or(current.manifest),
            created_at: current.created_at,
            updated_at: chrono::Utc::now(),
        };

        let saved = self.repo.save(&updated).await?;
        let state = self
            .repo
            .find_state(saved.id)
            .await?
            .unwrap_or_else(ReiState::default_values);

        Ok((saved, state))
    }

    /// Delete a Rei
    pub async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let deleted = self.repo.delete(id).await?;
        if deleted {
            tracing::info!("Deleted Rei: {}", id);
        }
        Ok(deleted)
    }

    /// Get Rei state
    pub async fn get_state(&self, rei_id: Uuid) -> Result<Option<ReiState>, DomainError> {
        self.repo.find_state(rei_id).await
    }

    /// Update Rei state
    pub async fn update_state(
        &self,
        rei_id: Uuid,
        energy_level: Option<i32>,
        mood: Option<String>,
        token_budget: Option<i32>,
        tokens_used: Option<i32>,
        energy_regen_per_hour: Option<i32>,
    ) -> Result<ReiState, DomainError> {
        let current = self
            .repo
            .find_state(rei_id)
            .await?
            .ok_or_else(|| DomainError::not_found("ReiState", rei_id))?;

        let updated = ReiState {
            id: current.id,
            rei_id: current.rei_id,
            token_budget: token_budget.unwrap_or(current.token_budget),
            tokens_used: tokens_used.unwrap_or(current.tokens_used),
            energy_level: energy_level.unwrap_or(current.energy_level),
            mood: mood.unwrap_or(current.mood),
            last_active_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            energy_regen_per_hour: energy_regen_per_hour.unwrap_or(current.energy_regen_per_hour),
            last_digest_at: current.last_digest_at,
            last_learn_at: current.last_learn_at,
        };

        self.repo.save_state(&updated).await
    }
}
