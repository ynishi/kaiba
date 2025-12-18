//! Kaiba Data Models
//!
//! - Rei (霊): Persistent persona identity
//! - Tei (体): Execution interface with expertise
//! - Memory: Long-term storage
//! - Call: LLM invocation
//! - Webhook: Outbound webhook configuration

mod call;
mod memory;
mod prompt;
mod rei;
mod tei;
mod webhook;

pub use call::*;
pub use memory::*;
pub use prompt::*;
pub use rei::*;
pub use tei::*;
pub use webhook::*;
