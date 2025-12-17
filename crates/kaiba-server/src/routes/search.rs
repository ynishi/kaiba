//! Search Routes - Web search via Gemini

use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::services::web_search::{WebSearchReference, WebSearchResponse};
use crate::AppState;

/// Search request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub query: String,
}

/// Search response (simplified)
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResult {
    pub query: String,
    pub answer: String,
    pub references: Vec<WebSearchReference>,
}

impl From<WebSearchResponse> for SearchResult {
    fn from(res: WebSearchResponse) -> Self {
        Self {
            query: res.query,
            answer: res.answer,
            references: res.references,
        }
    }
}

/// Execute web search
#[utoipa::path(
    post,
    path = "/kaiba/search",
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Search results", body = SearchResult),
        (status = 503, description = "WebSearch not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Search"
)]
pub async fn web_search(
    State(state): State<AppState>,
    Json(payload): Json<SearchRequest>,
) -> Result<Json<SearchResult>, (axum::http::StatusCode, String)> {
    let agent = state.web_search.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "WebSearch not available".to_string(),
    ))?;

    let result = agent
        .search(&payload.query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        "🔍 WebSearch: {} -> {} references",
        payload.query,
        result.references.len()
    );

    Ok(Json(result.into()))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/kaiba/search", post(web_search))
}
