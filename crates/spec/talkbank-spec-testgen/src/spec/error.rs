//! # Error Specification Types
//!
//! Structured representation of the error spec files in `resources/spec/errors/`.
//!
//! Each Markdown file defines one error code with its metadata (severity,
//! category, layer), a human-readable description, and one or more bad-input
//! examples that should trigger the error. Generators consume these types to
//! emit Rust validation tests and error documentation pages.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

/// Root structure for an error specification file.
///
/// Typically loaded from a single `resources/spec/errors/E###_*.md` Markdown file.
/// Contains category-level metadata plus one or more error definitions (in
/// practice, one per file).
#[derive(Debug, Deserialize)]
pub struct ErrorSpec {
    /// Category-level metadata (code range, layer, status).
    pub metadata: ErrorMetadata,
    /// Error definitions contained in this spec (usually exactly one).
    pub errors: Vec<ErrorDefinition>,
    /// Basename of the source Markdown file (e.g. `"E304_MissingUtteranceTerminator.md"`),
    /// populated after loading -- not present in the Markdown itself.
    #[serde(skip)]
    pub source_file: String,
}

/// Metadata about the error category
#[derive(Debug, Deserialize)]
pub struct ErrorMetadata {
    /// Error code range: "E200-E299", "E500-E599", etc.
    pub range: String,
    /// Error category: "word", "header", "alignment", etc.
    pub category: String,
    /// Error type: "parser" or "validation"
    #[serde(rename = "type")]
    pub error_type: String,
    /// Human-readable description
    pub description: String,
    /// Implementation status: "implemented" (default) or "not_implemented"
    #[serde(default = "default_status")]
    pub status: String,
}

/// Serde default for `ErrorMetadata::status` -- specs without an explicit
/// `Status` field are assumed to be implemented.
fn default_status() -> String {
    "implemented".to_string()
}

/// A single error definition
#[derive(Debug, Deserialize)]
pub struct ErrorDefinition {
    /// Error code: "E241", "E520", etc.
    pub code: String,
    /// Short name: "IllegalUntranscribed", "SpeakerNotInParticipants", etc.
    pub name: String,
    /// Severity: "error" or "warning"
    pub severity: String,
    /// Human-readable description
    pub description: String,
    /// How to fix the error
    pub suggestion: String,
    /// URL to documentation
    #[serde(default)]
    pub help_url: Option<String>,
    /// References this error needs for message generation
    pub references: ErrorReference,
    /// Bad examples that trigger this error
    pub examples: Vec<ErrorExample>,
}

/// Declares which source spans and contextual data an error message needs.
///
/// Generators use these flags to emit the correct `ErrorReference` construction
/// in Rust code. The `additional` map provides forward-compatible extensibility
/// for new reference kinds without changing the struct.
#[derive(Debug, Deserialize, Default)]
pub struct ErrorReference {
    // -- Common references --
    /// The span of the offending word.
    #[serde(default)]
    pub word_span: bool,
    /// The textual content of the offending word.
    #[serde(default)]
    pub word_text: bool,
    /// The span of the containing dependent tier line.
    #[serde(default)]
    pub tier_span: bool,
    /// The span of the containing utterance (main tier + dependents).
    #[serde(default)]
    pub utterance_span: bool,

    // -- Type-specific references --
    /// The specific illegal character that caused the error.
    #[serde(default)]
    pub illegal_char: bool,
    /// Byte offset of the illegal character within the word.
    #[serde(default)]
    pub char_position: bool,
    /// Three-letter speaker code (e.g. `CHI`, `MOT`).
    #[serde(default)]
    pub speaker_code: bool,
    /// The span of the `@Participants` header (for cross-reference labels).
    #[serde(default)]
    pub participants_span: bool,

    /// Catch-all for reference kinds not yet promoted to named fields.
    #[serde(flatten)]
    pub additional: HashMap<String, bool>,
}

/// A bad example that triggers an error
#[derive(Debug, Deserialize)]
pub struct ErrorExample {
    /// The input that triggers the error
    pub input: String,
    /// Context level: "word", "tier", "utterance", "file"
    pub context: String,
    /// Expected error codes
    #[serde(default)]
    pub expected_codes: Vec<String>,
    /// Expected error message (or substring)
    pub expected_message: String,
    /// Optional labels for multi-span errors
    #[serde(default)]
    pub expected_labels: Vec<ErrorLabel>,
}

/// A label for multi-span errors
#[derive(Debug, Deserialize)]
pub struct ErrorLabel {
    /// Which span: "utterance", "participants", etc.
    pub span: String,
    /// Label text: "speaker used here", "@Participants declared here", etc.
    pub text: String,
}

impl ErrorSpec {
    /// Load an error specification from a Markdown file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        // Parse to AST
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, &content, &comrak::Options::default());

        let mut name = String::new();
        let mut description = String::new();
        let mut examples = Vec::new();
        let mut metadata = std::collections::HashMap::new();

        let mut found_h1 = false;
        let mut current_h2 = String::new();

        // Walk the AST
        for node in root.descendants() {
            let node_data = node.data.borrow();

            match &node_data.value {
                // H1 heading - extract code and name
                comrak::nodes::NodeValue::Heading(heading) if heading.level == 1 && !found_h1 => {
                    name = extract_text_from_children(node);
                    found_h1 = true;
                }

                // H2 heading
                comrak::nodes::NodeValue::Heading(heading) if heading.level == 2 => {
                    current_h2 = normalize_whitespace(&extract_text_from_children(node));
                }

                // Description paragraph
                comrak::nodes::NodeValue::Paragraph
                    if current_h2 == "Description" && description.is_empty() =>
                {
                    description = normalize_whitespace(&extract_text_from_children(node));
                }

                // Metadata list
                comrak::nodes::NodeValue::List(_) if current_h2 == "Metadata" => {
                    for child in node.children() {
                        if let comrak::nodes::NodeValue::Item(_) = child.data.borrow().value {
                            extract_metadata_from_list_item(child, &mut metadata);
                        }
                    }
                }

                // Example code block
                comrak::nodes::NodeValue::CodeBlock(code_block)
                    if current_h2.starts_with("Example") =>
                {
                    let input = strip_single_trailing_newline(&code_block.literal);
                    let mut context = code_block.info.clone();
                    if context == "chat" {
                        context = "chat_file".to_string();
                    }
                    if context.is_empty() {
                        context = "utterance".to_string();
                    }

                    // Try to find "Expected Error Codes" in preceding siblings
                    let mut expected_codes = Vec::new();
                    let mut prev = node.previous_sibling();
                    while let Some(sibling) = prev {
                        let text = extract_text_from_children(sibling);
                        if let Some(pos) = text.find("Expected Error Codes:") {
                            let rest = &text[pos + "Expected Error Codes:".len()..];
                            let codes_str = rest.lines().next().unwrap_or("");
                            expected_codes = codes_str
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            break;
                        }
                        // Stop if we hit another H2
                        if let comrak::nodes::NodeValue::Heading(h) = sibling.data.borrow().value {
                            if h.level == 2 {
                                break;
                            }
                        }
                        prev = sibling.previous_sibling();
                    }

                    examples.push(ErrorExample {
                        input,
                        context,
                        expected_codes,
                        expected_message: String::new(),
                        expected_labels: Vec::new(),
                    });
                }

                _ => {}
            }
        }

        let code = metadata.get("Error Code").cloned().unwrap_or_else(|| {
            // Try to extract from name if not in metadata (e.g., "# E304 - ...")
            name.split_whitespace()
                .next()
                .map(|s| s.trim_end_matches(':').to_string())
                .unwrap_or_default()
        });

        let error_def = ErrorDefinition {
            code: code.clone(),
            name: name
                .split('-')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| name.clone()),
            severity: metadata
                .get("Severity")
                .cloned()
                .unwrap_or_else(|| "error".to_string()),
            description: description.clone(),
            suggestion: String::new(), // TODO
            help_url: None,
            references: ErrorReference::default(),
            examples,
        };

        Ok(ErrorSpec {
            metadata: ErrorMetadata {
                range: format!("{}x", &code[..2]),
                category: metadata.get("Category").cloned().unwrap_or_default(),
                error_type: metadata
                    .get("Layer")
                    .cloned()
                    .unwrap_or_else(|| "parser".to_string()),
                description: description.clone(),
                status: metadata
                    .get("Status")
                    .cloned()
                    .unwrap_or_else(|| "implemented".to_string()),
            },
            errors: vec![error_def],
            source_file: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Load all error specifications from a directory
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();
        let mut issues = Vec::new();

        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut paths: Vec<_> = walkdir::WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(e) => Some(e),
                Err(err) => {
                    issues.push(format!("WalkDir error: {}", err));
                    None
                }
            })
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
            .filter(|entry| {
                let file_name = entry.file_name().to_str().unwrap_or_default();
                !file_name.starts_with('_')
                    && file_name != "README.md"
                    && file_name != "SPEC_ENHANCEMENT_GUIDE.md"
            })
            .map(|entry| entry.into_path())
            .collect();
        paths.sort();

        for path in &paths {
            match Self::load(path) {
                Ok(spec) => specs.push(spec),
                Err(err) => issues.push(format!("Failed to load {}: {}", path.display(), err)),
            }
        }

        if !issues.is_empty() {
            // println!("Warnings while loading error specs:\n{}", issues.join("\n"));
        }

        Ok(specs)
    }
}

/// Extract plain text from all text nodes under this node
fn extract_text_from_children<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    let mut result = String::new();

    for child in node.descendants() {
        if let comrak::nodes::NodeValue::Text(ref text) = child.data.borrow().value {
            result.push_str(text);
        }
    }

    result
}

/// Extract metadata key-value pairs from a list item
fn extract_metadata_from_list_item<'a>(
    list_item: &'a comrak::nodes::AstNode<'a>,
    metadata: &mut std::collections::HashMap<String, String>,
) {
    let text = extract_text_from_children(list_item);
    if let Some((key, value)) = text.split_once(':') {
        metadata.insert(normalize_whitespace(key), normalize_whitespace(value));
    }
}

/// Collapse all runs of whitespace into single spaces and trim both ends.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove at most one trailing newline (`\n` or `\r\n`) from code block content.
fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}

impl ErrorDefinition {
    /// Get the Rust variant name for this error code
    /// E241 -> E241
    pub fn code_variant(&self) -> String {
        self.code.clone()
    }

    /// Generate a sanitized test name from the error code
    pub fn test_name_prefix(&self) -> String {
        self.code.to_lowercase().replace(['e', 'w'], "")
    }
}

impl ErrorExample {
    /// Generate a sanitized name for this example.
    ///
    /// Uses NFKC normalization to convert uncommon codepoints, then
    /// collapses consecutive underscores to avoid non_snake_case warnings.
    pub fn sanitized_name(&self) -> String {
        // Use first few words of input or expected message
        let name = self
            .input
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join("_");
        let filtered: String = name
            .nfkc()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();
        // Collapse consecutive underscores and trim leading/trailing underscores
        let mut result = String::with_capacity(filtered.len());
        let mut prev_underscore = false;
        for c in filtered.chars() {
            if c == '_' {
                if !prev_underscore && !result.is_empty() {
                    result.push('_');
                }
                prev_underscore = true;
            } else {
                result.push(c);
                prev_underscore = false;
            }
        }
        // Trim trailing underscore
        if result.ends_with('_') {
            result.pop();
        }
        result
    }

    /// Extract expected error message substring for assertion
    pub fn expected_substring(&self) -> &str {
        &self.expected_message
    }
}
