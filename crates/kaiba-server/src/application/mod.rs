//! Application Layer (Use Cases)
//!
//! Orchestrates domain operations and coordinates between
//! repositories and external services.

mod rei_service;
mod tei_service;

pub use rei_service::ReiService;
pub use tei_service::TeiService;
