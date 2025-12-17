//! PostgreSQL implementation of ReiRepository

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use kaiba::{DomainError, Rei, ReiRepository, ReiState};

/// PostgreSQL implementation of ReiRepository
pub struct PgReiRepository {
    pool: PgPool,
}

impl PgReiRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Internal row type for sqlx mapping
#[derive(sqlx::FromRow)]
struct ReiRow {
    id: Uuid,
    name: String,
    role: String,
    avatar_url: Option<String>,
    manifest: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ReiRow> for Rei {
    fn from(row: ReiRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            role: row.role,
            avatar_url: row.avatar_url,
            manifest: row.manifest,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ReiStateRow {
    id: Uuid,
    rei_id: Uuid,
    token_budget: i32,
    tokens_used: i32,
    energy_level: i32,
    mood: String,
    last_active_at: Option<chrono::DateTime<chrono::Utc>>,
    updated_at: chrono::DateTime<chrono::Utc>,
    energy_regen_per_hour: i32,
}

impl From<ReiStateRow> for ReiState {
    fn from(row: ReiStateRow) -> Self {
        Self {
            id: row.id,
            rei_id: row.rei_id,
            token_budget: row.token_budget,
            tokens_used: row.tokens_used,
            energy_level: row.energy_level,
            mood: row.mood,
            last_active_at: row.last_active_at,
            updated_at: row.updated_at,
            energy_regen_per_hour: row.energy_regen_per_hour,
        }
    }
}

#[async_trait]
impl ReiRepository for PgReiRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Rei>, DomainError> {
        let row = sqlx::query_as::<_, ReiRow>("SELECT * FROM reis WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn find_all(&self) -> Result<Vec<Rei>, DomainError> {
        let rows = sqlx::query_as::<_, ReiRow>("SELECT * FROM reis ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn save(&self, rei: &Rei) -> Result<Rei, DomainError> {
        // Check if exists
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM reis WHERE id = $1)")
                .bind(rei.id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        let row = if exists {
            // Update
            sqlx::query_as::<_, ReiRow>(
                r#"
                UPDATE reis
                SET name = $2, role = $3, avatar_url = $4, manifest = $5, updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(rei.id)
            .bind(&rei.name)
            .bind(&rei.role)
            .bind(&rei.avatar_url)
            .bind(&rei.manifest)
            .fetch_one(&self.pool)
            .await
        } else {
            // Insert
            sqlx::query_as::<_, ReiRow>(
                r#"
                INSERT INTO reis (id, name, role, avatar_url, manifest)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING *
                "#,
            )
            .bind(rei.id)
            .bind(&rei.name)
            .bind(&rei.role)
            .bind(&rei.avatar_url)
            .bind(&rei.manifest)
            .fetch_one(&self.pool)
            .await
        }
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let result = sqlx::query("DELETE FROM reis WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_state(&self, rei_id: Uuid) -> Result<Option<ReiState>, DomainError> {
        let row = sqlx::query_as::<_, ReiStateRow>("SELECT * FROM rei_states WHERE rei_id = $1")
            .bind(rei_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn save_state(&self, state: &ReiState) -> Result<ReiState, DomainError> {
        let row = sqlx::query_as::<_, ReiStateRow>(
            r#"
            UPDATE rei_states
            SET energy_level = $2, mood = $3, token_budget = $4, tokens_used = $5,
                energy_regen_per_hour = $6, last_active_at = NOW(), updated_at = NOW()
            WHERE rei_id = $1
            RETURNING *
            "#,
        )
        .bind(state.rei_id)
        .bind(state.energy_level)
        .bind(&state.mood)
        .bind(state.token_budget)
        .bind(state.tokens_used)
        .bind(state.energy_regen_per_hour)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn create_state(&self, rei_id: Uuid) -> Result<ReiState, DomainError> {
        let row = sqlx::query_as::<_, ReiStateRow>(
            r#"
            INSERT INTO rei_states (rei_id)
            VALUES ($1)
            RETURNING *
            "#,
        )
        .bind(rei_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }
}
