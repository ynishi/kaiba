//! Kaiba Data Models
//!
//! - Rei (霊): Persistent persona identity
//! - Tei (体): Execution interface with expertise
//! - Memory: Long-term storage
//! - Call: LLM invocation

mod rei;
mod tei;
mod memory;
mod call;
mod prompt;

pub use rei::*;
pub use tei::*;
pub use memory::*;
pub use call::*;
pub use prompt::*;
