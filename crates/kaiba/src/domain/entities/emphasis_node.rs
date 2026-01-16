//! EmphasisNode - Extracted emphasis from Markdown documents
//!
//! Represents a piece of emphasized text with its semantic context,
//! used for GraphKai emphasis-driven semantic linking.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_objects::EmphasisStyle;

/// Position information within a document
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextPosition {
    /// Byte offset from start of document
    pub byte_offset: usize,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
}

impl TextPosition {
    pub fn new(byte_offset: usize, line: usize, column: usize) -> Self {
        Self {
            byte_offset,
            line,
            column,
        }
    }
}

/// An extracted emphasis node from a Markdown document
///
/// Contains the emphasized text, its style/weight, position in the source,
/// and contextual text for embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmphasisNode {
    /// Unique identifier
    pub id: Uuid,
    /// Source document ID
    pub doc_id: Uuid,
    /// The emphasized text content
    pub text: String,
    /// Emphasis style (bold, italic, etc.)
    pub style: EmphasisStyle,
    /// Position in source document
    pub position: TextPosition,
    /// Surrounding context (±50 tokens by default)
    pub contextual_text: String,
}

impl EmphasisNode {
    /// Create a new EmphasisNode
    pub fn new(
        doc_id: Uuid,
        text: String,
        style: EmphasisStyle,
        position: TextPosition,
        contextual_text: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            doc_id,
            text,
            style,
            position,
            contextual_text,
        }
    }

    /// Returns the semantic weight of this node
    pub fn weight(&self) -> f32 {
        self.style.weight()
    }

    /// Returns true if this is a high-importance node
    pub fn is_high_importance(&self) -> bool {
        self.style.is_high_importance()
    }

    /// Generate text for embedding (contextual text with emphasis markers)
    pub fn embedding_text(&self) -> String {
        // Include both the contextual text and the emphasized text for better embeddings
        format!("{}\n[Emphasis: {}]", self.contextual_text, self.text)
    }
}

/// Result of parsing emphasis from a document
#[derive(Debug, Clone, Default)]
pub struct EmphasisParseResult {
    /// All extracted emphasis nodes
    pub nodes: Vec<EmphasisNode>,
    /// Total count by style
    pub bold_count: usize,
    pub italic_count: usize,
    pub bold_italic_count: usize,
    pub code_count: usize,
}

impl EmphasisParseResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: EmphasisNode) {
        match node.style {
            EmphasisStyle::Bold => self.bold_count += 1,
            EmphasisStyle::Italic => self.italic_count += 1,
            EmphasisStyle::BoldItalic => self.bold_italic_count += 1,
            EmphasisStyle::Code => self.code_count += 1,
        }
        self.nodes.push(node);
    }

    pub fn total_count(&self) -> usize {
        self.nodes.len()
    }

    /// Filter nodes by minimum weight threshold
    pub fn filter_by_weight(&self, min_weight: f32) -> Vec<&EmphasisNode> {
        self.nodes
            .iter()
            .filter(|n| n.weight() >= min_weight)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emphasis_node_creation() {
        let doc_id = Uuid::new_v4();
        let node = EmphasisNode::new(
            doc_id,
            "important concept".to_string(),
            EmphasisStyle::Bold,
            TextPosition::new(100, 5, 10),
            "This is an important concept in the domain.".to_string(),
        );

        assert_eq!(node.doc_id, doc_id);
        assert_eq!(node.text, "important concept");
        assert_eq!(node.weight(), 1.0);
        assert!(node.is_high_importance());
    }

    #[test]
    fn test_parse_result_filter() {
        let doc_id = Uuid::new_v4();
        let mut result = EmphasisParseResult::new();

        result.add_node(EmphasisNode::new(
            doc_id,
            "bold text".to_string(),
            EmphasisStyle::Bold,
            TextPosition::new(0, 1, 1),
            "context".to_string(),
        ));

        result.add_node(EmphasisNode::new(
            doc_id,
            "italic text".to_string(),
            EmphasisStyle::Italic,
            TextPosition::new(50, 2, 1),
            "context".to_string(),
        ));

        assert_eq!(result.total_count(), 2);
        assert_eq!(result.bold_count, 1);
        assert_eq!(result.italic_count, 1);

        let high_importance = result.filter_by_weight(1.0);
        assert_eq!(high_importance.len(), 1);
        assert_eq!(high_importance[0].text, "bold text");
    }
}
