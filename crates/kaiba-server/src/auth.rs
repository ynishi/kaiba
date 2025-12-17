//! Simple API Key Authentication (Bearer Token)

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

/// API Key from environment/secrets
static API_KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Initialize the API key
pub fn init_api_key(key: String) {
    let _ = API_KEY.set(key);
}

/// Get the API key
fn get_api_key() -> Option<&'static str> {
    API_KEY.get().map(|s| s.as_str())
}

/// Authentication middleware
/// Validates Bearer token against the API key
pub async fn auth_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    // Get API key
    let api_key = match get_api_key() {
        Some(key) if !key.is_empty() => key,
        _ => {
            // No API key configured = auth disabled (for development)
            tracing::warn!("No API key configured, authentication disabled");
            return Ok(next.run(request).await);
        }
    };

    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..]; // Remove "Bearer " prefix
            if token == api_key {
                Ok(next.run(request).await)
            } else {
                tracing::warn!("Invalid API key attempted");
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        Some(_) => {
            tracing::warn!("Invalid Authorization header format");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            tracing::warn!("Missing Authorization header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
