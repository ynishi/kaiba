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
    Call, DomainError, Memory, MemoryType, Prompt, Provider, Rei, ReiState, ReiTei,
    TagMatchMode, Tei,
};
pub use ports::{
    EmbeddingService, MemoryRepository, MemorySearchFilter, ReiRepository, TeiRepository,
    WebSearchResult, WebSearchService,
};
