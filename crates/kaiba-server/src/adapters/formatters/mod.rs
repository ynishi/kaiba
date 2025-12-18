//! Webhook Payload Formatters
//!
//! Transform WebhookPayload into integration-specific formats.

mod github_issue;

pub use github_issue::format_as_github_issue;

use kaiba::WebhookPayload;

/// Format a webhook payload based on the specified format type
pub fn format_payload(format: Option<&str>, payload: &WebhookPayload) -> serde_json::Value {
    match format {
        Some("github_issue") => format_as_github_issue(payload),
        _ => serde_json::to_value(payload).unwrap_or_default(),
    }
}
