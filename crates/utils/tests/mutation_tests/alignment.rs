//! Test module for alignment in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCode;
use talkbank_parser::{ParserInitError, TreeSitterParser};

use super::helpers::{get_error_codes, has_error, validate_chat_file_with_alignment};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
    #[error("Parse failed")]
    ParseErrors { errors: talkbank_model::ParseErrors },
}

// E6xx: Alignment Mutation Tests

/// Tests e705 mor count mismatch too few.
#[test]
fn test_e705_mor_count_mismatch_too_few() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	the dog runs .
%mor:	det|the n|dog .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let mut chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;

    let errors = validate_chat_file_with_alignment(&mut chat_file);

    assert!(
        has_error(&errors, ErrorCode::MorCountMismatchTooFew),
        "Expected E705 (mor tier too short), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e706 mor count mismatch too many.
#[test]
fn test_e706_mor_count_mismatch_too_many() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	the dog .
%mor:	det|the n|dog v|run-3S .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let mut chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;

    let errors = validate_chat_file_with_alignment(&mut chat_file);

    assert!(
        has_error(&errors, ErrorCode::MorCountMismatchTooMany),
        "Expected E706 (mor tier too long), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e712 gra invalid word index.
#[test]
fn test_e712_gra_invalid_word_index() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	the dog runs .
%mor:	det|the n|dog v|run-3S .
%gra:	1|2|DET 2|3|SUBJ 3|0|ROOT 10|3|PUNCT
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let mut chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;

    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == ErrorCode::GraInvalidWordIndex)
        .collect();
    assert!(
        !alignment_errors.is_empty(),
        "Expected E712 (gra invalid word index), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}
