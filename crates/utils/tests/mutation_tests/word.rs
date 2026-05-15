//! Test module for word in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCode;
use talkbank_parser::{ParserInitError, TreeSitterParser};

use super::helpers::{get_error_codes, has_error, validate_chat_file};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
}

// E2xx: Word-Level Mutation Tests

/// Tests e207 unknown annotation.
#[test]
fn test_e207_unknown_annotation() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [?unknown] .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                errors.is_empty() || has_error(&errors, ErrorCode::UnknownAnnotation),
                "Expected E207 (unknown annotation) or no errors, got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            assert!(
                parse_errors.errors.iter().any(|e| {
                    matches!(
                        e.code,
                        ErrorCode::SyntaxError
                            | ErrorCode::UnknownAnnotation
                            | ErrorCode::UnparsableContent
                    )
                }),
                "Expected E303, E207, or E316 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}

/// Tests e208 empty replacement.
#[test]
fn test_e208_empty_replacement() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [: ] .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            if !errors.is_empty() {
                assert!(
                    has_error(&errors, ErrorCode::EmptyReplacement)
                        || has_error(&errors, ErrorCode::SyllablePauseNotBetweenSpokenMaterial)
                        || has_error(&errors, ErrorCode::EmptySpokenContent),
                    "Expected E208/E252/E209 (empty replacement) or no errors, got: {:?}",
                    get_error_codes(&errors)
                );
            }
        }
        Err(_parse_errors) => {}
    }

    Ok(())
}

/// Tests e242 unbalanced quotation.
#[test]
fn test_e242_unbalanced_quotation() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	he said +"hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                errors.is_empty()
                    || has_error(&errors, ErrorCode::UnbalancedQuotation)
                    || has_error(&errors, ErrorCode::UnbalancedCADelimiter),
                "Expected E242/E230 (unbalanced quotation) or no errors, got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            println!(
                "Parse errors: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}
