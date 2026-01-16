//! Repository Ports
//!
//! Abstract interfaces for data persistence operations.

mod doc_repository;
mod memory_repository;
mod rei_repository;
mod tei_repository;
mod webhook_repository;

pub use doc_repository::*;
pub use memory_repository::*;
pub use rei_repository::*;
pub use tei_repository::*;
pub use webhook_repository::*;
