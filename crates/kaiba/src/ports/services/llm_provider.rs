//! LLM Provider Port
//!
//! Abstract interface for LLM (Large Language Model) invocations.
//! This is a "Tei" (体) - an execution interface that can be swapped
//! between different providers (Anthropic, OpenAI, Google, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Options for LLM completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop_sequences: Option<Vec<String>>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(4096),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
        }
    }
}

/// Response from LLM completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Generated text content
    pub content: String,
    /// Model that generated the response
    pub model: String,
    /// Token usage statistics
    pub usage: TokenUsage,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens in the prompt
    pub prompt_tokens: u32,
    /// Tokens in the completion
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// LLM Provider interface (Tei - 体)
///
/// This trait abstracts LLM invocations across different providers.
/// Each provider (Anthropic, OpenAI, Google) should have its own implementation.
///
/// # Example
///
/// ```rust,ignore
/// use kaiba::ports::TeiLlmProvider;
///
/// struct AnthropicProvider { /* ... */ }
///
/// #[async_trait]
/// impl TeiLlmProvider for AnthropicProvider {
///     async fn complete(&self, messages: &[ChatMessage], options: &CompletionOptions)
///         -> Result<CompletionResponse, DomainError> {
///         // Call Claude API
///     }
///     // ...
/// }
/// ```
#[async_trait]
pub trait TeiLlmProvider: Send + Sync {
    /// Generate a completion from messages
    async fn complete(
        &self,
        messages: &[ChatMessage],
        options: &CompletionOptions,
    ) -> Result<CompletionResponse, DomainError>;

    /// Generate a simple completion from a single prompt
    async fn complete_simple(&self, prompt: &str) -> Result<String, DomainError> {
        let messages = vec![ChatMessage::user(prompt)];
        let response = self
            .complete(&messages, &CompletionOptions::default())
            .await?;
        Ok(response.content)
    }

    /// Get the provider name (e.g., "anthropic", "openai", "google")
    fn provider_name(&self) -> &str;

    /// Get the model ID being used
    fn model_id(&self) -> &str;

    /// Check if the provider is available and healthy
    async fn health_check(&self) -> Result<bool, DomainError> {
        Ok(true)
    }

    /// Estimate token count for text (provider-specific)
    fn estimate_tokens(&self, text: &str) -> u32 {
        // Rough estimate: ~4 chars per token
        (text.len() / 4) as u32
    }
}

/// Streaming chunk from LLM (for use in streaming implementations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Delta content
    pub content: String,
    /// Whether this is the final chunk
    pub is_final: bool,
}

// Note: Streaming support (TeiLlmProviderStreaming) should be implemented
// in infrastructure crates that can depend on `futures` or `tokio-stream`.
