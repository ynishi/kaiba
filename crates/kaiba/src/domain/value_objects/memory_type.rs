//! MemoryType - Classification of memory content

use serde::{Deserialize, Serialize};

/// Memory type classification
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    #[default]
    Conversation,
    Learning,
    Fact,
    Expertise,
    Reflection,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "conversation"),
            MemoryType::Learning => write!(f, "learning"),
            MemoryType::Fact => write!(f, "fact"),
            MemoryType::Expertise => write!(f, "expertise"),
            MemoryType::Reflection => write!(f, "reflection"),
        }
    }
}

impl std::str::FromStr for MemoryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "conversation" => Ok(MemoryType::Conversation),
            "learning" => Ok(MemoryType::Learning),
            "fact" => Ok(MemoryType::Fact),
            "expertise" => Ok(MemoryType::Expertise),
            "reflection" => Ok(MemoryType::Reflection),
            _ => Err(format!("Unknown memory type: {}", s)),
        }
    }
}
