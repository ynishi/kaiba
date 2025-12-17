//! Domain Entities
//!
//! Pure domain models without infrastructure dependencies.
//! - Rei (霊): Persistent persona identity
//! - Tei (体): Execution interface with expertise
//! - Memory: Long-term storage
//! - Call: LLM invocation record
//! - Prompt: Prompt templates

mod call;
mod memory;
mod prompt;
mod rei;
mod tei;

pub use call::*;
pub use memory::*;
pub use prompt::*;
pub use rei::*;
pub use tei::*;
