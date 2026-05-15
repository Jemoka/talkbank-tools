//! Snapshot testing infrastructure for parser golden tests.
//!
//! Provides utilities for parsing CHAT inputs and generating insta snapshots
//! for regression testing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::test_error::TestError;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;

/// Parse a word and return a snapshot value.
pub fn parse_word_snapshot(
    parser: &TreeSitterParser,
    input: &str,
) -> Result<serde_json::Value, TestError> {
    let errors = ErrorCollector::new();
    match parser.parse_word_fragment(input, 0, &errors) {
        ParseOutcome::Parsed(word) => Ok(serde_json::to_value(&word)?),
        ParseOutcome::Rejected => Err(TestError::Failure(format!(
            "Parser failed to parse word '{}'",
            input
        ))),
    }
}

/// Parse a complete file and return a snapshot value.
pub fn parse_file_snapshot(
    parser: &TreeSitterParser,
    content: &str,
) -> Result<serde_json::Value, TestError> {
    let errors = ErrorCollector::new();
    match parser.parse_chat_file_fragment(content, 0, &errors) {
        ParseOutcome::Parsed(file) => Ok(serde_json::to_value(&file)?),
        ParseOutcome::Rejected => Err(TestError::Failure(
            "Parser failed to parse file".to_string(),
        )),
    }
}

/// Test helper: parse a word and snapshot the result.
///
/// # Example
/// ```ignore
/// #[test]
/// fn spec_word_basic_hello() {
///     spec_word_test("hello");
/// }
/// ```
pub fn spec_word_test(input: &str) -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let snapshot = parse_word_snapshot(&parser, input)?;
    insta::assert_json_snapshot!(snapshot);
    Ok(())
}

/// Test helper: parse a reference corpus file and snapshot the result.
///
/// # Example
/// ```ignore
/// #[test]
/// fn reference_corpus_abc_001() {
///     reference_file_test("resources/corpus/reference/ABC_001.cha");
/// }
/// ```
pub fn reference_file_test(path: &str) -> Result<(), TestError> {
    // Resolve path relative to repo root if it doesn't exist as-is
    let full_path = if std::path::Path::new(path).exists() {
        path.to_string()
    } else {
        // CARGO_MANIFEST_DIR is crates/talkbank-parser-tests
        // corpus is at repo root, so go up two levels (../.. from crate dir)
        let crate_dir = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(dir) => dir,
            Err(_err) => ".".to_string(),
        };
        let repo_root = std::path::Path::new(&crate_dir).join("../..").join(path);
        repo_root.to_string_lossy().to_string()
    };

    let content = std::fs::read_to_string(&full_path)?;

    // Use filename as snapshot suffix to make each test case unique
    let filename = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        // DEFAULT: If the file stem is unavailable, use a stable snapshot name.
        .unwrap_or("unknown");

    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let snapshot = parse_file_snapshot(&parser, &content)?;

    insta::assert_json_snapshot!(filename, snapshot);
    Ok(())
}

/// Test helper: parse with tree-sitter and snapshot a word.
pub fn legacy_tree_sitter_word_snapshot(input: &str) -> Result<serde_json::Value, TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    parse_word_snapshot(&parser, input)
}

/// Test helper: parse file with tree-sitter and snapshot the result.
pub fn legacy_tree_sitter_file_snapshot(path: &str) -> Result<serde_json::Value, TestError> {
    let content = std::fs::read_to_string(path)?;
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    parse_file_snapshot(&parser, &content)
}
