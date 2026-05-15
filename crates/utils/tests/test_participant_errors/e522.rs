//! Test module for e522 in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::helpers::{TestError, load_error_corpus, validation_parser_suite};

/// Tests e522 missing id for participant.
#[test]
fn test_e522_missing_id_for_participant() -> Result<(), TestError> {
    let content = load_error_corpus(
        "tests/error_corpus/E5xx_header_errors/E522_missing_id_for_participant.cha",
    )?;

    // File structure (byte offsets):
    // @UTF8\n                                             (0-6, 6 bytes)
    // @Begin\n                                            (6-13, 7 bytes)
    // @Languages:\teng\n                                  (13-29, 16 bytes)
    // @Participants:\tCHI Ruth Target_Child, MOT Mother\n (29-80, 51 bytes)
    //                 ^^^ CHI at bytes 44-47
    // @ID:\teng|corpus|MOT|||||Mother|||\n                (80-115, 35 bytes)
    // ...

    // Test validation (TreeSitterParser only - TreeSitterParser doesn't do validation yet)
    for parser in validation_parser_suite()? {
        let result = parser.parse_chat_file_result(&content);

        // E522 is a critical error, so parse should fail.
        assert!(
            result.is_err(),
            "[{}] Parse should fail with E522 error",
            parser.name()
        );

        let errors = match result {
            Err(TestError::ParseErrors { errors, .. }) => errors,
            Err(err) => return Err(err),
            Ok(_) => {
                return Err(TestError::UnexpectedParseSuccess {
                    parser: parser.name(),
                });
            }
        };

        // Should contain E522 error.
        let e522_errors: Vec<_> = errors
            .errors
            .iter()
            .filter(|e| e.code.to_string() == "E522")
            .collect();

        assert!(
            !e522_errors.is_empty(),
            "[{}] Should have E522 error for missing @ID",
            parser.name()
        );

        // Verify error span is within the file bounds
        for e522_err in &e522_errors {
            let span = &e522_err.location.span;
            assert!(
                span.start <= content.len() as u32,
                "[{}] E522 error start {} exceeds file length {}",
                parser.name(),
                span.start,
                content.len()
            );
            assert!(
                span.end <= content.len() as u32,
                "[{}] E522 error end {} exceeds file length {}",
                parser.name(),
                span.end,
                content.len()
            );

            assert!(
                span.start >= 29 && span.end <= 80 && span.end > span.start,
                "[{}] E522 span should point to @Participants line (29-80), got {}..{}",
                parser.name(),
                span.start,
                span.end
            );
        }
    }

    Ok(())
}
