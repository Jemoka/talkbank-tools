//! Test module for e524 in `talkbank-tools`.
//!
//! These tests document expected behavior and regressions.

use crate::helpers::{TestError, load_error_corpus, validation_parser_suite};

/// Tests e524 birth unknown participant.
#[test]
fn test_e524_birth_unknown_participant() -> Result<(), TestError> {
    let content = load_error_corpus(
        "tests/error_corpus/E5xx_header_errors/E524_birth_unknown_participant.cha",
    )?;

    // File structure (byte offsets):
    // @UTF8\n                                               (0-6, 6 bytes)
    // @Begin\n                                              (6-13, 7 bytes)
    // @Languages:\teng\n                                    (13-29, 16 bytes)
    // @Participants:\tCHI Ruth Target_Child\n              (29-68, 39 bytes)
    // @ID:\teng|corpus|CHI|2;06.00||||Target_Child|||\n      (68-114, 46 bytes)
    // @Birth of MOT:\t01-JAN-2000\n                        (114-143, 29 bytes)
    //           ^^^ MOT at bytes 124-127 (unknown participant - not in @Participants)
    // ...

    // Test validation (TreeSitterParser only - TreeSitterParser doesn't do validation yet)
    for parser in validation_parser_suite()? {
        let result = parser.parse_chat_file_result(&content);

        match result {
            Ok(chat_file) => {
                let chi =
                    chat_file
                        .get_participant("CHI")
                        .ok_or(TestError::MissingParticipant {
                            parser: parser.name(),
                            participant: "CHI",
                        })?;
                assert_eq!(
                    chi.birth_date,
                    None,
                    "[{}] CHI should not have birth date",
                    parser.name()
                );
            }
            Err(TestError::ParseErrors { errors, .. }) => {
                let parse_errors = errors;
                let e524_errors: Vec<_> = parse_errors
                    .errors
                    .iter()
                    .filter(|e| e.code.as_str() == "E524")
                    .collect();

                assert!(
                    !e524_errors.is_empty(),
                    "[{}] Expected E524 parse error, got: {:?}",
                    parser.name(),
                    parse_errors
                        .errors
                        .iter()
                        .map(|e| e.code.as_str())
                        .collect::<Vec<_>>()
                );

                // Verify error spans are within file bounds
                for e524_err in &e524_errors {
                    let span = &e524_err.location.span;
                    assert!(
                        span.start <= content.len() as u32,
                        "[{}] E524 error start {} exceeds file length {}",
                        parser.name(),
                        span.start,
                        content.len()
                    );
                    assert!(
                        span.end <= content.len() as u32,
                        "[{}] E524 error end {} exceeds file length {}",
                        parser.name(),
                        span.end,
                        content.len()
                    );

                    assert!(
                        span.start <= 124 && span.end >= 127 && span.end > span.start,
                        "[{}] E524 span should cover unknown participant token bytes 124-127, got {}..{}",
                        parser.name(),
                        span.start,
                        span.end
                    );
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(())
}
