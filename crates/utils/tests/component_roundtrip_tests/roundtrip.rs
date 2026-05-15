//! Test module for roundtrip in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

// Roundtrip test utilities - may be used in future component tests
#![allow(dead_code)]

use serde_json::Value;
use std::path::PathBuf;
use talkbank_model::ErrorCollector;
use talkbank_model::WriteChat;
use talkbank_model::model::{Utterance, Word};
use talkbank_model::{ParseError, ParseErrors, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use thiserror::Error;

/// Helper function to perform true roundtrip test (no validation).
pub fn true_roundtrip_tier<T, F>(chat_input: &str, parse_fn: F) -> Result<(), RoundtripError>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + WriteChat + PartialEq,
    F: Fn(&str) -> Result<T, ParseErrors>,
{
    true_roundtrip_tier_with_validation(chat_input, parse_fn, |_| Vec::new())
}

/// Parse a word through the tree-sitter parser.
pub fn parse_word_direct(input: &str) -> Result<Word, ParseErrors> {
    let parser = TreeSitterParser::new().expect("TreeSitterParser should construct");
    let errors = ErrorCollector::new();
    match parser.parse_word_fragment(input, 0, &errors) {
        ParseOutcome::Parsed(word) => Ok(word),
        ParseOutcome::Rejected => Err(ParseErrors::from(errors.into_vec())),
    }
}

/// Parse an utterance fragment through the tree-sitter parser.
pub fn parse_utterance_direct(input: &str) -> Result<Utterance, ParseErrors> {
    let parser = TreeSitterParser::new().expect("TreeSitterParser should construct");
    let errors = ErrorCollector::new();
    match parser.parse_utterance_fragment(input, 0, &errors) {
        ParseOutcome::Parsed(utterance) => Ok(utterance),
        ParseOutcome::Rejected => Err(ParseErrors::from(errors.into_vec())),
    }
}

/// Helper function to perform true roundtrip test WITH validation.
pub fn true_roundtrip_tier_with_validation<T, F, V>(
    chat_input: &str,
    parse_fn: F,
    validate_fn: V,
) -> Result<(), RoundtripError>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + WriteChat + PartialEq,
    F: Fn(&str) -> Result<T, ParseErrors>,
    V: Fn(&T) -> Vec<ParseError>,
{
    let tier = parse_fn(chat_input).map_err(|source| RoundtripError::Parse { source })?;

    let validation_errors = validate_fn(&tier);
    if !validation_errors.is_empty() {
        let messages = validation_errors
            .iter()
            .map(|e| e.message.clone())
            .collect();
        return Err(RoundtripError::Validation {
            count: validation_errors.len(),
            messages,
        });
    }

    let json = serde_json::to_string_pretty(&tier)
        .map_err(|source| RoundtripError::JsonSerialize { source })?;

    if let Some(thread_name) = std::thread::current().name() {
        let output_dir = std::path::Path::new("target/roundtrip-json");
        if let Err(source) = std::fs::create_dir_all(output_dir) {
            eprintln!(
                "Warning: Failed to create output dir {:?}: {}",
                output_dir, source
            );
        }
        let output_path = output_dir.join(format!("{}.json", thread_name));
        if let Err(source) = std::fs::write(&output_path, &json) {
            eprintln!(
                "Warning: Failed to save JSON to {:?}: {}",
                output_path, source
            );
        }
    }

    let _json_value: Value = parse_json(&json)?;

    let tier_from_json: T =
        serde_json::from_str(&json).map_err(|source| RoundtripError::JsonDeserialize { source })?;

    let chat_output = tier_from_json.to_chat_string();

    // Strip trailing newline: write_chat appends '\n' for components like
    // Utterance, but the original input typically omits it.  Keeping the
    // newline would shift all byte-level spans in the re-parsed model,
    // breaking PartialEq even when the logical content is identical.
    let chat_output_trimmed = chat_output.trim_end_matches('\n');

    let roundtrip_tier = parse_fn(chat_output_trimmed)
        .map_err(|source| RoundtripError::RoundtripParse { source })?;

    if roundtrip_tier != tier {
        return Err(RoundtripError::RoundtripMismatch {
            input: chat_input.to_string(),
            output: chat_output_trimmed.to_string(),
        });
    }

    Ok(())
}

/// Enum variants for RoundtripError.
#[derive(Debug, Error)]
pub enum RoundtripError {
    #[error("Parse failed")]
    Parse { source: ParseErrors },
    #[error("Validation failed with {count} error(s)")]
    Validation { count: usize, messages: Vec<String> },
    #[error("JSON serialization failed")]
    JsonSerialize { source: serde_json::Error },
    #[error("JSON deserialization failed")]
    JsonDeserialize { source: serde_json::Error },
    #[error("JSON parse failed")]
    JsonParse { source: serde_json::Error },
    #[error("Roundtrip parse failed")]
    RoundtripParse { source: ParseErrors },
    #[error("Roundtrip semantic mismatch")]
    RoundtripMismatch { input: String, output: String },
    #[error("Failed to create output directory: {path}")]
    OutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to write output: {path}")]
    OutputWrite {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Parses json.
fn parse_json(json: &str) -> Result<Value, RoundtripError> {
    serde_json::from_str(json).map_err(|source| RoundtripError::JsonParse { source })
}
