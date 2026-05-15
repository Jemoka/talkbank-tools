//! Test module for happy path in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{
    TestError, is_alignment_error, parser_suite, read_file, validate_chat_file_with_alignment,
};

/// Tests happy path main mor aligned.
#[test]
fn test_happy_path_main_mor_aligned() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/happy_path/main_mor_aligned.cha")?;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let mut chat_file = parser.parse_chat_file_result(&content)?;

        let errors = validate_chat_file_with_alignment(&mut chat_file);

        let alignment_errors: Vec<_> = errors
            .iter()
            .filter(|e| is_alignment_error(e.code))
            .collect();

        assert!(
            alignment_errors.is_empty(),
            "[{}] Happy path should have no alignment errors. Got: {:#?}",
            parser.name(),
            alignment_errors
        );
    }

    Ok(())
}

/// Tests happy path mor gra aligned.
#[test]
fn test_happy_path_mor_gra_aligned() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/happy_path/mor_gra_aligned.cha")?;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let mut chat_file = parser.parse_chat_file_result(&content)?;

        let errors = validate_chat_file_with_alignment(&mut chat_file);

        let alignment_errors: Vec<_> = errors
            .iter()
            .filter(|e| is_alignment_error(e.code))
            .collect();

        assert!(
            alignment_errors.is_empty(),
            "[{}] Happy path should have no alignment errors. Got: {:#?}",
            parser.name(),
            alignment_errors
        );
    }

    Ok(())
}

/// Tests happy path main pho aligned.
#[test]
fn test_happy_path_main_pho_aligned() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/happy_path/main_pho_aligned.cha")?;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let mut chat_file = parser.parse_chat_file_result(&content)?;

        let errors = validate_chat_file_with_alignment(&mut chat_file);

        let alignment_errors: Vec<_> = errors
            .iter()
            .filter(|e| is_alignment_error(e.code))
            .collect();

        assert!(
            alignment_errors.is_empty(),
            "[{}] Happy path should have no alignment errors. Got: {:#?}",
            parser.name(),
            alignment_errors
        );
    }

    Ok(())
}
