//! Domain Entities
//!
//! Pure domain models without infrastructure dependencies.
//! - Rei (霊): Persistent persona identity
//! - Tei (体): Execution interface with expertise
//! - Memory: Long-term storage
//! - Call: LLM invocation record
//! - Prompt: Prompt templates
//! - Message: Platform integration message

mod call;
mod memory;
mod message;
mod prompt;
mod rei;
mod tei;

pub use call::*;
pub use memory::*;
pub use message::*;
pub use prompt::*;
pub use rei::*;
pub use tei::*;
