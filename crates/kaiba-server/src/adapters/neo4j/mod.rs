//! Neo4j Adapters - Graph database integration for GraphKai

mod graph_repository;
#[cfg(test)]
mod tests;

pub use graph_repository::Neo4jGraphRepository;
