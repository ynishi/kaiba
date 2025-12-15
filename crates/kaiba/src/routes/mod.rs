//! Kaiba API Routes
//!
//! - /kaiba/rei - Rei (霊) management
//! - /kaiba/tei - Tei (体) management
//! - /kaiba/rei/:id/call - LLM invocation
//! - /kaiba/rei/:id/memories - Memory storage (MemoryKai/Qdrant)

pub mod rei;
pub mod tei;
pub mod call;
pub mod memory;
