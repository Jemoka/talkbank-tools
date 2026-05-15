//! Integration tests for the parse/validate/convert pipeline.
//!
//! Tests cover: parse_and_validate, streaming parse, file I/O, and conversion.

use std::io::Write as _;
use talkbank_model::{ErrorCollector, ParseValidateOptions};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::{
    PipelineError, chat_to_json, chat_to_json_unvalidated, normalize_chat, parse_and_validate,
    parse_and_validate_streaming, parse_and_validate_streaming_with_parser,
    parse_file_and_validate,
};

/// Minimal valid CHAT file with one utterance and a %mor tier.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";

/// Minimal valid CHAT file without utterances.
const MINIMAL_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n@End\n";

// ===== Parse & validate (5 tests) =====

#[test]
fn parse_valid_minimal_chat() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default().with_validation();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    // Should parse one utterance with no errors
    assert_eq!(chat_file.utterances().count(), 1);
    Ok(())
}

#[test]
fn parse_with_alignment_on_vs_off() -> Result<(), PipelineError> {
    // Without alignment: should succeed
    let options_no_align = ParseValidateOptions::default().with_validation();
    let result_no_align = parse_and_validate(VALID_CHAT, options_no_align);
    assert!(
        result_no_align.is_ok(),
        "Validation without alignment should pass"
    );

    // With alignment: may or may not produce alignment errors, but should not panic
    let options_align = ParseValidateOptions::default().with_alignment();
    let _result_align = parse_and_validate(VALID_CHAT, options_align);
    // We just verify it does not panic; alignment checking is a superset of validation.
    Ok(())
}

#[test]
fn parse_invalid_missing_end() {
    // Missing @End triggers parse error
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    let result = parse_and_validate(content, options);
    assert!(result.is_err(), "Missing @End should produce an error");
}

#[test]
fn parse_invalid_missing_begin() {
    // Missing @Begin triggers parse/validation error
    let content = "@UTF8\n@Languages:\teng\n@Participants:\tCHI Child\n@End\n";
    let options = ParseValidateOptions::default().with_validation();
    let result = parse_and_validate(content, options);
    assert!(result.is_err(), "Missing @Begin should produce an error");
}

#[test]
fn parse_empty_content() {
    let options = ParseValidateOptions::default();
    let result = parse_and_validate("", options);
    assert!(result.is_err(), "Empty content should produce an error");
}

// ===== Streaming parse (3 tests) =====

#[test]
fn streaming_parse_collects_errors() -> Result<(), PipelineError> {
    // Invalid content: missing @End
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    let errors = ErrorCollector::new();
    let _chat_file = parse_and_validate_streaming(content, options, &errors)?;
    // Errors should be collected in the sink
    let error_vec = errors.into_vec();
    assert!(
        !error_vec.is_empty(),
        "Streaming parse of invalid content should collect errors"
    );
    Ok(())
}

#[test]
fn streaming_parse_valid_no_errors() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default().with_validation();
    let errors = ErrorCollector::new();
    let chat_file = parse_and_validate_streaming(VALID_CHAT, options, &errors)?;
    let error_vec = errors.into_vec();
    // Filter to actual errors (not warnings)
    let actual_errors: Vec<_> = error_vec
        .iter()
        .filter(|e| e.severity == talkbank_model::Severity::Error)
        .collect();
    assert!(
        actual_errors.is_empty(),
        "Valid CHAT should produce no errors, got: {:?}",
        actual_errors
    );
    assert_eq!(chat_file.utterances().count(), 1);
    Ok(())
}

#[test]
fn streaming_parse_with_parser_reuse() -> Result<(), PipelineError> {
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    let options = ParseValidateOptions::default();

    // Parse two different inputs with the same parser instance
    let errors1 = ErrorCollector::new();
    let file1 =
        parse_and_validate_streaming_with_parser(&parser, VALID_CHAT, options.clone(), &errors1)?;
    assert_eq!(file1.utterances().count(), 1);

    let errors2 = ErrorCollector::new();
    let file2 = parse_and_validate_streaming_with_parser(&parser, MINIMAL_CHAT, options, &errors2)?;
    assert_eq!(file2.utterances().count(), 0);

    Ok(())
}

// ===== File I/O (3 tests) =====

#[test]
fn parse_file_valid() -> Result<(), PipelineError> {
    let dir = tempfile::tempdir().map_err(|e| PipelineError::Io(e))?;
    let file_path = dir.path().join("test.cha");
    {
        let mut f = std::fs::File::create(&file_path).map_err(PipelineError::Io)?;
        f.write_all(VALID_CHAT.as_bytes())
            .map_err(PipelineError::Io)?;
    }
    let options = ParseValidateOptions::default().with_validation();
    let chat_file = parse_file_and_validate(&file_path, options)?;
    assert_eq!(chat_file.utterances().count(), 1);
    Ok(())
}

#[test]
fn parse_file_not_found() {
    let path = std::path::Path::new("/tmp/talkbank_nonexistent_test_file_12345.cha");
    let options = ParseValidateOptions::default();
    let result = parse_file_and_validate(path, options);
    assert!(
        matches!(result, Err(PipelineError::Io(_))),
        "Nonexistent file should return IO error, got: {:?}",
        result.err()
    );
}

#[test]
fn parse_file_empty() {
    let dir = tempfile::tempdir().ok();
    let dir = dir.as_ref().map(|d| d.path());
    if let Some(dir) = dir {
        let file_path = dir.join("empty.cha");
        std::fs::write(&file_path, "").ok();
        let options = ParseValidateOptions::default();
        let result = parse_file_and_validate(&file_path, options);
        assert!(result.is_err(), "Empty file should return an error");
    }
}

// ===== Conversion (4 tests) =====

#[test]
fn chat_to_json_valid() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let json = chat_to_json(VALID_CHAT, options, true)?;
    // Should produce valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(parsed.is_object(), "JSON output should be an object");
    Ok(())
}

#[test]
fn chat_to_json_compact_vs_pretty() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let pretty = chat_to_json(VALID_CHAT, options.clone(), true)?;
    let compact = chat_to_json(VALID_CHAT, options, false)?;

    assert!(
        compact.len() < pretty.len(),
        "Compact JSON ({} bytes) should be shorter than pretty ({} bytes)",
        compact.len(),
        pretty.len()
    );
    // Both should be valid JSON
    assert!(serde_json::from_str::<serde_json::Value>(&pretty).is_ok());
    assert!(serde_json::from_str::<serde_json::Value>(&compact).is_ok());
    Ok(())
}

#[test]
fn chat_to_json_unvalidated_produces_json() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let json = chat_to_json_unvalidated(VALID_CHAT, options, true)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(parsed.is_object());
    Ok(())
}

#[test]
fn normalize_chat_idempotent() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let first = normalize_chat(VALID_CHAT, options.clone())?;
    let second = normalize_chat(&first, options)?;
    assert_eq!(
        first, second,
        "Normalizing twice should produce the same output"
    );
    Ok(())
}
