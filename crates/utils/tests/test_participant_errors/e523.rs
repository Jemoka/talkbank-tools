//! Test module for e523 in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::helpers::{TestError, load_error_corpus, validation_parser_suite};

/// Tests e523 orphan id header.
#[test]
fn test_e523_orphan_id_header() -> Result<(), TestError> {
    let content =
        load_error_corpus("tests/error_corpus/E5xx_header_errors/E523_orphan_id_header.cha")?;

    // File structure (byte offsets):
    // @UTF8\n                                               (0-6, 6 bytes)
    // @Begin\n                                              (6-13, 7 bytes)
    // @Languages:\teng\n                                    (13-29, 16 bytes)
    // @Participants:\tCHI Ruth Target_Child\n              (29-68, 39 bytes)
    // @ID:\teng|corpus|CHI|2;06.00||||Target_Child|||\n      (68-114, 46 bytes)
    // @ID:\teng|corpus|MOT|||||Mother|||\n                 (114-149, 35 bytes)
    //                    ^^^ MOT at bytes 132-135 (orphan - not in @Participants)
    // ...

    // Test validation (TreeSitterParser only - TreeSitterParser doesn't do validation yet)
    for parser in validation_parser_suite()? {
        let result = parser.parse_chat_file_result(&content);

        match result {
            Ok(chat_file) => {
                assert!(
                    chat_file.get_participant("CHI").is_some(),
                    "[{}] CHI should have participant entry",
                    parser.name()
                );
                assert!(
                    chat_file.get_participant("MOT").is_none(),
                    "[{}] MOT should not have participant entry (orphan @ID)",
                    parser.name()
                );
            }
            Err(TestError::ParseErrors { errors, .. }) => {
                let e523_errors: Vec<_> = errors
                    .errors
                    .iter()
                    .filter(|e| e.code.as_str() == "E523")
                    .collect();

                assert!(
                    !e523_errors.is_empty(),
                    "[{}] Expected E523 parse error, got: {:?}",
                    parser.name(),
                    errors
                        .errors
                        .iter()
                        .map(|e| e.code.as_str())
                        .collect::<Vec<_>>()
                );

                // Verify error spans are within file bounds
                for e523_err in &e523_errors {
                    let span = &e523_err.location.span;
                    assert!(
                        span.start <= content.len() as u32,
                        "[{}] E523 error start {} exceeds file length {}",
                        parser.name(),
                        span.start,
                        content.len()
                    );
                    assert!(
                        span.end <= content.len() as u32,
                        "[{}] E523 error end {} exceeds file length {}",
                        parser.name(),
                        span.end,
                        content.len()
                    );

                    assert!(
                        span.start <= 132 && span.end >= 135 && span.end > span.start,
                        "[{}] E523 span should cover orphan speaker token bytes 132-135, got {}..{}",
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
