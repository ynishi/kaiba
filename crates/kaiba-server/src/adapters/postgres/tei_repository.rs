//! PostgreSQL implementation of TeiRepository

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use kaiba::{DomainError, ReiTei, Tei, TeiRepository};

/// PostgreSQL implementation of TeiRepository
pub struct PgTeiRepository {
    pool: PgPool,
}

impl PgTeiRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Internal row type for sqlx mapping
#[derive(sqlx::FromRow)]
struct TeiRow {
    id: Uuid,
    name: String,
    provider: String,
    model_id: String,
    is_fallback: bool,
    priority: i32,
    config: serde_json::Value,
    expertise: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<TeiRow> for Tei {
    fn from(row: TeiRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            provider: row.provider,
            model_id: row.model_id,
            is_fallback: row.is_fallback,
            priority: row.priority,
            config: row.config,
            expertise: row.expertise,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ReiTeiRow {
    rei_id: Uuid,
    tei_id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<ReiTeiRow> for ReiTei {
    fn from(row: ReiTeiRow) -> Self {
        Self {
            rei_id: row.rei_id,
            tei_id: row.tei_id,
            created_at: row.created_at,
        }
    }
}

#[async_trait]
impl TeiRepository for PgTeiRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tei>, DomainError> {
        let row = sqlx::query_as::<_, TeiRow>("SELECT * FROM teis WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn find_all(&self) -> Result<Vec<Tei>, DomainError> {
        let rows =
            sqlx::query_as::<_, TeiRow>("SELECT * FROM teis ORDER BY priority, created_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn save(&self, tei: &Tei) -> Result<Tei, DomainError> {
        // Check if exists
        let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM teis WHERE id = $1)")
            .bind(tei.id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        let row = if exists {
            // Update
            sqlx::query_as::<_, TeiRow>(
                r#"
                UPDATE teis
                SET name = $2, provider = $3, model_id = $4, is_fallback = $5,
                    priority = $6, config = $7, expertise = $8, updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(tei.id)
            .bind(&tei.name)
            .bind(&tei.provider)
            .bind(&tei.model_id)
            .bind(tei.is_fallback)
            .bind(tei.priority)
            .bind(&tei.config)
            .bind(&tei.expertise)
            .fetch_one(&self.pool)
            .await
        } else {
            // Insert
            sqlx::query_as::<_, TeiRow>(
                r#"
                INSERT INTO teis (id, name, provider, model_id, is_fallback, priority, config, expertise)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING *
                "#,
            )
            .bind(tei.id)
            .bind(&tei.name)
            .bind(&tei.provider)
            .bind(&tei.model_id)
            .bind(tei.is_fallback)
            .bind(tei.priority)
            .bind(&tei.config)
            .bind(&tei.expertise)
            .fetch_one(&self.pool)
            .await
        }
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let result = sqlx::query("DELETE FROM teis WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<Tei>, DomainError> {
        let rows = sqlx::query_as::<_, TeiRow>(
            r#"
            SELECT t.* FROM teis t
            INNER JOIN rei_teis rt ON t.id = rt.tei_id
            WHERE rt.rei_id = $1
            ORDER BY t.priority, t.created_at DESC
            "#,
        )
        .bind(rei_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn associate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<ReiTei, DomainError> {
        let row = sqlx::query_as::<_, ReiTeiRow>(
            r#"
            INSERT INTO rei_teis (rei_id, tei_id)
            VALUES ($1, $2)
            ON CONFLICT (rei_id, tei_id) DO UPDATE SET rei_id = EXCLUDED.rei_id
            RETURNING *
            "#,
        )
        .bind(rei_id)
        .bind(tei_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn disassociate(&self, rei_id: Uuid, tei_id: Uuid) -> Result<bool, DomainError> {
        let result = sqlx::query("DELETE FROM rei_teis WHERE rei_id = $1 AND tei_id = $2")
            .bind(rei_id)
            .bind(tei_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn rei_exists(&self, rei_id: Uuid) -> Result<bool, DomainError> {
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM reis WHERE id = $1)")
                .bind(rei_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(exists)
    }

    async fn tei_exists(&self, tei_id: Uuid) -> Result<bool, DomainError> {
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM teis WHERE id = $1)")
                .bind(tei_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(exists)
    }
}
