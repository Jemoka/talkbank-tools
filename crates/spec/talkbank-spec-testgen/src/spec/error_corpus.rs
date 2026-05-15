//! # Error Corpus Specification Types
//!
//! Types for error corpus specifications - invalid CHAT examples
//! that should produce parse errors.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Root structure for an error corpus specification file
#[derive(Debug, Deserialize)]
pub struct ErrorCorpusSpec {
    pub metadata: ErrorCorpusMetadata,
    pub examples: Vec<ErrorCorpusExample>,
}

/// Metadata about the error category
#[derive(Debug, Deserialize)]
pub struct ErrorCorpusMetadata {
    /// Error category: "missing_headers", "invalid_headers", "utterance_errors", etc.
    pub category: String,
    /// Human-readable description of this error category
    pub description: String,
    /// Level where errors occur: "file", "utterance", "tier", "word"
    pub level: String,
    /// Error layer: "parser" (grammar-level) or "validation" (semantic-level)
    #[serde(default = "default_layer")]
    pub layer: String,
}

/// Runs default layer.
fn default_layer() -> String {
    "parser".to_string()
}

/// A single error corpus example with invalid input
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorCorpusExample {
    /// Unique name for this example (used in test names)
    pub name: String,
    /// Human-readable description of what's wrong
    pub description: String,
    /// The invalid CHAT input that should produce an error
    pub input: String,
    /// Expected error code (E5xx, E3xx, etc.) - optional, for documentation
    #[serde(default)]
    pub error_code: Option<String>,
    /// Human description of where the error occurs
    #[serde(default)]
    pub error_location: Option<String>,
    /// Additional notes about the error
    #[serde(default)]
    pub notes: Option<String>,
    /// Expected CST showing ERROR nodes (optional, can be auto-generated)
    #[serde(default)]
    pub expected_cst: Option<String>,
}

impl ErrorCorpusSpec {
    /// Load an error corpus specification from a markdown file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        Self::parse_markdown(&content, path)
    }

    /// Load all error corpus specifications from a directory tree
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();

        if !root.exists() {
            return Ok(specs);
        }

        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match Self::load(path) {
                    Ok(spec) => specs.push(spec),
                    Err(e) => eprintln!("Warning: Failed to load {}: {}", path.display(), e),
                }
            }
        }

        Ok(specs)
    }

    /// Parse markdown content into an ErrorCorpusSpec
    fn parse_markdown(content: &str, path: &Path) -> Result<Self, String> {
        /// Enum variants for Section.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Section {
            None,
            Description,
            Metadata,
            Example,
            ExpectedBehavior,
            ChatRule,
            Notes,
        }

        let arena = Arena::new();
        let root = parse_document(&arena, content, &Options::default());

        let mut section = Section::None;
        let mut title = None::<String>;
        let mut description_parts = Vec::new();
        let mut expected_behavior_parts = Vec::new();
        let mut chat_rule_parts = Vec::new();
        let mut notes_parts = Vec::new();

        let mut category = None::<String>;
        let mut level = None::<String>;
        let mut layer = None::<String>;

        let mut input = None::<String>;

        for node in root.descendants() {
            let node_data = node.data.borrow();
            match &node_data.value {
                NodeValue::Heading(heading) if heading.level == 1 => {
                    title = Some(normalize_whitespace(&extract_text_from_children(node)));
                }
                NodeValue::Heading(heading) if heading.level == 2 => {
                    let heading_text = normalize_whitespace(&extract_text_from_children(node));
                    section = if heading_text == "Description" {
                        Section::Description
                    } else if heading_text == "Metadata" {
                        Section::Metadata
                    } else if heading_text == "Example" || heading_text.starts_with("Example ") {
                        Section::Example
                    } else if heading_text == "Expected Behavior" {
                        Section::ExpectedBehavior
                    } else if heading_text == "CHAT Rule" {
                        Section::ChatRule
                    } else if heading_text == "Notes" {
                        Section::Notes
                    } else {
                        Section::None
                    };
                }
                NodeValue::Paragraph => {
                    let text = normalize_whitespace(&extract_text_from_children(node));
                    if text.is_empty() {
                        continue;
                    }
                    match section {
                        Section::Description => description_parts.push(text),
                        Section::ExpectedBehavior => expected_behavior_parts.push(text),
                        Section::ChatRule => chat_rule_parts.push(text),
                        Section::Notes => notes_parts.push(text),
                        _ => {}
                    }
                }
                NodeValue::List(_) if section == Section::Metadata => {
                    for child in node.children() {
                        if let NodeValue::Item(_) = child.data.borrow().value {
                            let mut key = String::new();
                            let mut value = String::new();
                            let mut found_colon = false;

                            for item_node in child.descendants() {
                                // Check if this node is inside a Strong parent
                                let is_in_strong = item_node.parent().is_some_and(|p| {
                                    matches!(p.data.borrow().value, NodeValue::Strong)
                                });

                                match &item_node.data.borrow().value {
                                    NodeValue::Text(text) => {
                                        if is_in_strong {
                                            let mut strong_text = text.to_string();
                                            if strong_text.ends_with(':') {
                                                strong_text.pop();
                                            }
                                            key.push_str(&strong_text);
                                        } else if text.contains(':') && !found_colon {
                                            found_colon = true;
                                            let parts: Vec<&str> = text.splitn(2, ':').collect();
                                            if parts.len() == 2 {
                                                value.push_str(parts[1]);
                                            }
                                        } else if found_colon {
                                            value.push_str(text);
                                        }
                                    }
                                    NodeValue::Code(code) => {
                                        if found_colon {
                                            value.push_str(&code.literal);
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            let key = normalize_whitespace(&key);
                            let value = normalize_whitespace(&value);
                            if key == "Category" {
                                category = Some(value);
                            } else if key == "Level" {
                                level = Some(value);
                            } else if key == "Layer" {
                                layer = Some(value);
                            }
                        }
                    }
                }
                NodeValue::CodeBlock(code_block) if section == Section::Example => {
                    if code_block.info == "chat" {
                        input = Some(strip_single_trailing_newline(&code_block.literal));
                    }
                }
                _ => {}
            }
        }

        let title = title.ok_or_else(|| format!("Missing title in {}", path.display()))?;
        let mut title_parts = title.splitn(2, ':');
        let error_code = title_parts
            .next()
            .ok_or_else(|| format!("Missing error code in title for {}", path.display()))?;
        let name = title_parts
            .next()
            .ok_or_else(|| format!("Missing error title after code in {}", path.display()))?;
        let error_code = normalize_whitespace(error_code);
        let name = normalize_whitespace(name);

        if error_code.is_empty() || name.is_empty() {
            return Err(format!("Invalid title format in {}", path.display()));
        }

        let description = normalize_whitespace(&description_parts.join(" "));
        if description.is_empty() {
            return Err(format!("Missing Description content in {}", path.display()));
        }

        let category = category
            .ok_or_else(|| format!("Missing Category in Metadata in {}", path.display()))?;
        let level =
            level.ok_or_else(|| format!("Missing Level in Metadata in {}", path.display()))?;
        let layer = match layer {
            Some(layer) => layer,
            None => default_layer(),
        };

        if input.is_none() {
            return Err(format!(
                "Missing Example chat code block in {}",
                path.display()
            ));
        }
        let input = input
            .ok_or_else(|| format!("Missing Example chat code block in {}", path.display()))?;

        let _expected_behavior = expected_behavior_parts.join("\n");
        let _chat_rule = normalize_whitespace(&chat_rule_parts.join(" "));

        let notes = if notes_parts.is_empty() {
            None
        } else {
            Some(normalize_whitespace(&notes_parts.join(" ")))
        };

        // Build spec
        let metadata = ErrorCorpusMetadata {
            category,
            description: description.clone(),
            level,
            layer,
        };

        let example = ErrorCorpusExample {
            name,
            description,
            input,
            error_code: Some(error_code),
            error_location: None,
            notes,
            expected_cst: None,
        };

        Ok(ErrorCorpusSpec {
            metadata,
            examples: vec![example],
        })
    }
}

impl ErrorCorpusExample {
    /// Generate a sanitized test name
    pub fn test_name(&self) -> String {
        self.name.replace(['-', ' '], "_").to_lowercase()
    }

    /// Get the expected CST if available, otherwise return placeholder
    pub fn expected_cst_or_placeholder(&self) -> String {
        match self.expected_cst.as_ref() {
            Some(cst) => cst.clone(),
            None => "(todo)".to_string(),
        }
    }
}

/// Extracts text from children.
fn extract_text_from_children<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        if let NodeValue::Text(ref text) = child.data.borrow().value {
            result.push_str(text);
        }
    }
    result
}

/// Runs normalize whitespace.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Runs strip single trailing newline.
fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}
