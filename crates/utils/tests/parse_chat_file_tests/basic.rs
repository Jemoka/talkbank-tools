//! Test module for basic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{
    TestError, parse_chat_file_result, parse_chat_file_streaming_or_err, parser_suite,
};

/// Tests parse minimal chat file.
#[test]
fn test_parse_minimal_chat_file() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        let headers: Vec<_> = chat_file.headers().collect();
        assert_eq!(headers.len(), 3, "[{}] expected 3 headers", parser.name());
        assert_eq!(headers[0].name(), "UTF8");
        assert_eq!(headers[1].name(), "Begin");
        assert_eq!(headers[2].name(), "End");
        assert_eq!(
            chat_file.utterance_count(),
            0,
            "[{}] expected 0 utterances",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse file with one utterance.
#[test]
fn test_parse_file_with_one_utterance() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 @Languages:\teng\n\
                 @Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n\
                 *CHI:\thello world .\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        assert_eq!(
            chat_file.header_count(),
            6,
            "[{}] expected 6 headers",
            parser.name()
        );
        assert_eq!(
            chat_file.utterance_count(),
            1,
            "[{}] expected 1 utterance",
            parser.name()
        );

        let utterances: Vec<_> = chat_file.utterances().collect();
        assert_eq!(
            utterances[0].main.speaker.as_str(),
            "CHI",
            "[{}] expected speaker CHI",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse file with multiple utterances.
#[test]
fn test_parse_file_with_multiple_utterances() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 *CHI:\thello .\n\
                 *MOT:\thi there .\n\
                 *CHI:\tgoodbye .\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        assert_eq!(
            chat_file.header_count(),
            3,
            "[{}] expected 3 headers",
            parser.name()
        );
        assert_eq!(
            chat_file.utterance_count(),
            3,
            "[{}] expected 3 utterances",
            parser.name()
        );

        let utterances: Vec<_> = chat_file.utterances().collect();
        assert_eq!(
            utterances[0].main.speaker.as_str(),
            "CHI",
            "[{}] utterance 0 speaker",
            parser.name()
        );
        assert_eq!(
            utterances[1].main.speaker.as_str(),
            "MOT",
            "[{}] utterance 1 speaker",
            parser.name()
        );
        assert_eq!(
            utterances[2].main.speaker.as_str(),
            "CHI",
            "[{}] utterance 2 speaker",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse file with one utterance result api.
#[test]
fn test_parse_file_with_one_utterance_result_api() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @Begin\n\
                 *CHI:\thello .\n\
                 @End\n";

    let result = parse_chat_file_result(input);
    assert!(result.is_ok(), "Should parse minimal file");

    Ok(())
}
