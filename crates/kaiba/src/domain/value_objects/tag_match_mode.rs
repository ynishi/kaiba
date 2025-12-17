//! TagMatchMode - How to match tags in search queries

use serde::{Deserialize, Serialize};

/// Tag matching mode for search filtering
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TagMatchMode {
    /// Match any of the specified tags (OR)
    #[default]
    Any,
    /// Match all of the specified tags (AND)
    All,
}
