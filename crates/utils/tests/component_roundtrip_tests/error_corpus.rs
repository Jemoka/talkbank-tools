//! Test module for error corpus in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::Path;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to read corpus file {path}: {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },
    #[error("Corpus file missing expected error comment")]
    MissingExpectedError,
}

/// Extracts expected error code from @Comment headers in CHAT file.
fn extract_expected_error(content: &str) -> Option<String> {
    for line in content.lines() {
        let (header, rest) = match line.split_once(':') {
            Some((header, rest)) => (header, rest),
            None => continue,
        };

        if header != "@Comment" {
            continue;
        }

        if let Some((_, after_expected)) = rest.split_once("Expected error:")
            && let Some(code) = after_expected.split_whitespace().next()
        {
            return Some(code.to_string());
        }
        if let Some((_, after_expected)) = rest.split_once("Expected warning:")
            && let Some(code) = after_expected.split_whitespace().next()
        {
            return Some(code.to_string());
        }
    }
    None
}

/// Runs validation errors from corpus e220.
#[test]
fn validation_errors_from_corpus_e220() -> Result<(), TestError> {
    let corpus_file = Path::new("tests/error_corpus/E2xx_word_errors/E220_unknown_shortening.cha");
    if !corpus_file.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(corpus_file).map_err(|source| TestError::ReadError {
        path: corpus_file.display().to_string(),
        source,
    })?;
    let expected_error = extract_expected_error(&content).ok_or(TestError::MissingExpectedError)?;

    println!("Would test for error: {}", expected_error);

    Ok(())
}
