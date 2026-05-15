//! # Tree-Sitter Test Generator
//!
//! Generates tree-sitter native test files (`test/corpus/*.txt`)

use crate::spec::construct::{ConstructExample, ConstructSpec};
use crate::spec::error::ErrorSpec;
use crate::templates::TemplateLoader;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Parser, Tree};
use tree_sitter_talkbank::LANGUAGE;

/// Enum variants for TreeSitterTestError.
#[derive(Debug, Error)]
pub enum TreeSitterTestError {
    #[error("Failed to load TalkBank grammar for tree-sitter")]
    GrammarLoadFailed,
    #[error("Failed to load templates from {path}: {source}")]
    TemplateLoadFailed {
        path: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Failed to parse wrapped input for example '{0}'")]
    ParseFailed(String),
    #[error("Tree-sitter parse contains error nodes for example '{0}'")]
    ParseHasErrors(String),
    #[error("Template error for example '{example}': {source}")]
    TemplateWrapFailed {
        example: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Missing wrapped input and no templates provided for example '{0}'")]
    MissingWrappedInput(String),
    #[error("Generation issues:\n{0}")]
    Issues(String),
}

/// Builds parser for downstream use.
fn create_parser() -> Result<Parser, TreeSitterTestError> {
    let mut parser = Parser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|_| TreeSitterTestError::GrammarLoadFailed)?;
    Ok(parser)
}

/// Parses wrapped input.
fn parse_wrapped_input(
    parser: &mut Parser,
    example_name: &str,
    input: &str,
) -> Result<Tree, TreeSitterTestError> {
    parser
        .parse(input, None)
        .ok_or_else(|| TreeSitterTestError::ParseFailed(example_name.to_string()))
}

/// Format a single example as a tree-sitter test
/// If template_loader is provided and example lacks full_cst, auto-wraps using templates
pub fn format_test(example: &ConstructExample, _level: &str) -> String {
    let separator = "=".repeat(80);
    let divider = "-".repeat(80);

    // Use actual extracted data from specs
    let input = match example.expected.wrapped_input.as_ref() {
        Some(wrapped) => wrapped.as_str(),
        None => example.input.as_str(),
    };

    let cst = match example.expected.full_cst.as_ref() {
        Some(full) => full.clone(),
        None => example.expected_cst(),
    };

    // Use example name as test name (for matching with extract_cst_from_tests)
    let test_name = if example.description.is_empty() {
        example.name.clone()
    } else {
        format!("{} - {}", example.name, example.description)
    };

    format!(
        "{separator}\n{name}\n{separator}\n{input}\n\n{divider}\n\n{cst}\n",
        name = test_name,
        input = input,
        cst = cst,
    )
}

/// Format multiple examples into a single corpus file
pub fn format_corpus(examples: &[ConstructExample], level: &str) -> String {
    let mut output = String::new();

    for example in examples {
        output.push_str(&format_test(example, level));
        output.push('\n');
    }

    output
}

/// Generate tree-sitter corpus files from construct specs
/// If template_dir is provided, auto-wraps fragments using templates when full_cst is missing
pub fn generate_corpus_files(
    specs: &[ConstructSpec],
) -> Result<Vec<(String, String)>, TreeSitterTestError> {
    generate_corpus_files_with_templates(specs, None)
}

/// Generate tree-sitter corpus files with optional template support
/// Each example becomes its own test file, organized by level subdirectory
/// e.g., word/overlap_enclosed.txt, main_tier/simple_utterance.txt
pub fn generate_corpus_files_with_templates(
    specs: &[ConstructSpec],
    template_dir: Option<&Path>,
) -> Result<Vec<(String, String)>, TreeSitterTestError> {
    // Load templates if directory provided
    let template_loader = match template_dir {
        Some(dir) => {
            let loader = TemplateLoader::new(dir).map_err(|err| {
                TreeSitterTestError::TemplateLoadFailed {
                    path: dir.display().to_string(),
                    source: anyhow::anyhow!(err),
                }
            })?;
            Some(loader)
        }
        None => None,
    };

    let mut parser = create_parser()?;
    let mut files = Vec::new();
    let mut issues = Vec::new();

    for spec in specs {
        let level = &spec.metadata.level;

        for example in &spec.examples {
            // Try to get or generate full_cst
            let wrapped_input = if let Some(ref input) = example.expected.wrapped_input {
                input.clone()
            } else if let Some(ref loader) = template_loader {
                match loader.wrap_fragment(&example.input_type, &example.input) {
                    Ok(wrapped) => wrapped,
                    Err(err) => {
                        issues.push(TreeSitterTestError::TemplateWrapFailed {
                            example: example.name.clone(),
                            source: anyhow::anyhow!(err),
                        });
                        continue;
                    }
                }
            } else {
                issues.push(TreeSitterTestError::MissingWrappedInput(
                    example.name.clone(),
                ));
                continue;
            };

            let tree = match parse_wrapped_input(&mut parser, &example.name, &wrapped_input) {
                Ok(tree) => tree,
                Err(err) => {
                    issues.push(err);
                    continue;
                }
            };

            if tree.root_node().has_error() {
                issues.push(TreeSitterTestError::ParseHasErrors(example.name.clone()));
                continue;
            }

            let full_cst = match example.expected.full_cst.as_ref() {
                Some(full) => full.clone(),
                None => tree.root_node().to_sexp(),
            };

            // Create example with full_cst for test generation
            let mut example_with_cst = example.clone();
            example_with_cst.expected.wrapped_input = Some(wrapped_input.clone());
            example_with_cst.expected.full_cst = Some(full_cst);

            // Generate one file per example, organized by level subdirectory.
            // Use filesystem_name() to avoid spaces/unsafe characters without
            // introducing case-only path churn across platforms.
            let filename = format!("{}/{}.txt", level, example.filesystem_name());
            let content = format_test(&example_with_cst, level);
            files.push((filename, content));
        }
    }

    if issues.is_empty() {
        Ok(files)
    } else {
        let details = issues
            .into_iter()
            .map(|issue| format!("- {}", issue))
            .collect::<Vec<_>>()
            .join("\n");
        Err(TreeSitterTestError::Issues(details))
    }
}

/// Check if any node in the tree is MISSING (inserted by error recovery).
fn tree_has_missing(node: tree_sitter::Node<'_>) -> bool {
    if node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if tree_has_missing(cursor.node()) {
                return true;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

/// Generate tree-sitter error test corpus files from parser-layer error specs.
///
/// For each error spec with examples, parses the input with tree-sitter and captures
/// the CST (including ERROR nodes). Only generates tests where the grammar
/// actually produces ERROR nodes. Warns about parser-layer specs where the grammar
/// is too permissive (no ERROR nodes).
///
/// Returns `(filename, content)` pairs to be written under `errors/` subdirectory.
pub fn generate_error_corpus_files(
    specs: &[&ErrorSpec],
) -> Result<Vec<(String, String)>, TreeSitterTestError> {
    let mut parser = create_parser()?;
    let mut files = Vec::new();
    let mut no_error_warnings: Vec<String> = Vec::new();
    let mut missing_warnings: Vec<String> = Vec::new();

    for spec in specs {
        for error_def in &spec.errors {
            for (idx, example) in error_def.examples.iter().enumerate() {
                // Ensure input has @UTF8 prefix (tree-sitter document rule expects optional utf8_header)
                let mut input = if example.input.starts_with("@UTF8") {
                    example.input.clone()
                } else {
                    format!("@UTF8\n{}", example.input)
                };

                // Ensure input ends with newline — the grammar's `end_header` rule
                // requires a trailing newline token. Without it, tree-sitter inserts
                // a MISSING node, which the test format cannot represent.
                if !input.ends_with('\n') {
                    input.push('\n');
                }

                let tree = match parser.parse(&input, None) {
                    Some(tree) => tree,
                    None => continue,
                };

                if !tree.root_node().has_error() {
                    // Grammar is too permissive for this parser-layer error
                    no_error_warnings.push(format!(
                        "{} ({}): grammar accepts input without ERROR nodes",
                        error_def.code, error_def.name
                    ));
                    continue;
                }

                // Tree-sitter's test format cannot represent MISSING nodes at all.
                // If the parse tree has any MISSING nodes, the test will always fail
                // because the test runner treats them as "extra" nodes in comparison.
                // Skip these — they're still tested by the Rust parser tests.
                if tree_has_missing(tree.root_node()) {
                    missing_warnings.push(format!(
                        "{} ({}): tree has MISSING nodes (test format limitation)",
                        error_def.code, error_def.name
                    ));
                    continue;
                }

                let cst = tree.root_node().to_sexp();

                // Build test name
                let suffix = if error_def.examples.len() > 1 {
                    format!("_{}", idx + 1)
                } else {
                    String::new()
                };
                let test_name = format!("{}{} - {}", error_def.code, suffix, error_def.name);

                let separator = "=".repeat(80);
                let divider = "-".repeat(80);
                let content = format!(
                    "{separator}\n{name}\n{separator}\n{input}\n{divider}\n\n{cst}\n",
                    name = test_name,
                    input = input,
                    cst = cst,
                );

                let filename = format!("errors/{}{}.txt", error_def.code.to_lowercase(), suffix);
                files.push((filename, content));
            }
        }
    }

    // Print warnings about parser-layer specs without ERROR nodes
    if !no_error_warnings.is_empty() {
        eprintln!(
            "\n⚠ {} parser-layer error spec(s) did NOT produce tree-sitter ERROR nodes:",
            no_error_warnings.len()
        );
        for warning in &no_error_warnings {
            eprintln!("  - {}", warning);
        }
        eprintln!("  (Grammar may be too permissive for these errors — they are only caught at the Rust parser layer)\n");
    }

    // Print warnings about specs with MISSING nodes (test format limitation)
    if !missing_warnings.is_empty() {
        eprintln!(
            "⚠ {} error spec(s) skipped due to MISSING nodes in parse tree:",
            missing_warnings.len()
        );
        for warning in &missing_warnings {
            eprintln!("  - {}", warning);
        }
        eprintln!("  (Tree-sitter test format cannot represent MISSING nodes — these are tested by Rust parser tests)\n");
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::construct::*;

    /// Tests format test.
    #[test]
    fn test_format_test() {
        let example = ConstructExample {
            name: "simple_word".to_string(),
            input: "hello".to_string(),
            description: "Plain word".to_string(),
            expected: ExpectedParseTree {
                cst: "(word\n  (segment))".to_string(),
                wrapped_input: Some("@UTF8\n@Begin\n*CHI:\thello .\n@End".to_string()),
                full_cst: Some("(document\n  (utf8_header))".to_string()),
            },
            input_type: "standalone_word".to_string(),
        };

        let output = format_test(&example, "word");
        assert!(output.contains("Plain word"));
        assert!(output.contains("hello"));
        // Should use wrapped input and full CST from specs
        assert!(output.contains("@UTF8"));
        assert!(output.contains("*CHI:"));
        assert!(output.contains("document"));
    }
}
