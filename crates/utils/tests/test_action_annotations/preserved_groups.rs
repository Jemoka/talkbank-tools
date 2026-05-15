//! Test module for preserved groups in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::helpers::{
    TestError, action_annotations_input, parse_chat_file_streaming_or_err, parser_suite,
};
use talkbank_model::WriteChat;

/// Tests action annotations preserved in groups.
#[test]
fn test_action_annotations_preserved_in_groups() -> Result<(), TestError> {
    let input = action_annotations_input();

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        let output = chat_file.to_chat_string();

        assert!(
            output.contains("<0 [= ! whining]>"),
            "[{}] Action annotation [= ! whining] was lost!
Output:\n{}",
            parser.name(),
            output
        );
        assert!(
            output.contains("<0 [= ! meowing]>"),
            "[{}] Action annotation [= ! meowing] was lost!
Output:\n{}",
            parser.name(),
            output
        );
    }

    Ok(())
}
