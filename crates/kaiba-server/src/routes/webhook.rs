//! Webhook Routes - Rei's External Actions
//!
//! HTTP handlers for managing webhooks that enable Rei to interact
//! with the external world.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use kaiba::{ReiWebhook, ReiWebhookRepository, TeiWebhook, WebhookEventType, WebhookPayload};

use crate::models::{
    parse_event_types, CreateWebhookRequest, TriggerWebhookRequest, UpdateWebhookRequest,
    WebhookDeliveryResponse, WebhookResponse,
};
use crate::AppState;

/// List all webhooks for a Rei
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/webhooks",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID")
    ),
    responses(
        (status = 200, description = "List of webhooks", body = Vec<WebhookResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn list_webhooks(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<WebhookResponse>>, (axum::http::StatusCode, String)> {
    let webhooks = state
        .webhook_repo
        .find_by_rei(rei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<WebhookResponse> = webhooks
        .into_iter()
        .map(WebhookResponse::from_domain)
        .collect();

    Ok(Json(responses))
}

/// Create a new webhook for a Rei
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/webhooks",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID")
    ),
    request_body = CreateWebhookRequest,
    responses(
        (status = 200, description = "Webhook created", body = WebhookResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn create_webhook(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookResponse>, (axum::http::StatusCode, String)> {
    let events = parse_event_types(payload.events);

    let mut webhook = ReiWebhook::new(rei_id, payload.name, payload.url).with_events(events);

    if let Some(secret) = payload.secret {
        webhook = webhook.with_secret(secret);
    }
    if let Some(headers) = payload.headers {
        webhook = webhook.with_headers(headers);
    }
    if let Some(max_retries) = payload.max_retries {
        webhook.max_retries = max_retries;
    }
    if let Some(timeout_ms) = payload.timeout_ms {
        webhook.timeout_ms = timeout_ms;
    }
    if let Some(payload_format) = payload.payload_format {
        webhook.payload_format = Some(payload_format);
    }

    let saved = state
        .webhook_repo
        .save(&webhook)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(WebhookResponse::from_domain(saved)))
}

/// Get webhook by ID
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/webhooks/{webhook_id}",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("webhook_id" = Uuid, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Webhook found", body = WebhookResponse),
        (status = 404, description = "Webhook not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn get_webhook(
    State(state): State<AppState>,
    Path((rei_id, webhook_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<WebhookResponse>, (axum::http::StatusCode, String)> {
    let webhook = state
        .webhook_repo
        .find_by_id(webhook_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ))?;

    // Verify webhook belongs to this Rei
    if webhook.rei_id != rei_id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ));
    }

    Ok(Json(WebhookResponse::from_domain(webhook)))
}

/// Update webhook
#[utoipa::path(
    put,
    path = "/kaiba/rei/{rei_id}/webhooks/{webhook_id}",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("webhook_id" = Uuid, Path, description = "Webhook ID")
    ),
    request_body = UpdateWebhookRequest,
    responses(
        (status = 200, description = "Webhook updated", body = WebhookResponse),
        (status = 404, description = "Webhook not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn update_webhook(
    State(state): State<AppState>,
    Path((rei_id, webhook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, (axum::http::StatusCode, String)> {
    let mut webhook = state
        .webhook_repo
        .find_by_id(webhook_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ))?;

    // Verify webhook belongs to this Rei
    if webhook.rei_id != rei_id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ));
    }

    // Apply updates
    if let Some(name) = payload.name {
        webhook.name = name;
    }
    if let Some(url) = payload.url {
        webhook.url = url;
    }
    if let Some(secret) = payload.secret {
        webhook.secret = Some(secret);
    }
    if let Some(enabled) = payload.enabled {
        webhook.enabled = enabled;
    }
    if let Some(events) = payload.events {
        webhook.events = parse_event_types(Some(events));
    }
    if let Some(headers) = payload.headers {
        webhook.headers = headers;
    }
    if let Some(max_retries) = payload.max_retries {
        webhook.max_retries = max_retries;
    }
    if let Some(timeout_ms) = payload.timeout_ms {
        webhook.timeout_ms = timeout_ms;
    }
    if let Some(payload_format) = payload.payload_format {
        webhook.payload_format = Some(payload_format);
    }

    let saved = state
        .webhook_repo
        .save(&webhook)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(WebhookResponse::from_domain(saved)))
}

/// Delete webhook
#[utoipa::path(
    delete,
    path = "/kaiba/rei/{rei_id}/webhooks/{webhook_id}",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("webhook_id" = Uuid, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Webhook deleted"),
        (status = 404, description = "Webhook not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn delete_webhook(
    State(state): State<AppState>,
    Path((_rei_id, webhook_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let deleted = state
        .webhook_repo
        .delete(webhook_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !deleted {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Webhook deleted"
    })))
}

/// Trigger a test webhook delivery
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/webhooks/{webhook_id}/trigger",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("webhook_id" = Uuid, Path, description = "Webhook ID")
    ),
    request_body = TriggerWebhookRequest,
    responses(
        (status = 200, description = "Webhook triggered", body = WebhookDeliveryResponse),
        (status = 404, description = "Webhook not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn trigger_webhook(
    State(state): State<AppState>,
    Path((rei_id, webhook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<TriggerWebhookRequest>,
) -> Result<Json<WebhookDeliveryResponse>, (axum::http::StatusCode, String)> {
    let webhook = state
        .webhook_repo
        .find_by_id(webhook_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ))?;

    // Verify webhook belongs to this Rei
    if webhook.rei_id != rei_id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ));
    }

    // Create test payload
    let event = payload
        .event
        .map(|e| match e.as_str() {
            "response_completed" => WebhookEventType::ResponseCompleted,
            "state_changed" => WebhookEventType::StateChanged,
            "memory_added" => WebhookEventType::MemoryAdded,
            "search_completed" => WebhookEventType::SearchCompleted,
            "learning_completed" => WebhookEventType::LearningCompleted,
            s => WebhookEventType::Custom(s.to_string()),
        })
        .unwrap_or(WebhookEventType::Custom("test".to_string()));

    let data = payload
        .data
        .unwrap_or(serde_json::json!({"test": true, "message": "Test webhook trigger"}));

    let webhook_payload = WebhookPayload::new(event, rei_id, data);

    // Deliver webhook
    let delivery = state
        .http_webhook
        .deliver_with_retry(&webhook, &webhook_payload)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Save delivery record
    let saved_delivery = state
        .webhook_repo
        .save_delivery(&delivery)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(WebhookDeliveryResponse::from_domain(saved_delivery)))
}

/// Get recent deliveries for a webhook
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/webhooks/{webhook_id}/deliveries",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("webhook_id" = Uuid, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "List of deliveries", body = Vec<WebhookDeliveryResponse>),
        (status = 404, description = "Webhook not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Webhook"
)]
pub async fn list_deliveries(
    State(state): State<AppState>,
    Path((rei_id, webhook_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<WebhookDeliveryResponse>>, (axum::http::StatusCode, String)> {
    // Verify webhook exists and belongs to this Rei
    let webhook = state
        .webhook_repo
        .find_by_id(webhook_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ))?;

    if webhook.rei_id != rei_id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Webhook not found".to_string(),
        ));
    }

    let deliveries = state
        .webhook_repo
        .find_deliveries(webhook_id, 50)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<WebhookDeliveryResponse> = deliveries
        .into_iter()
        .map(WebhookDeliveryResponse::from_domain)
        .collect();

    Ok(Json(responses))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/kaiba/rei/:rei_id/webhooks",
            get(list_webhooks).post(create_webhook),
        )
        .route(
            "/kaiba/rei/:rei_id/webhooks/:webhook_id",
            get(get_webhook).put(update_webhook).delete(delete_webhook),
        )
        .route(
            "/kaiba/rei/:rei_id/webhooks/:webhook_id/trigger",
            axum::routing::post(trigger_webhook),
        )
        .route(
            "/kaiba/rei/:rei_id/webhooks/:webhook_id/deliveries",
            get(list_deliveries),
        )
}
