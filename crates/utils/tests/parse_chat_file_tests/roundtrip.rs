//! Test module for roundtrip in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parse_chat_file_streaming_or_err, parser_suite};

/// Tests roundtrip minimal file.
#[test]
fn test_roundtrip_minimal_file() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 *CHI:\thello .\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        let serialized = chat_file.to_chat();

        assert_eq!(
            serialized,
            input,
            "[{}] Minimal file should roundtrip exactly",
            parser.name()
        );
    }

    Ok(())
}
