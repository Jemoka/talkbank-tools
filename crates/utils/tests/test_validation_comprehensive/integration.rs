//! Test module for integration in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCode;
use talkbank_model::model::{ChatOptionFlag, Header};

use super::helpers::{TestError, parse_and_validate, parse_only};

/// Tests integration participant completeness.
#[test]
fn test_integration_participant_completeness() -> Result<(), TestError> {
    let valid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother, FAT Father
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@ID:	eng|corpus|FAT|||||Father|||
*CHI:	hello .
*MOT:	hi .
*FAT:	hey .
@End
"#;

    let errors = parse_and_validate(valid_chat)?;

    let e522_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == ErrorCode::SpeakerNotDefined)
        .collect();
    assert_eq!(e522_errors.len(), 0, "All participants have @ID headers");

    Ok(())
}

/// Tests integration missing id for participant.
#[test]
fn test_integration_missing_id_for_participant() -> Result<(), TestError> {
    let invalid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
@End
"#;

    let result = parse_only(invalid_chat);

    assert!(result.is_err(), "Should fail with E522 during parsing");
    match result {
        Err(TestError::ParseErrors { errors, .. }) => {
            let e522_errors: Vec<_> = errors
                .errors
                .iter()
                .filter(|e| e.code == ErrorCode::SpeakerNotDefined)
                .collect();
            assert!(
                !e522_errors.is_empty(),
                "Should have E522 error for missing @ID"
            );
        }
        Err(err) => return Err(err),
        Ok(_) => {
            return Err(TestError::UnexpectedParseSuccess {
                parser: "tree-sitter",
            });
        }
    }

    Ok(())
}

/// Tests ca mode detected and permits legacy forms.
#[test]
fn test_ca_mode_detected_and_permits_legacy_forms() -> Result<(), TestError> {
    let ca_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	CA can use xx and XXX
@End
"#;

    let chat_file = parse_only(ca_chat)?;

    let options_value = chat_file
        .headers()
        .find_map(|header| {
            if let Header::Options { options } = header {
                Some(options.clone())
            } else {
                None
            }
        })
        .ok_or(TestError::MissingOptionsHeader)?;

    assert_eq!(
        options_value.as_slice(),
        [ChatOptionFlag::Ca].as_slice(),
        "@Options contents should capture CA flag exactly"
    );

    let errors = parse_and_validate(ca_chat)?;

    assert!(
        errors
            .iter()
            .all(|e| e.code != ErrorCode::IllegalUntranscribed),
        "CA mode should not emit E241 for xx/XXX legacy tokens, got: {:?}",
        errors
    );
    assert!(
        errors.iter().all(|e| e.code != ErrorCode::MissingSpeaker),
        "CA mode should not emit E304 for missing terminator, got: {:?}",
        errors
    );

    Ok(())
}

/// Tests that CLAN enforces header ordering: @Participants must appear before @ID.
///
/// CLAN CHECK errors 61 and 125 require @Participants before @Options and @ID
/// respectively. Placing @ID before @Participants produces E543.
#[test]
fn test_integration_header_ordering() -> Result<(), TestError> {
    // @ID before @Participants — CLAN rejects this order.
    let misordered_chat = r#"@UTF8
@Begin
@Date:	15-JAN-2020
@Languages:	eng
@ID:	eng|corpus|CHI|||||Child|||
@Participants:	CHI Child
@Comment:	Headers in unusual order
*CHI:	hello .
@End
"#;

    let errors = parse_and_validate(misordered_chat)?;

    let e543_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == ErrorCode::HeaderOutOfOrder)
        .collect();
    assert_eq!(
        e543_errors.len(),
        1,
        "Expected E543 for @ID before @Participants"
    );

    // Correct order: @Participants before @ID — no errors.
    let correct_chat = r#"@UTF8
@Begin
@Date:	15-JAN-2020
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Headers in correct order
*CHI:	hello .
@End
"#;

    let errors = parse_and_validate(correct_chat)?;

    let e543_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == ErrorCode::HeaderOutOfOrder)
        .collect();
    assert_eq!(
        e543_errors.len(),
        0,
        "Correct header order should produce no E543"
    );

    Ok(())
}
