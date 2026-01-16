//! Domain Entities
//!
//! Pure domain models without infrastructure dependencies.
//! - Rei (霊): Persistent persona identity
//! - Tei (体): Execution interface with expertise
//! - Memory: Long-term storage
//! - Document: Raw Markdown source (GraphKai)
//! - Call: LLM invocation record
//! - Prompt: Prompt templates
//! - Message: Platform integration message
//! - Webhook: Outbound webhook for external actions

mod call;
mod document;
mod memory;
mod message;
mod prompt;
mod rei;
mod tei;
mod webhook;

pub use call::*;
pub use document::*;
pub use memory::*;
pub use message::*;
pub use prompt::*;
pub use rei::*;
pub use tei::*;
pub use webhook::*;
