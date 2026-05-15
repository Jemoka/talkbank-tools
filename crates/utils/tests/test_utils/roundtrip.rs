//! Shared roundtrip testing logic - DO NOT DUPLICATE
//!
//! Single source of truth for how roundtrip tests should work.

#![allow(dead_code)] // Some utilities are for future batch workflows

use talkbank_model::ParseValidateOptions;
use talkbank_model::Severity;
use talkbank_model::model::ChatFile;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::{PipelineError, parse_and_validate, parse_and_validate_with_parser};

/// Parse and validate a CHAT file with consistent error handling.
///
/// This is the ONLY way roundtrip tests should parse files.
///
/// Returns:
/// - Ok(ChatFile) if parsing succeeds (with or without warnings)
/// - Err(PipelineError) if parsing or validation fails
pub fn parse_for_roundtrip(
    content: &str,
    check_alignment: bool,
) -> Result<ChatFile, PipelineError> {
    let options = if check_alignment {
        ParseValidateOptions::default().with_alignment()
    } else {
        ParseValidateOptions::default().with_validation()
    };

    match parse_and_validate(content, options) {
        Ok(file) => Ok(file),
        Err(PipelineError::Validation(errors)) => {
            // Only fail if there were actual errors (not warnings)
            let has_error = errors.iter().any(|e| e.severity == Severity::Error);

            if has_error {
                Err(PipelineError::Validation(errors))
            } else {
                parse_and_validate(content, ParseValidateOptions::default())
            }
        }
        Err(e) => Err(e),
    }
}

/// Parser-reuse variant of parse_for_roundtrip for batch workflows.
pub fn parse_for_roundtrip_with_parser(
    parser: &TreeSitterParser,
    content: &str,
    check_alignment: bool,
) -> Result<ChatFile, PipelineError> {
    let options = if check_alignment {
        ParseValidateOptions::default().with_alignment()
    } else {
        ParseValidateOptions::default().with_validation()
    };

    match parse_and_validate_with_parser(parser, content, options) {
        Ok(file) => Ok(file),
        Err(PipelineError::Validation(errors)) => {
            // Only fail if there were actual errors (not warnings)
            let has_error = errors.iter().any(|e| e.severity == Severity::Error);

            if has_error {
                Err(PipelineError::Validation(errors))
            } else {
                parse_and_validate_with_parser(parser, content, ParseValidateOptions::default())
            }
        }
        Err(e) => Err(e),
    }
}
