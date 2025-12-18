//! PostgreSQL implementation of ReiWebhookRepository

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use kaiba::{
    DeliveryStatus, DomainError, ReiWebhook, ReiWebhookRepository, WebhookDelivery,
    WebhookEventType, WebhookPayload,
};

/// PostgreSQL implementation of ReiWebhookRepository
pub struct PgReiWebhookRepository {
    pool: PgPool,
}

impl PgReiWebhookRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Internal row type for sqlx mapping
#[derive(sqlx::FromRow)]
struct ReiWebhookRow {
    id: Uuid,
    rei_id: Uuid,
    name: String,
    url: String,
    secret: Option<String>,
    enabled: bool,
    events: serde_json::Value,
    headers: serde_json::Value,
    max_retries: i32,
    timeout_ms: i32,
    payload_format: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ReiWebhookRow> for ReiWebhook {
    fn from(row: ReiWebhookRow) -> Self {
        let events: Vec<WebhookEventType> =
            serde_json::from_value(row.events).unwrap_or_else(|_| vec![WebhookEventType::All]);

        Self {
            id: row.id,
            rei_id: row.rei_id,
            name: row.name,
            url: row.url,
            secret: row.secret,
            enabled: row.enabled,
            events,
            headers: row.headers,
            max_retries: row.max_retries,
            timeout_ms: row.timeout_ms,
            payload_format: row.payload_format,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct WebhookDeliveryRow {
    id: Uuid,
    webhook_id: Uuid,
    payload: serde_json::Value,
    status: String,
    status_code: Option<i32>,
    response_body: Option<String>,
    attempts: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<WebhookDeliveryRow> for WebhookDelivery {
    fn from(row: WebhookDeliveryRow) -> Self {
        let payload: WebhookPayload =
            serde_json::from_value(row.payload).expect("Invalid payload JSON");

        let status = match row.status.as_str() {
            "success" => DeliveryStatus::Success,
            "failed" => DeliveryStatus::Failed,
            "retrying" => DeliveryStatus::Retrying,
            _ => DeliveryStatus::Pending,
        };

        Self {
            id: row.id,
            webhook_id: row.webhook_id,
            payload,
            status,
            status_code: row.status_code,
            response_body: row.response_body,
            attempts: row.attempts,
            created_at: row.created_at,
            completed_at: row.completed_at,
        }
    }
}

fn delivery_status_to_string(status: &DeliveryStatus) -> &'static str {
    match status {
        DeliveryStatus::Pending => "pending",
        DeliveryStatus::Success => "success",
        DeliveryStatus::Failed => "failed",
        DeliveryStatus::Retrying => "retrying",
    }
}

#[async_trait]
impl ReiWebhookRepository for PgReiWebhookRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ReiWebhook>, DomainError> {
        let row = sqlx::query_as::<_, ReiWebhookRow>("SELECT * FROM rei_webhooks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<ReiWebhook>, DomainError> {
        let rows = sqlx::query_as::<_, ReiWebhookRow>(
            "SELECT * FROM rei_webhooks WHERE rei_id = $1 ORDER BY created_at DESC",
        )
        .bind(rei_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_rei_and_event(
        &self,
        rei_id: Uuid,
        event: &WebhookEventType,
    ) -> Result<Vec<ReiWebhook>, DomainError> {
        // Get all enabled webhooks for this Rei, then filter by event type
        let rows = sqlx::query_as::<_, ReiWebhookRow>(
            "SELECT * FROM rei_webhooks WHERE rei_id = $1 AND enabled = true",
        )
        .bind(rei_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        let webhooks: Vec<ReiWebhook> = rows
            .into_iter()
            .map(Into::into)
            .filter(|w: &ReiWebhook| w.should_receive(event))
            .collect();

        Ok(webhooks)
    }

    async fn save(&self, webhook: &ReiWebhook) -> Result<ReiWebhook, DomainError> {
        let events_json = serde_json::to_value(&webhook.events)
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        // Check if exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM rei_webhooks WHERE id = $1)",
        )
        .bind(webhook.id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        let row = if exists {
            // Update
            sqlx::query_as::<_, ReiWebhookRow>(
                r#"
                UPDATE rei_webhooks
                SET name = $2, url = $3, secret = $4, enabled = $5, events = $6,
                    headers = $7, max_retries = $8, timeout_ms = $9, payload_format = $10, updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(webhook.id)
            .bind(&webhook.name)
            .bind(&webhook.url)
            .bind(&webhook.secret)
            .bind(webhook.enabled)
            .bind(&events_json)
            .bind(&webhook.headers)
            .bind(webhook.max_retries)
            .bind(webhook.timeout_ms)
            .bind(&webhook.payload_format)
            .fetch_one(&self.pool)
            .await
        } else {
            // Insert
            sqlx::query_as::<_, ReiWebhookRow>(
                r#"
                INSERT INTO rei_webhooks (id, rei_id, name, url, secret, enabled, events, headers, max_retries, timeout_ms, payload_format)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                RETURNING *
                "#,
            )
            .bind(webhook.id)
            .bind(webhook.rei_id)
            .bind(&webhook.name)
            .bind(&webhook.url)
            .bind(&webhook.secret)
            .bind(webhook.enabled)
            .bind(&events_json)
            .bind(&webhook.headers)
            .bind(webhook.max_retries)
            .bind(webhook.timeout_ms)
            .bind(&webhook.payload_format)
            .fetch_one(&self.pool)
            .await
        }
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, DomainError> {
        let result = sqlx::query("DELETE FROM rei_webhooks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<bool, DomainError> {
        let result =
            sqlx::query("UPDATE rei_webhooks SET enabled = $2, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .bind(enabled)
                .execute(&self.pool)
                .await
                .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn save_delivery(
        &self,
        delivery: &WebhookDelivery,
    ) -> Result<WebhookDelivery, DomainError> {
        let payload_json = serde_json::to_value(&delivery.payload)
            .map_err(|e| DomainError::Repository(e.to_string()))?;
        let status = delivery_status_to_string(&delivery.status);

        // Check if exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM webhook_deliveries WHERE id = $1)",
        )
        .bind(delivery.id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        let row = if exists {
            // Update
            sqlx::query_as::<_, WebhookDeliveryRow>(
                r#"
                UPDATE webhook_deliveries
                SET status = $2, status_code = $3, response_body = $4, attempts = $5, completed_at = $6
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(delivery.id)
            .bind(status)
            .bind(delivery.status_code)
            .bind(&delivery.response_body)
            .bind(delivery.attempts)
            .bind(delivery.completed_at)
            .fetch_one(&self.pool)
            .await
        } else {
            // Insert
            sqlx::query_as::<_, WebhookDeliveryRow>(
                r#"
                INSERT INTO webhook_deliveries (id, webhook_id, payload, status, status_code, response_body, attempts, completed_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING *
                "#,
            )
            .bind(delivery.id)
            .bind(delivery.webhook_id)
            .bind(&payload_json)
            .bind(status)
            .bind(delivery.status_code)
            .bind(&delivery.response_body)
            .bind(delivery.attempts)
            .bind(delivery.completed_at)
            .fetch_one(&self.pool)
            .await
        }
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(row.into())
    }

    async fn find_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i32,
    ) -> Result<Vec<WebhookDelivery>, DomainError> {
        let rows = sqlx::query_as::<_, WebhookDeliveryRow>(
            "SELECT * FROM webhook_deliveries WHERE webhook_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(webhook_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_pending_deliveries(&self) -> Result<Vec<WebhookDelivery>, DomainError> {
        let rows = sqlx::query_as::<_, WebhookDeliveryRow>(
            "SELECT * FROM webhook_deliveries WHERE status IN ('pending', 'retrying') ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Repository(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
