//! Service Ports
//!
//! Abstract interfaces for external services.
//! These are "Tei" (体) implementations - execution interfaces
//! that can be swapped between different providers.

mod embedding;
mod llm_provider;
mod web_search;

pub use embedding::*;
pub use llm_provider::*;
pub use web_search::*;
