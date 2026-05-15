//! Test module for sad path in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{
    TestError, is_alignment_error, parse_chat_file, read_file, validate_chat_file_with_alignment,
};
use talkbank_model::ErrorCode;

/// Tests sad path e411 mor too few.
#[test]
fn test_sad_path_e411_mor_too_few() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E411_mor_too_few_items.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %mor tier with too few items"
    );

    assert!(
        alignment_errors
            .iter()
            .any(|e| e.code == ErrorCode::MorCountMismatchTooFew),
        "Should emit E705 for %mor count too few"
    );

    Ok(())
}

/// Tests sad path e412 mor too many.
#[test]
fn test_sad_path_e412_mor_too_many() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E412_mor_too_many_items.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %mor tier with too many items"
    );

    assert!(
        alignment_errors
            .iter()
            .any(|e| e.code == ErrorCode::MorCountMismatchTooMany),
        "Should emit E706 for %mor count too many"
    );

    Ok(())
}

/// Tests sad path e421 gra too few.
#[test]
fn test_sad_path_e421_gra_too_few() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E421_gra_too_few_relations.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %gra tier with too few relations"
    );

    Ok(())
}

/// Tests sad path e422 gra too many.
#[test]
fn test_sad_path_e422_gra_too_many() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E422_gra_too_many_relations.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %gra tier with too many relations"
    );

    Ok(())
}

/// Tests sad path e431 pho too few.
#[test]
fn test_sad_path_e431_pho_too_few() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E431_pho_too_few_forms.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %pho tier with too few forms"
    );

    assert!(
        alignment_errors
            .iter()
            .any(|e| e.code == ErrorCode::PhoCountMismatchTooFew),
        "Should emit E714 for %pho count too few"
    );

    Ok(())
}

/// Tests sad path e432 pho too many.
#[test]
fn test_sad_path_e432_pho_too_many() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E432_pho_too_many_forms.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect %pho tier with too many forms"
    );

    assert!(
        alignment_errors
            .iter()
            .any(|e| e.code == ErrorCode::PhoCountMismatchTooMany),
        "Should emit E715 for %pho count too many"
    );

    Ok(())
}

/// Tests sad path complex misalignment.
#[test]
fn test_sad_path_complex_misalignment() -> Result<(), TestError> {
    let content = read_file("tests/alignment_corpus/sad_path/E441_complex_misalignment.cha")?;
    let mut chat_file = parse_chat_file(&content)?;
    let errors = validate_chat_file_with_alignment(&mut chat_file);

    let alignment_errors: Vec<_> = errors
        .iter()
        .filter(|e| is_alignment_error(e.code))
        .collect();

    assert!(
        !alignment_errors.is_empty(),
        "Should detect complex alignment errors"
    );

    Ok(())
}
