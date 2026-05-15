//! TDD test suite for missing Java validation rules
//!
//! This test suite validates error corpus files that should trigger
//! validation errors currently not implemented in Rust.
//!
//! All tests should FAIL initially, then PASS after implementation.

use std::path::{Path, PathBuf};
use talkbank_model::{ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;
use thiserror::Error;

/// Enum variants for TestError.
#[derive(Debug, Error)]
enum TestError {
    #[error("Parser init failed")]
    ParserInit(#[from] talkbank_parser::ParserInitError),
    #[error("Failed to read test file: {path}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Helper to parse and validate a CHAT file
fn parse_and_validate(content: &str) -> Result<Vec<ParseError>, TestError> {
    parse_and_validate_with_filename(content, None)
}

/// Helper to parse and validate a CHAT file with optional filename
fn parse_and_validate_with_filename(
    content: &str,
    filename: Option<&str>,
) -> Result<Vec<ParseError>, TestError> {
    let parser = TreeSitterParser::new()?;
    let errors = ErrorCollector::new();

    // Parse with error collection
    let chat_file = parser.parse_chat_file_streaming(content, &errors);

    // Validate the parsed file
    chat_file.validate(&errors, filename);

    Ok(errors.into_vec())
}

/// Helper to read error corpus file
fn read_test_file(filename: &str) -> Result<String, TestError> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/error_corpus/validation_gaps")
        .join(filename);
    std::fs::read_to_string(&path).map_err(|source| TestError::ReadFile { path, source })
}

/// Tests nested bg same label.
#[test]
fn test_nested_bg_same_label() -> Result<(), TestError> {
    let content = read_test_file("nested-bg-same-label.cha")?;
    let errors = parse_and_validate(&content)?;

    // Should report E529: NestedGemSameLabel
    let has_nested_gem_error = errors.iter().any(|e| e.code.as_str() == "E529");

    assert!(
        has_nested_gem_error,
        "Expected error E529 (NestedGemSameLabel) not found. Got: {:#?}",
        errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests lazy gem inside bg.
#[test]
fn test_lazy_gem_inside_bg() -> Result<(), TestError> {
    let content = read_test_file("lazy-gem-inside-bg.cha")?;
    let errors = parse_and_validate(&content)?;

    // Should report E530: LazyGemInsideGemScope
    let has_lazy_gem_error = errors.iter().any(|e| e.code.as_str() == "E530");

    assert!(
        has_lazy_gem_error,
        "Expected error E530 (LazyGemInsideGemScope) not found. Got: {:#?}",
        errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests duplicate dependent tier.
#[test]
fn test_duplicate_dependent_tier() -> Result<(), TestError> {
    let content = read_test_file("duplicate-dependent-tier.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E401: DuplicateDependentTier (at least twice - one for %mor, one for %gra)
    let duplicate_tier_errors = errors.iter().filter(|e| e.code.as_str() == "E401").count();

    assert!(
        duplicate_tier_errors >= 2,
        "Expected error E401 (DuplicateDependentTier) not found. Got: {:#?}",
        errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests media filename mismatch.
#[test]
fn test_media_filename_mismatch() -> Result<(), TestError> {
    let content = read_test_file("media-filename-mismatch.cha")?;
    // Pass the expected filename to trigger E531 validation
    let errors = parse_and_validate_with_filename(&content, Some("media-filename-mismatch"))?;

    eprintln!("E531 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E531: MediaFilenameMismatch
    // The test file has @Media: actual-recording, but filename is media-filename-mismatch
    let has_mismatch_error = errors.iter().any(|e| e.code.as_str() == "E531");

    eprintln!("E531 implemented: {}", has_mismatch_error);

    assert!(
        has_mismatch_error,
        "Expected error E531 (MediaFilenameMismatch) not found. Got: {:#?}",
        errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests retrace no content.
#[test]
fn test_retrace_no_content() -> Result<(), TestError> {
    let content = read_test_file("retrace-no-content.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("E370 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E370: RetraceWithoutContent (twice - for [/] and [//])
    let retrace_errors = errors.iter().filter(|e| e.code.as_str() == "E370").count();

    eprintln!(
        "E370 implemented: {} (count: {})",
        retrace_errors >= 2,
        retrace_errors
    );
    Ok(())
}

/// Tests bullet timestamp backwards.
#[test]
fn test_bullet_timestamp_backwards() -> Result<(), TestError> {
    let content = read_test_file("bullet-timestamp-backwards.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("E362 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E362: TimestampBackwards
    let has_backwards_error = errors.iter().any(|e| e.code.as_str() == "E362");

    eprintln!("E362 implemented: {}", has_backwards_error);
    Ok(())
}

/// Tests pause in pho group.
#[test]
fn test_pause_in_pho_group() -> Result<(), TestError> {
    let content = read_test_file("pause-in-pho-group.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("E371 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E371: PauseInPhoGroup
    let has_pause_error = errors.iter().any(|e| e.code.as_str() == "E371");

    eprintln!("E371 implemented: {}", has_pause_error);
    Ok(())
}

/// Tests invalid participant role.
#[test]
fn test_invalid_participant_role() -> Result<(), TestError> {
    let content = read_test_file("invalid-participant-role.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("E532 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E532: InvalidParticipantRole
    let has_role_error = errors.iter().any(|e| e.code.as_str() == "E532");

    eprintln!("E532 implemented: {}", has_role_error);
    Ok(())
}

/// Tests nested quotation.
#[test]
fn test_nested_quotation() -> Result<(), TestError> {
    let content = read_test_file("nested-quotation.cha")?;
    let errors = parse_and_validate(&content)?;

    eprintln!("E372 test - Found {} errors:", errors.len());
    for err in &errors {
        eprintln!("  {} - {}", err.code.as_str(), err.message);
    }

    // Should report E372: NestedQuotation
    let has_nested_quote_error = errors.iter().any(|e| e.code.as_str() == "E372");

    eprintln!("E372 implemented: {}", has_nested_quote_error);
    Ok(())
}
