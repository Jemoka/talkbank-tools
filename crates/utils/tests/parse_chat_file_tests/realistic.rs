//! Test module for realistic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parse_chat_file_streaming_or_err, parser_suite};

/// Tests parse real chat file structure.
#[test]
fn test_parse_real_chat_file_structure() -> Result<(), TestError> {
    let input = "@UTF8\n\
                 @PID:\t11312/c-00016447-1\n\
                 @Begin\n\
                 @Languages:\teng\n\
                 @Participants:\tFAT Father, CHI Target_Child, MOT Mother\n\
                 @ID:\teng|MacWhinney|FAT|||||Father|||\n\
                 @ID:\teng|MacWhinney|CHI|1;04.11|male|TD||Target_Child|||\n\
                 @ID:\teng|MacWhinney|MOT|||||Mother|||\n\
                 @Media:\t010411a, audio\n\
                 @Date:\t06-MAY-1979\n\
                 *FAT:\twanna give me a kiss ?\n\
                 %mor:\tintj|wanna verb|give-Fin-Imp-S pron|I-Prs-Acc-S1 det|a-Ind-Art noun|kiss-Acc ?\n\
                 %gra:\t1|2|DISCOURSE 2|5|ROOT 3|2|IOBJ 4|5|DET 5|2|OBJ 6|2|PUNCT\n\
                 *CHI:\tnice .\n\
                 %mor:\tadj|nice-S1 .\n\
                 %gra:\t1|1|ROOT 2|1|PUNCT\n\
                 @End\n";

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, input)?;

        let headers: Vec<_> = chat_file.headers().collect();
        assert!(
            headers.len() >= 10,
            "[{}] Should have multiple headers",
            parser.name()
        );
        assert_eq!(headers[0].name(), "UTF8");
        assert_eq!(headers[1].name(), "PID");

        assert_eq!(
            chat_file.utterance_count(),
            2,
            "[{}] expected 2 utterances",
            parser.name()
        );

        let utterances: Vec<_> = chat_file.utterances().collect();
        assert_eq!(
            utterances[0].main.speaker.as_str(),
            "FAT",
            "[{}] utterance 0 speaker",
            parser.name()
        );
        assert_eq!(
            utterances[1].main.speaker.as_str(),
            "CHI",
            "[{}] utterance 1 speaker",
            parser.name()
        );

        assert!(
            utterances[0].mor_tier().is_some(),
            "[{}] FAT utterance should have %mor",
            parser.name()
        );
        assert!(
            utterances[0].gra_tier().is_some(),
            "[{}] FAT utterance should have %gra",
            parser.name()
        );
        assert!(
            utterances[1].mor_tier().is_some(),
            "[{}] CHI utterance should have %mor",
            parser.name()
        );
        assert!(
            utterances[1].gra_tier().is_some(),
            "[{}] CHI utterance should have %gra",
            parser.name()
        );
    }

    Ok(())
}
