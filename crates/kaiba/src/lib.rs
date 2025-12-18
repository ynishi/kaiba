//! Kaiba Domain Library
//!
//! Core domain types and interfaces for the Kaiba AI persona system.
//!
//! # Architecture
//!
//! This crate follows Clean Architecture / Hexagonal Architecture principles:
//!
//! - **Domain Layer** (`domain/`): Pure business entities and logic
//!   - `entities/`: Core domain models (Rei, Tei, Memory, Call, Prompt)
//!   - `value_objects/`: Immutable value types (MemoryType, TagMatchMode)
//!   - `errors/`: Domain-specific error types
//!
//! - **Ports** (`ports/`): Abstract interfaces (traits)
//!   - `repositories/`: Data access interfaces
//!   - `services/`: External service interfaces
//!
//! # Usage
//!
//! ```rust,ignore
//! use kaiba::domain::{Rei, Tei, Memory};
//! use kaiba::ports::{ReiRepository, EmbeddingService};
//! ```

pub mod domain;
pub mod ports;

// Re-export commonly used types
pub use domain::{
    Call, DeliveryStatus, DomainError, Memory, MemoryType, Message, Prompt, Provider, Rei,
    ReiState, ReiTei, ReiWebhook, TagMatchMode, Tei, WebhookDelivery, WebhookEventType,
    WebhookPayload,
};
pub use ports::{
    // Tei Services (体 - execution interfaces)
    ChatMessage,
    CompletionOptions,
    CompletionResponse,
    EmbeddingService,
    IntegrationConfig,
    IntegrationEvent,
    // Repositories
    MemoryRepository,
    MemorySearchFilter,
    MessageRole,
    ReiRepository,
    ReiWebhookRepository,
    TeiIntegration,
    TeiLlmProvider,
    TeiRepository,
    TeiWebhook,
    TokenUsage,
    WebSearchResult,
    WebSearchService,
    WebhookDeliveryConfig,
};
