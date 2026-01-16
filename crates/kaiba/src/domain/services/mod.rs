//! Domain Services
//!
//! Pure business logic that doesn't fit into entities.
//! These services orchestrate domain operations without external dependencies.

mod emphasis_parser;
mod graph_builder;

pub use emphasis_parser::*;
pub use graph_builder::*;

// TODO: Move decision, digest, self_learning logic here
// For now, these remain in the application layer until refactored
