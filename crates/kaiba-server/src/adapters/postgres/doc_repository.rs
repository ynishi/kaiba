//! PostgreSQL implementation of DocRepository

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use kaiba::{DeleteBatchResult, DocRepository, Document, DocumentSaveResult, DomainError, SaveStatus};

/// PostgreSQL implementation of DocRepository
pub struct PgDocRepository {
    pool: PgPool,
}

impl PgDocRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Internal row type for sqlx mapping
#[derive(sqlx::FromRow)]
struct DocumentRow {
    id: Uuid,
    rei_id: Uuid,
    title: String,
    raw_content: String,
    source_path: Option<String>,
    checksum: String,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DocumentRow> for Document {
    fn from(row: DocumentRow) -> Self {
        Self {
            id: row.id,
            rei_id: row.rei_id,
            title: row.title,
            raw_content: row.raw_content,
            source_path: row.source_path,
            checksum: row.checksum,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[async_trait]
impl DocRepository for PgDocRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Document>, DomainError> {
        let row = sqlx::query_as::<_, DocumentRow>("SELECT * FROM documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn find_by_checksum(
        &self,
        rei_id: Uuid,
        checksum: &str,
    ) -> Result<Option<Document>, DomainError> {
        let row = sqlx::query_as::<_, DocumentRow>(
            "SELECT * FROM documents WHERE rei_id = $1 AND checksum = $2",
        )
        .bind(rei_id)
        .bind(checksum)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<Document>, DomainError> {
        let rows = sqlx::query_as::<_, DocumentRow>(
            "SELECT * FROM documents WHERE rei_id = $1 ORDER BY created_at DESC",
        )
        .bind(rei_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_modified_since(
        &self,
        rei_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<Document>, DomainError> {
        let rows = sqlx::query_as::<_, DocumentRow>(
            "SELECT * FROM documents WHERE rei_id = $1 AND updated_at > $2 ORDER BY updated_at ASC",
        )
        .bind(rei_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn save(&self, doc: &Document) -> Result<Document, DomainError> {
        // Use ON CONFLICT for upsert based on rei_id + checksum
        let row = sqlx::query_as::<_, DocumentRow>(
            r#"
            INSERT INTO documents (id, rei_id, title, raw_content, source_path, checksum, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (rei_id, checksum) DO UPDATE SET
                title = EXCLUDED.title,
                raw_content = EXCLUDED.raw_content,
                source_path = EXCLUDED.source_path,
                metadata = EXCLUDED.metadata,
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(doc.id)
        .bind(doc.rei_id)
        .bind(&doc.title)
        .bind(&doc.raw_content)
        .bind(&doc.source_path)
        .bind(&doc.checksum)
        .bind(&doc.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn save_batch(&self, docs: &[Document]) -> Result<Vec<DocumentSaveResult>, DomainError> {
        if docs.is_empty() {
            return Ok(vec![]);
        }

        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        let mut results = Vec::with_capacity(docs.len());

        for doc in docs {
            // Check if document exists with same checksum
            let existing = sqlx::query_as::<_, DocumentRow>(
                "SELECT * FROM documents WHERE rei_id = $1 AND checksum = $2",
            )
            .bind(doc.rei_id)
            .bind(&doc.checksum)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

            let (saved_doc, status) = if let Some(existing_row) = existing {
                // Same checksum = unchanged
                (existing_row.into(), SaveStatus::Unchanged)
            } else {
                // Check if document exists by source_path (for updates)
                let by_path = if doc.source_path.is_some() {
                    sqlx::query_as::<_, DocumentRow>(
                        "SELECT * FROM documents WHERE rei_id = $1 AND source_path = $2",
                    )
                    .bind(doc.rei_id)
                    .bind(&doc.source_path)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Repository(e.to_string()))?
                } else {
                    None
                };

                if let Some(_existing) = by_path {
                    // Update existing document (content changed)
                    let row = sqlx::query_as::<_, DocumentRow>(
                        r#"
                        UPDATE documents
                        SET title = $3, raw_content = $4, checksum = $5, metadata = $6, updated_at = NOW()
                        WHERE rei_id = $1 AND source_path = $2
                        RETURNING *
                        "#,
                    )
                    .bind(doc.rei_id)
                    .bind(&doc.source_path)
                    .bind(&doc.title)
                    .bind(&doc.raw_content)
                    .bind(&doc.checksum)
                    .bind(&doc.metadata)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Repository(e.to_string()))?;

                    (row.into(), SaveStatus::Updated)
                } else {
                    // Insert new document
                    let row = sqlx::query_as::<_, DocumentRow>(
                        r#"
                        INSERT INTO documents (id, rei_id, title, raw_content, source_path, checksum, metadata)
                        VALUES ($1, $2, $3, $4, $5, $6, $7)
                        RETURNING *
                        "#,
                    )
                    .bind(doc.id)
                    .bind(doc.rei_id)
                    .bind(&doc.title)
                    .bind(&doc.raw_content)
                    .bind(&doc.source_path)
                    .bind(&doc.checksum)
                    .bind(&doc.metadata)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Repository(e.to_string()))?;

                    (row.into(), SaveStatus::Created)
                }
            };

            results.push(DocumentSaveResult {
                document: saved_doc,
                status,
            });
        }

        tx.commit().await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(results)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let result = sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_batch(&self, ids: &[Uuid]) -> Result<DeleteBatchResult, DomainError> {
        if ids.is_empty() {
            return Ok(DeleteBatchResult {
                deleted: 0,
                not_found: vec![],
            });
        }

        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        let mut deleted = 0;
        let mut not_found = Vec::new();

        for id in ids {
            let result = sqlx::query("DELETE FROM documents WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

            if result.rows_affected() > 0 {
                deleted += 1;
            } else {
                not_found.push(*id);
            }
        }

        tx.commit().await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(DeleteBatchResult { deleted, not_found })
    }

    async fn count_by_rei(&self, rei_id: Uuid) -> Result<usize, DomainError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM documents WHERE rei_id = $1")
                .bind(rei_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(count as usize)
    }
}
