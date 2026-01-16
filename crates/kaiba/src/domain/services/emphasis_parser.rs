//! EmphasisParser - Markdown emphasis extraction service
//!
//! Parses Markdown documents to extract emphasized text (bold, italic, code)
//! and generates EmphasisNodes with contextual information for GraphKai.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use uuid::Uuid;

use crate::domain::entities::{EmphasisNode, EmphasisParseResult, TextPosition};
use crate::domain::value_objects::EmphasisStyle;

/// Configuration for emphasis parsing
#[derive(Debug, Clone)]
pub struct EmphasisParserConfig {
    /// Number of characters to include before/after emphasis for context
    pub context_chars: usize,
    /// Minimum text length to consider for emphasis extraction
    pub min_text_length: usize,
}

impl Default for EmphasisParserConfig {
    fn default() -> Self {
        Self {
            context_chars: 200, // ~50 tokens worth of context
            min_text_length: 2,
        }
    }
}

/// Service for parsing emphasis from Markdown documents
pub struct EmphasisParser {
    config: EmphasisParserConfig,
}

impl EmphasisParser {
    pub fn new() -> Self {
        Self {
            config: EmphasisParserConfig::default(),
        }
    }

    pub fn with_config(config: EmphasisParserConfig) -> Self {
        Self { config }
    }

    /// Parse a Markdown document and extract all emphasis nodes
    pub fn parse(&self, doc_id: Uuid, content: &str) -> EmphasisParseResult {
        let mut result = EmphasisParseResult::new();
        let parser = Parser::new(content);

        // Track current emphasis state
        let mut in_bold = false;
        let mut in_italic = false;
        let mut current_text = String::new();
        let mut text_start_offset: usize = 0;

        for (event, range) in parser.into_offset_iter() {
            match event {
                Event::Start(Tag::Strong) => {
                    in_bold = true;
                    current_text.clear();
                    text_start_offset = range.start;
                }
                Event::End(TagEnd::Strong) => {
                    if !current_text.is_empty()
                        && current_text.len() >= self.config.min_text_length
                    {
                        let style = if in_italic {
                            EmphasisStyle::BoldItalic
                        } else {
                            EmphasisStyle::Bold
                        };
                        let position = self.calculate_position(content, text_start_offset);
                        let contextual_text =
                            self.extract_context(content, text_start_offset, range.end);

                        result.add_node(EmphasisNode::new(
                            doc_id,
                            current_text.clone(),
                            style,
                            position,
                            contextual_text,
                        ));
                    }
                    in_bold = false;
                    current_text.clear();
                }
                Event::Start(Tag::Emphasis) => {
                    if !in_bold {
                        current_text.clear();
                        text_start_offset = range.start;
                    }
                    in_italic = true;
                }
                Event::End(TagEnd::Emphasis) => {
                    if !in_bold
                        && !current_text.is_empty()
                        && current_text.len() >= self.config.min_text_length
                    {
                        let position = self.calculate_position(content, text_start_offset);
                        let contextual_text =
                            self.extract_context(content, text_start_offset, range.end);

                        result.add_node(EmphasisNode::new(
                            doc_id,
                            current_text.clone(),
                            EmphasisStyle::Italic,
                            position,
                            contextual_text,
                        ));
                    }
                    in_italic = false;
                    if !in_bold {
                        current_text.clear();
                    }
                }
                Event::Code(code) => {
                    if code.len() >= self.config.min_text_length {
                        let position = self.calculate_position(content, range.start);
                        let contextual_text =
                            self.extract_context(content, range.start, range.end);

                        result.add_node(EmphasisNode::new(
                            doc_id,
                            code.to_string(),
                            EmphasisStyle::Code,
                            position,
                            contextual_text,
                        ));
                    }
                }
                Event::Text(text) => {
                    if in_bold || in_italic {
                        current_text.push_str(&text);
                    }
                }
                _ => {}
            }
        }

        result
    }

    /// Calculate line/column position from byte offset
    fn calculate_position(&self, content: &str, byte_offset: usize) -> TextPosition {
        let prefix = &content[..byte_offset.min(content.len())];
        let line = prefix.matches('\n').count() + 1;
        let column = prefix
            .rfind('\n')
            .map(|pos| byte_offset - pos)
            .unwrap_or(byte_offset + 1);

        TextPosition::new(byte_offset, line, column)
    }

    /// Extract surrounding context for an emphasis
    fn extract_context(&self, content: &str, start: usize, end: usize) -> String {
        let context_start = start.saturating_sub(self.config.context_chars);
        let context_end = (end + self.config.context_chars).min(content.len());

        // Find word boundaries
        let actual_start = content[..context_start]
            .rfind(char::is_whitespace)
            .map(|p| p + 1)
            .unwrap_or(context_start);

        let actual_end = content[context_end..]
            .find(char::is_whitespace)
            .map(|p| context_end + p)
            .unwrap_or(context_end);

        content[actual_start..actual_end].to_string()
    }
}

impl Default for EmphasisParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bold() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "This is **important** text.";

        let result = parser.parse(doc_id, content);

        assert_eq!(result.bold_count, 1);
        assert_eq!(result.nodes[0].text, "important");
        assert_eq!(result.nodes[0].style, EmphasisStyle::Bold);
    }

    #[test]
    fn test_parse_italic() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "This is *emphasized* text.";

        let result = parser.parse(doc_id, content);

        assert_eq!(result.italic_count, 1);
        assert_eq!(result.nodes[0].text, "emphasized");
        assert_eq!(result.nodes[0].style, EmphasisStyle::Italic);
    }

    #[test]
    fn test_parse_bold_italic() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "This is ***critical*** concept.";

        let result = parser.parse(doc_id, content);

        // pulldown-cmark parses ***text*** as Strong containing Emphasis
        assert_eq!(result.total_count(), 1);
        assert_eq!(result.nodes[0].text, "critical");
        assert_eq!(result.nodes[0].style, EmphasisStyle::BoldItalic);
    }

    #[test]
    fn test_parse_code() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "Use the `println!` macro.";

        let result = parser.parse(doc_id, content);

        assert_eq!(result.code_count, 1);
        assert_eq!(result.nodes[0].text, "println!");
        assert_eq!(result.nodes[0].style, EmphasisStyle::Code);
    }

    #[test]
    fn test_parse_mixed() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = r#"
# GraphKai Architecture

The **knowledge graph** stores *semantic relationships* between concepts.
Use `EmphasisNode` to represent emphasized text.
"#;

        let result = parser.parse(doc_id, content);

        assert_eq!(result.bold_count, 1);
        assert_eq!(result.italic_count, 1);
        assert_eq!(result.code_count, 1);
        assert_eq!(result.total_count(), 3);
    }

    #[test]
    fn test_context_extraction() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "The quick brown fox **jumps** over the lazy dog.";

        let result = parser.parse(doc_id, content);

        assert!(result.nodes[0].contextual_text.contains("jumps"));
        assert!(result.nodes[0].contextual_text.contains("fox"));
        assert!(result.nodes[0].contextual_text.contains("over"));
    }

    #[test]
    fn test_position_tracking() {
        let parser = EmphasisParser::new();
        let doc_id = Uuid::new_v4();
        let content = "Line 1\nLine 2 with **bold** text\nLine 3";

        let result = parser.parse(doc_id, content);

        assert_eq!(result.nodes[0].position.line, 2);
    }

    #[test]
    fn test_min_length_filter() {
        let parser = EmphasisParser::with_config(EmphasisParserConfig {
            context_chars: 200,
            min_text_length: 5,
        });
        let doc_id = Uuid::new_v4();
        let content = "Skip **a** but include **longer text** here.";

        let result = parser.parse(doc_id, content);

        assert_eq!(result.bold_count, 1);
        assert_eq!(result.nodes[0].text, "longer text");
    }
}
