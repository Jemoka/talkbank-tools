//! Test module for dependent tiers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parse_chat_file_streaming_or_err, parser_suite};

/// Tests parse file with dependent tiers.
#[test]
fn test_parse_file_with_dependent_tiers() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 *CHI:\thello .\n\
                 %mor:\tintj|hello .\n\
                 *MOT:\thi .\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        assert_eq!(
            chat_file.utterance_count(),
            2,
            "[{}] expected 2 utterances",
            parser.name()
        );

        let utterances: Vec<_> = chat_file.utterances().collect();
        assert_eq!(
            utterances[0].main.speaker.as_str(),
            "CHI",
            "[{}] expected speaker CHI",
            parser.name()
        );

        assert!(
            utterances[0].mor_tier().is_some(),
            "[{}] First utterance should have %mor tier attached",
            parser.name()
        );
    }

    Ok(())
}
