//! Validate that error words trigger expected parser errors.
//!
//! Tests the error_words_corpus.txt file which contains words that should
//! fail parsing or produce validation errors. These examples are extracted
//! from resources/spec/errors/*.md files.
//!
//! ## Usage
//!
//! ```bash
//! # Run error validation tests
//! cargo test error_words_validation
//!
//! # Show details
//! cargo test error_words_validation -- --nocapture
//! ```

use regex::Regex;
use std::sync::LazyLock;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

static HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*#\s*(E\d{3})\s*:").expect("valid regex"));

static WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s*$").expect("valid regex"));

static EMPTY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));

static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));

const ERROR_WORDS_FILE: &str = include_str!("../error_words_corpus.txt");

/// Parses error corpus.
fn parse_error_corpus() -> Result<Vec<(String, String)>, TestError> {
    // Parse format: # E202: description\nword\n
    let mut result = Vec::new();
    let parser = ErrorCorpusParser;
    let mut pending_code: Option<String> = None;

    for line in ERROR_WORDS_FILE.lines() {
        if let Some(code) = parser.parse_header_code(line)? {
            pending_code = Some(code);
            continue;
        }

        if parser.is_ignorable(line) {
            if let Some(code) = pending_code.as_ref() {
                return Err(TestError::Failure(format!(
                    "Missing word after error code {}",
                    code
                )));
            }
            continue;
        }

        if let Some(code) = pending_code.take() {
            let word = parser.parse_word_line(line)?;
            result.push((word, code));
            continue;
        }
    }

    if let Some(code) = pending_code {
        return Err(TestError::Failure(format!(
            "Missing word after error code {}",
            code
        )));
    }
    Ok(result)
}

/// Verifies known error-word corpus entries do not parse cleanly.
#[test]
fn error_words_should_fail_parsing() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let error_words = parse_error_corpus()?;

    println!(
        "
Testing {} error words...",
        error_words.len()
    );

    let mut unexpected_success = Vec::new();

    for (word, expected_code) in &error_words {
        let errors = ErrorCollector::new();
        let result = parser.parse_word_fragment(word, 0, &errors);

        // Should either fail to parse OR produce errors
        if result.is_some() && errors.into_vec().is_empty() {
            unexpected_success.push(format!("{} (expected {})", word, expected_code));
        }
    }

    if !unexpected_success.is_empty() {
        return Err(TestError::Failure(format!(
            "{} error words parsed successfully without errors:\n  {}",
            unexpected_success.len(),
            unexpected_success.join("\n  ")
        )));
    }

    Ok(())
}

/// Parser for `error_words.txt` sections grouped by expected error code.
struct ErrorCorpusParser;

impl ErrorCorpusParser {
    /// Parses header code.
    fn parse_header_code(&self, line: &str) -> Result<Option<String>, TestError> {
        let caps = match HEADER_RE.captures(line) {
            Some(caps) => caps,
            None => return Ok(None),
        };
        let code = caps
            .get(1)
            .ok_or_else(|| TestError::Failure(format!("Malformed error header line: {line}")))?
            .as_str()
            .to_string();
        Ok(Some(code))
    }

    /// Parses word line.
    fn parse_word_line(&self, line: &str) -> Result<String, TestError> {
        let caps = WORD_RE
            .captures(line)
            .ok_or_else(|| TestError::Failure(format!("Invalid word entry: {line}")))?;
        let word = caps
            .get(1)
            .ok_or_else(|| TestError::Failure(format!("Invalid word entry: {line}")))?
            .as_str()
            .to_string();
        if COMMENT_RE.is_match(&word) {
            return Err(TestError::Failure(format!("Invalid word entry: {line}")));
        }
        Ok(word)
    }

    /// Returns whether ignorable.
    fn is_ignorable(&self, line: &str) -> bool {
        EMPTY_RE.is_match(line) || COMMENT_RE.is_match(line)
    }
}
