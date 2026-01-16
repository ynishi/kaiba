//! EmphasisStyle - Markdown emphasis semantic weights
//!
//! Maps Markdown emphasis syntax to semantic importance weights
//! for GraphKai emphasis-driven semantic linking.

use serde::{Deserialize, Serialize};

/// Emphasis style with associated semantic weight
///
/// Weight values represent the semantic importance of emphasized text:
/// - Bold (`**text**`): High importance, key concepts (1.0)
/// - Italic (`*text*`): Medium importance, supporting details (0.7)
/// - BoldItalic (`***text***`): Maximum importance, critical concepts (1.2)
/// - Code (`` `text` ``): Technical terms, precise definitions (0.8)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EmphasisStyle {
    Bold,
    Italic,
    BoldItalic,
    Code,
}

impl EmphasisStyle {
    /// Returns the semantic weight for this emphasis style
    ///
    /// Used to determine the importance of nodes in the knowledge graph.
    pub fn weight(&self) -> f32 {
        match self {
            EmphasisStyle::Bold => 1.0,
            EmphasisStyle::Italic => 0.7,
            EmphasisStyle::BoldItalic => 1.2,
            EmphasisStyle::Code => 0.8,
        }
    }

    /// Returns true if this style indicates high importance (weight >= 1.0)
    pub fn is_high_importance(&self) -> bool {
        self.weight() >= 1.0
    }
}

impl std::fmt::Display for EmphasisStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmphasisStyle::Bold => write!(f, "bold"),
            EmphasisStyle::Italic => write!(f, "italic"),
            EmphasisStyle::BoldItalic => write!(f, "bold_italic"),
            EmphasisStyle::Code => write!(f, "code"),
        }
    }
}

impl std::str::FromStr for EmphasisStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bold" => Ok(EmphasisStyle::Bold),
            "italic" => Ok(EmphasisStyle::Italic),
            "bold_italic" | "bolditalic" => Ok(EmphasisStyle::BoldItalic),
            "code" => Ok(EmphasisStyle::Code),
            _ => Err(format!("Unknown emphasis style: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weights() {
        assert_eq!(EmphasisStyle::Bold.weight(), 1.0);
        assert_eq!(EmphasisStyle::Italic.weight(), 0.7);
        assert_eq!(EmphasisStyle::BoldItalic.weight(), 1.2);
        assert_eq!(EmphasisStyle::Code.weight(), 0.8);
    }

    #[test]
    fn test_high_importance() {
        assert!(EmphasisStyle::Bold.is_high_importance());
        assert!(!EmphasisStyle::Italic.is_high_importance());
        assert!(EmphasisStyle::BoldItalic.is_high_importance());
        assert!(!EmphasisStyle::Code.is_high_importance());
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "bold".parse::<EmphasisStyle>().unwrap(),
            EmphasisStyle::Bold
        );
        assert_eq!(
            "ITALIC".parse::<EmphasisStyle>().unwrap(),
            EmphasisStyle::Italic
        );
        assert_eq!(
            "bold_italic".parse::<EmphasisStyle>().unwrap(),
            EmphasisStyle::BoldItalic
        );
        assert_eq!(
            "code".parse::<EmphasisStyle>().unwrap(),
            EmphasisStyle::Code
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(EmphasisStyle::Bold.to_string(), "bold");
        assert_eq!(EmphasisStyle::BoldItalic.to_string(), "bold_italic");
    }
}
