//! JSON-serialization smoke tests for utterance model types.
//!
//! The goal is to keep serde contracts stable for downstream tools that ingest
//! utterance and diagnostic payloads as structured JSON.

use super::*;
use crate::model::{MainTier, ReplacedWord, Replacement, Terminator, UtteranceContent, Word};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation, Span};

/// Utterance JSON serialization includes speaker and replacement payload fields.
///
/// This is a smoke test for serde coverage on a nested replaced-word structure.
#[test]
fn demo_utterance_json_serialization() -> Result<(), String> {
    let word = Word::new_unchecked("hello", "hello");
    let replacement = Replacement::from_word(Word::new_unchecked("world", "world"));
    let replaced_word = ReplacedWord::new(word, replacement);

    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::ReplacedWord(Box::new(replaced_word))],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let json = serde_json::to_string_pretty(&utterance)
        .map_err(|err| format!("Failed to serialize utterance: {err}"))?;

    assert!(json.contains("CHI"));
    assert!(json.contains("hello"));
    assert!(json.contains("world"));
    assert!(json.contains("replaced_word") || json.contains("replacement"));
    Ok(())
}

/// Parse errors serialize with code, severity, and suggestion metadata.
///
/// Keeping this stable is important for downstream tooling that consumes diagnostics as JSON.
#[test]
fn demo_error_json_serialization() -> Result<(), String> {
    let error = ParseError::new(
        ErrorCode::SpeakerNotFoundInParticipants,
        Severity::Warning,
        SourceLocation::from_offsets(1, 3),
        ErrorContext::new("*MO: how are you ?", 1..3, "MO")
            .with_expected(vec!["CHI".to_string(), "MOT".to_string()]),
        "Speaker 'MO' not found in @Participants header",
    )
    .with_suggestion("Did you mean 'MOT' (Mother)?");

    let json = serde_json::to_string_pretty(&error)
        .map_err(|err| format!("Failed to serialize error: {err}"))?;

    assert!(json.contains("W108"));
    assert!(json.contains("warning"));
    assert!(json.contains("suggestion"));
    Ok(())
}
