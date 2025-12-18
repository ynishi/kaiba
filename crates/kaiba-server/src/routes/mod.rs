//! Kaiba API Routes
//!
//! - /kaiba/rei - Rei (霊) management
//! - /kaiba/tei - Tei (体) management
//! - /kaiba/rei/:id/call - LLM invocation
//! - /kaiba/rei/:id/memories - Memory storage (MemoryKai/Qdrant)
//! - /kaiba/rei/:id/webhooks - Webhook management (外界へのアクション)
//! - /kaiba/search - Web search (Gemini)
//! - /kaiba/rei/:id/learn - Self-learning (自己活動)

pub mod call;
pub mod learning;
pub mod memory;
pub mod prompt;
pub mod rei;
pub mod search;
pub mod swagger;
pub mod tei;
pub mod webhook;
