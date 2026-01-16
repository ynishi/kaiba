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
    // GraphKai - Graph builder
    BuildStats,
    // Entities
    Call,
    DeliveryStatus,
    Document,
    DomainError,
    // GraphKai - Graph types
    EdgeType,
    // GraphKai - Emphasis types
    EmphasisNode,
    EmphasisParseResult,
    EmphasisParser,
    EmphasisParserConfig,
    EmphasisStyle,
    // GraphKai - Configuration
    EmphasisWeights,
    GraphBuildResult,
    GraphBuilder,
    GraphEdge,
    GraphNode,
    GraphPath,
    LinkageConfig,
    LinkageStrategy,
    Memory,
    MemoryType,
    Message,
    NodeType,
    Prompt,
    Provider,
    Rei,
    ReiState,
    ReiTei,
    ReiWebhook,
    SearchConfig,
    SearchStrategy,
    TagMatchMode,
    Tei,
    TextPosition,
    WebhookDelivery,
    WebhookEventType,
    WebhookPayload,
};
pub use ports::{
    // Tei Services (体 - execution interfaces)
    ChatMessage,
    CompletionOptions,
    CompletionResponse,
    // Repositories
    DeleteBatchResult,
    DocRepository,
    DocumentSaveResult,
    // GraphKai - Graph repository
    EdgeBatchResult,
    EmbeddingService,
    GraphRepository,
    GraphStats,
    IntegrationConfig,
    IntegrationEvent,
    MemoryRepository,
    MemorySearchFilter,
    MessageRole,
    NodeBatchResult,
    ReiRepository,
    ReiWebhookRepository,
    SaveStatus,
    TeiIntegration,
    TeiLlmProvider,
    TeiRepository,
    TeiWebhook,
    TokenUsage,
    TraversalQuery,
    WebSearchResult,
    WebSearchService,
    WebhookDeliveryConfig,
};
