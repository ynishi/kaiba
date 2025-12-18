//! ReiWebhook Repository Port
//!
//! Abstract interface for ReiWebhook persistence operations.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::{ReiWebhook, WebhookDelivery, WebhookEventType};
use crate::domain::errors::DomainError;

/// Repository interface for ReiWebhook entities
#[async_trait]
pub trait ReiWebhookRepository: Send + Sync {
    /// Find a webhook by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ReiWebhook>, DomainError>;

    /// Find all webhooks for a Rei
    async fn find_by_rei(&self, rei_id: Uuid) -> Result<Vec<ReiWebhook>, DomainError>;

    /// Find all enabled webhooks for a Rei that subscribe to a specific event
    async fn find_by_rei_and_event(
        &self,
        rei_id: Uuid,
        event: &WebhookEventType,
    ) -> Result<Vec<ReiWebhook>, DomainError>;

    /// Save a webhook (insert or update)
    async fn save(&self, webhook: &ReiWebhook) -> Result<ReiWebhook, DomainError>;

    /// Delete a webhook by ID
    async fn delete(&self, id: Uuid) -> Result<bool, DomainError>;

    /// Enable/disable a webhook
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<bool, DomainError>;

    // --- Delivery tracking ---

    /// Save a delivery record
    async fn save_delivery(
        &self,
        delivery: &WebhookDelivery,
    ) -> Result<WebhookDelivery, DomainError>;

    /// Find recent deliveries for a webhook
    async fn find_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i32,
    ) -> Result<Vec<WebhookDelivery>, DomainError>;

    /// Find pending deliveries that need retry
    async fn find_pending_deliveries(&self) -> Result<Vec<WebhookDelivery>, DomainError>;
}
