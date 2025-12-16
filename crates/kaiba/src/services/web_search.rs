//! Google Web Search agent that leverages Gemini's `google_search` tool.
//!
//! Based on orcs implementation - uses Gemini API with grounding.

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";

/// Agent capable of calling Gemini with the google_search tool.
#[derive(Clone)]
pub struct WebSearchAgent {
    client: Client,
    api_key: String,
    model: String,
}

impl WebSearchAgent {
    /// Creates a new agent using the provided API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Overrides the Gemini model name if needed.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Execute a web search query
    pub async fn search(&self, query: &str) -> Result<WebSearchResponse, WebSearchError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err(WebSearchError::EmptyQuery);
        }

        self.perform_search(trimmed).await
    }

    async fn perform_search(&self, query: &str) -> Result<WebSearchResponse, WebSearchError> {
        let url = format!(
            "{}/{model}:generateContent?key={api_key}",
            BASE_URL,
            model = self.model,
            api_key = self.api_key
        );

        let request = GenerateContentRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part {
                    text: query.to_string(),
                }],
            }],
            tools: vec![Tool::default()],
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|err| WebSearchError::RequestFailed(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(map_http_error(status, body));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|err| WebSearchError::ParseError(err.to_string()))?;

        let answer = extract_answer(&payload)
            .unwrap_or_else(|| "Google Search returned no answer".to_string());
        let references = extract_references(&payload);

        Ok(WebSearchResponse {
            query: query.to_string(),
            answer,
            references,
        })
    }
}

// ============================================
// Request/Response Types
// ============================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentRequest {
    contents: Vec<Content>,
    tools: Vec<Tool>,
}

#[derive(Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize, Default)]
struct Tool {
    #[serde(rename = "google_search")]
    google_search: GoogleSearchConfig,
}

#[derive(Serialize, Default)]
struct GoogleSearchConfig {}

/// Structured reference returned by Gemini's grounding metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchReference {
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Search response returned to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResponse {
    pub query: String,
    pub answer: String,
    pub references: Vec<WebSearchReference>,
}

/// Web search error types
#[derive(Debug, Clone)]
pub enum WebSearchError {
    EmptyQuery,
    RequestFailed(String),
    ParseError(String),
    ApiError { status: u16, message: String },
    RateLimited { retry_after: Option<Duration> },
}

impl std::fmt::Display for WebSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSearchError::EmptyQuery => write!(f, "Search query cannot be empty"),
            WebSearchError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            WebSearchError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WebSearchError::ApiError { status, message } => {
                write!(f, "API error ({}): {}", status, message)
            }
            WebSearchError::RateLimited { retry_after } => {
                if let Some(duration) = retry_after {
                    write!(f, "Rate limited, retry after {:?}", duration)
                } else {
                    write!(f, "Rate limited")
                }
            }
        }
    }
}

impl std::error::Error for WebSearchError {}

// ============================================
// Helper Functions
// ============================================

fn extract_answer(root: &Value) -> Option<String> {
    let candidates = root.get("candidates")?.as_array()?;

    let mut collected = Vec::new();
    for candidate in candidates {
        if let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
        {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        collected.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    if collected.is_empty() {
        None
    } else {
        Some(collected.join("\n\n"))
    }
}

fn extract_references(root: &Value) -> Vec<WebSearchReference> {
    let mut seen = HashSet::new();
    let mut references = Vec::new();

    let candidates = match root.get("candidates").and_then(|c| c.as_array()) {
        Some(list) => list,
        None => return references,
    };

    for candidate in candidates {
        let metadata = match candidate.get("groundingMetadata") {
            Some(value) => value,
            None => continue,
        };

        let chunks = match metadata
            .get("groundingChunks")
            .and_then(|chunks| chunks.as_array())
        {
            Some(list) => list,
            None => continue,
        };

        for chunk in chunks {
            let web = chunk
                .get("web")
                .or_else(|| chunk.get("webSearch"))
                .or_else(|| chunk.get("retrievedReference"));

            let Some(web_obj) = web else {
                continue;
            };

            let url = web_obj
                .get("uri")
                .or_else(|| web_obj.get("url"))
                .and_then(|v| v.as_str());
            let Some(url) = url else {
                continue;
            };

            if !seen.insert(url.to_string()) {
                continue;
            }

            let title = web_obj
                .get("title")
                .or_else(|| web_obj.get("pageTitle"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| url.to_string());

            let snippet = web_obj
                .get("snippet")
                .or_else(|| web_obj.get("text"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let source = web_obj
                .get("siteName")
                .or_else(|| web_obj.get("source"))
                .or_else(|| web_obj.get("displayUri"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            references.push(WebSearchReference {
                title,
                url: url.to_string(),
                snippet,
                source,
            });
        }
    }

    references
}

fn map_http_error(status: StatusCode, body: String) -> WebSearchError {
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|json| {
            json.get("error")
                .and_then(|err| err.get("message"))
                .and_then(|msg| msg.as_str())
                .map(|msg| msg.to_string())
        })
        .unwrap_or_else(|| body.clone());

    if status == StatusCode::TOO_MANY_REQUESTS {
        return WebSearchError::RateLimited { retry_after: None };
    }

    WebSearchError::ApiError {
        status: status.as_u16(),
        message,
    }
}
