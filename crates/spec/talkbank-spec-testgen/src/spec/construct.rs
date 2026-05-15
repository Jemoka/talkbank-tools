//! # Construct Specification Types
//!
//! Structured representation of the construct spec files in `resources/spec/constructs/`.
//!
//! Each Markdown file defines one valid CHAT example together with its expected
//! concrete syntax tree (CST). [`ConstructSpec`] groups examples by category
//! and provides the interface that generators (`gen_tree_sitter_tests`,
//! `gen_rust_tests`) consume to emit test files.

use serde::Deserialize;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

/// Root structure for a construct specification file
#[derive(Debug, Deserialize)]
pub struct ConstructSpec {
    pub metadata: ConstructMetadata,
    pub examples: Vec<ConstructExample>,
}

/// Metadata about the construct category
#[derive(Debug, Deserialize)]
pub struct ConstructMetadata {
    /// Construct level: "word", "main_tier", "dependent_tier", "utterance", "header", "file"
    pub level: String,
    /// Category within level: "basic", "shortenings", "ca_markers", etc.
    pub category: String,
    /// Human-readable description
    pub description: String,
}

/// A single example with input and expected parse tree
#[derive(Debug, Clone, Deserialize)]
pub struct ConstructExample {
    /// Unique name for this example (used in test names)
    pub name: String,
    /// The CHAT input to parse
    pub input: String,
    /// Human-readable description
    pub description: String,
    /// Expected parse tree
    pub expected: ExpectedParseTree,
    /// Input type (fence type from markdown) for template selection
    #[serde(default)]
    pub input_type: String,
}

/// Expected parse tree in CST format
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedParseTree {
    /// CST-level tree-sitter format (word node only)
    /// Example:
    /// ```text
    /// (word
    ///   (segment)
    ///   (lengthening)
    ///   (segment))
    /// ```
    pub cst: String,

    /// Wrapped input (for tree-sitter tests)
    /// This is the complete CHAT file with the construct embedded
    #[serde(default)]
    pub wrapped_input: Option<String>,

    /// Full document CST (for tree-sitter tests)
    /// This includes the entire CHAT file structure with headers and wrappers
    #[serde(default)]
    pub full_cst: Option<String>,
}

impl ConstructSpec {
    /// Load all construct specifications from a directory tree (Markdown only)
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();

        // Load Markdown format
        let categories = crate::spec::markdown::MarkdownCategory::load_all(root)?;
        for category in categories {
            specs.push(category.to_construct_spec());
        }

        if specs.is_empty() {
            return Err(format!(
                "No Markdown spec files found in {}",
                root.display()
            ));
        }

        Ok(specs)
    }
}

impl ConstructExample {
    /// Get the expected CST as a trimmed string
    pub fn expected_cst(&self) -> String {
        self.expected.cst.clone()
    }

    /// Generate a sanitized test name
    /// Uses NFKC normalization to convert uncommon codepoints (e.g., superscript 'ʰ' → 'h')
    pub fn test_name(&self) -> String {
        self.name
            .nfkc()
            .collect::<String>()
            .replace(['-', ' '], "_")
            .to_lowercase()
    }

    /// Generate a filesystem-safe lowercase name.
    ///
    /// This is intended for generated file paths where spaces and path-sensitive
    /// punctuation must be normalized and naming should be deterministic across
    /// case-insensitive and case-sensitive filesystems.
    pub fn filesystem_name(&self) -> String {
        let normalized = self.name.nfkc().collect::<String>();
        let mut out = String::new();
        let mut last_was_underscore = false;

        for c in normalized.chars() {
            let mapped = match c {
                ' ' | '-' => '_',
                '\'' | '"' | '%' | ':' | '/' | '\\' | '<' | '>' | '|' | '?' | '*' => '_',
                c if c.is_alphanumeric() || c == '_' => c,
                _ => '_',
            };

            if mapped == '_' {
                if !last_was_underscore {
                    out.push('_');
                    last_was_underscore = true;
                }
            } else {
                out.push(mapped);
                last_was_underscore = false;
            }
        }

        let trimmed = out.trim_matches('_').to_lowercase();
        if trimmed.is_empty() {
            "example".to_string()
        } else {
            trimmed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConstructExample, ExpectedParseTree};

    /// Build a minimal `ConstructExample` for unit tests.
    fn example(name: &str) -> ConstructExample {
        ConstructExample {
            name: name.to_string(),
            input: "x".to_string(),
            description: String::new(),
            expected: ExpectedParseTree {
                cst: "(word)".to_string(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "standalone_word".to_string(),
        }
    }

    /// Spaces, hyphens, and mixed case should all collapse into lowercase underscores.
    #[test]
    fn filesystem_name_lowercases_and_normalizes_separators() {
        let ex = example("Mary Had-A Little Lamb_12");
        assert_eq!(ex.filesystem_name(), "mary_had_a_little_lamb_12");
    }

    /// NFKC normalization maps compatibility codepoints (e.g. superscript h) to ASCII.
    #[test]
    fn filesystem_name_nfkc_normalizes_superscript_h() {
        let ex = example("gʰan-6");
        assert_eq!(ex.filesystem_name(), "ghan_6");
    }

    /// Characters unsafe in file paths (`/`, `:`, `*`, `?`) become underscores.
    #[test]
    fn filesystem_name_rejects_path_punctuation() {
        let ex = example("foo/bar:baz*qux?");
        assert_eq!(ex.filesystem_name(), "foo_bar_baz_qux");
    }
}
