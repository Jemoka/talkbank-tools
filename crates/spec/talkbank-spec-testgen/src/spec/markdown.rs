//! Markdown-based spec format parser
//!
//! Parses CHAT construct specifications from Markdown files with semantic code fences.
//! Uses comrak for AST-based parsing with roundtrip support via format_commonmark.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_commonmark, parse_document, Arena, Options};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A single parsed example from a Markdown spec file.
///
/// Each spec file in `resources/spec/constructs/` contains one example with an H1 name,
/// a description paragraph, a fenced input block, and a fenced CST block. This
/// struct holds the extracted pieces so generators can emit tree-sitter corpus
/// tests, Rust tests, or documentation without re-parsing the Markdown.
#[derive(Debug, Clone)]
pub struct MarkdownExample {
    /// H1 heading text, used as the test name (e.g. `"hello_0"`).
    pub name: String,
    /// First paragraph after the H1 -- a human-readable explanation of what the
    /// example demonstrates.
    pub description: String,
    /// Raw content of the first non-CST fenced code block (the CHAT fragment to
    /// parse).
    pub input: String,
    /// Fence info string that indicates the grammar level of the input (e.g.
    /// `"word"`, `"chat-file"`, `"mor-word"`). Drives template selection when
    /// wrapping fragments into full CHAT documents for tree-sitter tests.
    pub input_type: String,
    /// Expected concrete syntax tree in S-expression format, extracted from the
    /// ```` ```cst ```` fenced block.
    pub cst: String,
    /// Key-value pairs from the `## Metadata` section (e.g. `Source`, `Level`,
    /// `Category`). Used by generators for routing and categorization.
    pub metadata: HashMap<String, String>,
}

impl MarkdownExample {
    /// Parse a Markdown specification file
    pub fn parse(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        // Parse to AST
        let arena = Arena::new();
        let root = parse_document(&arena, &content, &Options::default());

        let mut name = String::new();
        let mut description = String::new();
        let mut input = String::new();
        let mut input_type = String::new();
        let mut cst = String::new();
        let mut metadata = HashMap::new();

        let mut found_h1 = false;
        let mut in_metadata_section = false;

        // Walk the AST
        for node in root.descendants() {
            let node_data = node.data.borrow();

            match &node_data.value {
                // H1 heading - extract name
                NodeValue::Heading(heading) if heading.level == 1 && !found_h1 => {
                    name = extract_text_from_children(node);
                    found_h1 = true;
                }

                // First paragraph after H1 - description
                NodeValue::Paragraph if found_h1 && description.is_empty() => {
                    description = extract_text_from_children(node);
                }

                // H2 heading - check for Metadata section
                NodeValue::Heading(heading) if heading.level == 2 => {
                    let heading_text = extract_text_from_children(node);
                    in_metadata_section = normalize_whitespace(&heading_text) == "Metadata";
                }

                // Code blocks - extract input and CST
                NodeValue::CodeBlock(code_block) => {
                    let lang = &code_block.info;
                    let content = &code_block.literal;

                    if lang == "cst" {
                        cst = strip_single_trailing_newline(content);
                    } else if !lang.is_empty() && input.is_empty() {
                        // Input fence - could be "word", "chat-file", "mor-word", etc.
                        input = strip_single_trailing_newline(content);
                        input_type = lang.clone();
                    }
                }

                // List in metadata section
                NodeValue::List(_) if in_metadata_section => {
                    for child in node.children() {
                        if let NodeValue::Item(_) = child.data.borrow().value {
                            extract_metadata_from_list_item(child, &mut metadata);
                        }
                    }
                }

                _ => {}
            }
        }

        Ok(MarkdownExample {
            name,
            description: normalize_whitespace(&description),
            input,
            input_type,
            cst,
            metadata,
        })
    }

    /// Get the wrapper strategy based on the fence marker
    /// Fence type directly specifies the CST node type (e.g., "standalone_word", "compound_word")
    /// Only "chat-file" and "document" don't need wrapping
    pub fn wrapper_strategy(&self) -> WrapperStrategy {
        match self.input_type.as_str() {
            "chat-file" | "document" => WrapperStrategy::None,
            "mor-word" => WrapperStrategy::ChatWithMor,
            "utterance" | "main_tier" => WrapperStrategy::ChatWithUtterance,
            "languages_header" | "participants_header" => WrapperStrategy::HeaderFragment,
            "com_dependent_tier" | "gra_dependent_tier" | "mor_dependent_tier"
            | "pho_dependent_tier" => WrapperStrategy::DependentTier,
            _ => WrapperStrategy::MinimalChat,
        }
    }

    /// Convert to ConstructExample format (for compatibility with generators)
    pub fn to_construct_example(&self) -> super::construct::ConstructExample {
        let strategy = self.wrapper_strategy();

        // For chat-file inputs, the input IS the full document
        // For fragments, let the test generator wrap using templates
        let (wrapped_input, full_cst) = match strategy {
            WrapperStrategy::None => {
                // chat-file: input and CST are already complete documents
                (Some(self.input.clone()), Some(self.cst.clone()))
            }
            _ => {
                // Fragment specs: DON'T set wrapped_input/full_cst here
                // Let the test generator wrap using templates and extract CST
                (None, None)
            }
        };

        super::construct::ConstructExample {
            name: self.name.clone(),
            input: self.input.clone(),
            description: self.description.clone(),
            expected: super::construct::ExpectedParseTree {
                cst: self.cst.clone(), // Fragment CST for Rust tests
                wrapped_input,         // Only set for chat-file inputs
                full_cst,              // Only set for chat-file inputs
            },
            input_type: self.input_type.clone(), // Fence type for template selection
        }
    }

    /// Update the CST section in a Markdown file and write back (roundtrip)
    pub fn update_cst(path: &Path, new_cst: &str) -> Result<(), String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        // Parse to AST
        let arena = Arena::new();
        let root = parse_document(&arena, &content, &Options::default());

        // Find and replace CST code block
        for node in root.descendants() {
            if let NodeValue::CodeBlock(code_block) = &mut node.data.borrow_mut().value {
                if code_block.info == "cst" {
                    code_block.literal = new_cst.to_string();
                    break;
                }
            }
        }

        // Serialize back to Markdown using format_commonmark
        let mut output = String::new();
        format_commonmark(root, &Options::default(), &mut output)
            .map_err(|e| format!("Failed to serialize markdown: {}", e))?;

        let updated = output;

        fs::write(path, updated)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

        Ok(())
    }
}

/// Extract plain text from all text nodes under this node
fn extract_text_from_children<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();

    for child in node.descendants() {
        if let NodeValue::Text(ref text) = child.data.borrow().value {
            result.push_str(text);
        }
    }

    result
}

/// Extract metadata key-value pairs from a list item
/// Expects format: **Key**: value
fn extract_metadata_from_list_item<'a>(
    list_item: &'a AstNode<'a>,
    metadata: &mut HashMap<String, String>,
) {
    let mut key = String::new();
    let mut value = String::new();
    let mut in_strong = false;
    let mut found_colon = false;

    for node in list_item.descendants() {
        match &node.data.borrow().value {
            NodeValue::Strong => {
                in_strong = true;
            }
            NodeValue::Text(text) => {
                if in_strong {
                    let mut strong_text = text.to_string();
                    if strong_text.ends_with(':') {
                        strong_text.pop();
                    }
                    key.push_str(&strong_text);
                } else if text.contains(':') && !found_colon {
                    // Found the colon separator
                    found_colon = true;
                    let parts: Vec<&str> = text.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        value.push_str(parts[1]);
                    }
                } else if found_colon {
                    // Value text after colon
                    value.push_str(text);
                }
            }
            NodeValue::Code(code) => {
                if found_colon {
                    // Inline code in value
                    value.push_str(&code.literal);
                }
            }
            _ => {
                // Exiting strong tag
                if in_strong {
                    in_strong = false;
                }
            }
        }
    }

    if !key.is_empty() {
        let key = normalize_whitespace(&key);
        if !key.is_empty() {
            let value = normalize_whitespace(&value);
            metadata.insert(key, value);
        }
    }
}

/// How to wrap a CHAT fragment so it becomes a complete, parseable CHAT file.
///
/// Spec examples are often sub-document fragments (a single word, a dependent
/// tier, etc.). Tree-sitter tests require a full document, so the generator
/// wraps each fragment using one of these strategies based on the fence type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrapperStrategy {
    /// Input is already a complete CHAT document (`chat-file` / `document`);
    /// no wrapping needed.
    None,
    /// Word-level fragment -- wrap in a minimal CHAT file with a single
    /// utterance containing the word.
    MinimalChat,
    /// Morphology-tier word -- wrap in a CHAT file with a `%mor` dependent
    /// tier so the fragment appears in the right context.
    ChatWithMor,
    /// Full utterance or main tier line -- wrap in a CHAT file with standard
    /// headers but let the fragment supply the utterance itself.
    ChatWithUtterance,
    /// Header fragment (`@Languages`, `@Participants`) -- wrap in a bare
    /// `@Begin`/`@End` skeleton.
    HeaderFragment,
    /// Dependent tier line (`%com`, `%gra`, `%mor`, `%pho`) -- wrap in a CHAT
    /// file with a preceding main tier so the tier attaches correctly.
    DependentTier,
    /// Auto-detect (falls back to `MinimalChat`).
    Infer,
}

impl WrapperStrategy {
    /// Wrap a CHAT fragment in the boilerplate needed to form a valid document.
    ///
    /// Returns the fragment unchanged for `None`, or embeds it inside the
    /// appropriate headers, participant declarations, and tier context for all
    /// other strategies.
    pub fn wrap_input(&self, input: &str) -> String {
        match self {
            Self::None => input.to_string(),
            Self::MinimalChat => format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t{} .\n@End",
                input
            ),
            Self::ChatWithMor => format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword .\n%mor:\t{}\n@End",
                input
            ),
            Self::ChatWithUtterance => format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n{}\n@End",
                input
            ),
            Self::HeaderFragment => format!("@UTF8\n@Begin\n{}\n@End", input),
            Self::DependentTier => format!(
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword .\n{}\n@End",
                input
            ),
            Self::Infer => Self::MinimalChat.wrap_input(input),
        }
    }
}

/// Collapse all runs of whitespace into single spaces and trim both ends.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove at most one trailing newline (`\n` or `\r\n`) from code block content.
///
/// Comrak appends a trailing newline to fenced code block literals; stripping
/// it avoids spurious whitespace differences in CST comparisons.
fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}

/// Check whether a filename starts with `_`, used to skip category-level
/// metadata files like `_category.md`.
fn is_underscore_prefixed(name: &str) -> bool {
    name.as_bytes().first() == Some(&b'_')
}

/// A directory of related spec examples grouped by construct category.
///
/// Each subdirectory under `resources/spec/constructs/` (e.g. `word/`, `tiers/`) is one
/// category. The category name comes from the directory name and determines
/// which tree-sitter corpus sub-directory the generated tests land in.
#[derive(Debug)]
pub struct MarkdownCategory {
    /// Directory name (e.g. `"basic"`, `"shortenings"`), used as the category
    /// label in generated test suites.
    pub name: String,
    /// Absolute path to the category directory on disk.
    pub path: PathBuf,
    /// All non-underscore-prefixed `.md` files parsed from this directory.
    pub examples: Vec<MarkdownExample>,
}

impl MarkdownCategory {
    /// Load all Markdown examples from a category directory
    pub fn load(category_dir: &Path) -> Result<Self, String> {
        let name = category_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid category directory: {}", category_dir.display()))?
            .to_string();

        let mut examples = Vec::new();

        let mut paths: Vec<_> = fs::read_dir(category_dir)
            .map_err(|e| format!("Failed to read directory {}: {}", category_dir.display(), e))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| !is_underscore_prefixed(name))
            })
            .collect();
        paths.sort();

        for path in &paths {
            let example = MarkdownExample::parse(path)?;
            examples.push(example);
        }

        Ok(MarkdownCategory {
            name,
            path: category_dir.to_path_buf(),
            examples,
        })
    }

    /// Get the level from the directory name (word, main_tier, tiers, etc.)
    pub fn level(&self) -> String {
        match self.path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => "unknown".to_string(),
        }
    }

    /// Load all categories from a root directory (recursively)
    pub fn load_all(root: &Path) -> Result<Vec<Self>, String> {
        use walkdir::WalkDir;

        let mut categories = Vec::new();

        // Find all directories containing .md files (but not starting with _)
        let mut issues = Vec::new();

        let mut dir_paths: Vec<_> = WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(e) if e.path().is_dir() => Some(e.into_path()),
                Ok(_) => None,
                Err(err) => {
                    issues.push(format!("WalkDir error: {}", err));
                    None
                }
            })
            .collect();
        dir_paths.sort();

        for dir_path in &dir_paths {
            let entries = match fs::read_dir(dir_path) {
                Ok(entries) => entries,
                Err(err) => {
                    issues.push(format!(
                        "Failed to read directory {}: {}",
                        dir_path.display(),
                        err
                    ));
                    continue;
                }
            };

            let has_examples = entries.filter_map(|e| e.ok()).any(|entry| {
                let path = entry.path();
                path.extension().is_some_and(|ext| ext == "md")
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|name| !is_underscore_prefixed(name))
            });

            if has_examples {
                match Self::load(dir_path) {
                    Ok(category) => {
                        if !category.examples.is_empty() {
                            categories.push(category);
                        }
                    }
                    Err(err) => issues.push(err),
                }
            }
        }

        if issues.is_empty() {
            Ok(categories)
        } else {
            Err(issues.join("\n"))
        }
    }

    /// Convert to ConstructSpec format (for compatibility with generators)
    pub fn to_construct_spec(&self) -> super::construct::ConstructSpec {
        let level = self.level();

        super::construct::ConstructSpec {
            metadata: super::construct::ConstructMetadata {
                level,
                category: self.name.clone(),
                description: format!("Category: {}", self.name),
            },
            examples: self
                .examples
                .iter()
                .map(|e| e.to_construct_example())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    /// Parse a realistic spec file and verify all fields are extracted correctly.
    #[test]
    fn test_markdown_parsing() -> Result<(), Box<dyn Error>> {
        let md = r#"# hello_0

Plain word without any special markers or annotations.

## Input

```word
hello
```

## Expected CST

```cst
(word_with_optional_annotations
  (standalone_word
    (word_body
      (initial_word_segment "hello"))))
```

## Metadata

- **Source**: resources/corpus/reference/basic.cha
- **Level**: word
- **Category**: basic
"#;

        // Write to temp file and parse
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_example.md");
        fs::write(&test_file, md)?;

        let example = MarkdownExample::parse(&test_file)?;

        assert_eq!(example.name, "hello_0");
        assert!(example.description.contains("Plain word"));
        assert_eq!(example.input, "hello");
        assert_eq!(example.input_type, "word");
        assert!(example.cst.contains("word_with_optional_annotations"));

        // Clean up
        let _ = fs::remove_file(test_file);
        Ok(())
    }

    /// Fence types map to the correct wrapping strategy.
    #[test]
    fn test_wrapper_strategy() {
        let word_example = MarkdownExample {
            name: "test".to_string(),
            description: "test".to_string(),
            input: "hello".to_string(),
            input_type: "word".to_string(),
            cst: String::new(),
            metadata: HashMap::new(),
        };

        assert_eq!(
            word_example.wrapper_strategy(),
            WrapperStrategy::MinimalChat
        );

        let chat_file_example = MarkdownExample {
            input_type: "chat-file".to_string(),
            ..word_example.clone()
        };

        assert_eq!(chat_file_example.wrapper_strategy(), WrapperStrategy::None);
    }

    /// Writing a new CST via `update_cst` and re-parsing yields the updated tree.
    #[test]
    fn test_cst_roundtrip() -> Result<(), Box<dyn Error>> {
        let md = r#"# test

Test example.

## Input

```word
hello
```

## Expected CST

```cst
(old_cst)
```
"#;

        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_roundtrip.md");
        fs::write(&test_file, md)?;

        // Update CST
        let new_cst = "(new_cst\n  (child))";
        MarkdownExample::update_cst(&test_file, new_cst)?;

        // Read back and verify
        let example = MarkdownExample::parse(&test_file)?;
        assert_eq!(example.cst, new_cst);

        // Clean up
        let _ = fs::remove_file(test_file);
        Ok(())
    }
}
