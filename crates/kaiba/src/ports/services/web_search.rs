//! Web Search Service Port
//!
//! Abstract interface for web search operations.

use async_trait::async_trait;

use crate::domain::errors::DomainError;

/// Search result from web search
#[derive(Debug, Clone)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Service interface for web search operations
#[async_trait]
pub trait WebSearchService: Send + Sync {
    /// Search the web for a query
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<WebSearchResult>, DomainError>;
}
