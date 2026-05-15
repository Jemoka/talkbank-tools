//! Structural roundtrip property tests for main tiers.
//!
//! Generates valid main tier lines from components (speaker, words, terminators,
//! pauses, fillers) and verifies parse → serialize → reparse stability.

use proptest::prelude::*;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

use super::slow_test_config;

/// Strategy for speaker codes (3-letter uppercase).
fn speaker_code() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("CHI"),
        Just("MOT"),
        Just("FAT"),
        Just("INV"),
        Just("EXP"),
    ]
}

/// Strategy for a simple word.
fn word() -> impl Strategy<Value = String> {
    "[a-z]{1,8}"
}

/// Strategy for a terminator.
fn terminator() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("."), Just("?"), Just("!"),]
}

/// Strategy for a content element (word, filler, or pause).
fn content_element() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => word(),
        1 => word().prop_map(|w| format!("&-{}", w)),       // filler
        1 => Just("(.)".to_string()),                         // short pause
    ]
}

/// Strategy for a full main tier line.
fn main_tier_line() -> impl Strategy<Value = String> {
    (
        speaker_code(),
        prop::collection::vec(content_element(), 1..6),
        terminator(),
    )
        .prop_map(|(speaker, words, term)| format!("*{}:\t{} {}", speaker, words.join(" "), term))
}

/// Helper: parse main tier with the tree-sitter parser.
fn parse_main(input: &str) -> Option<String> {
    let ts = TreeSitterParser::new().ok()?;
    let errors = ErrorCollector::new();
    let parsed = ts.parse_main_tier_fragment(input, 0, &errors);
    parsed.into_option().map(|tier| tier.to_chat())
}

proptest! {
    #![proptest_config(slow_test_config())]

    /// Simple main tier roundtrips.
    #[test]
    fn simple_main_tier_roundtrip(line in main_tier_line()) {
        if let Some(output) = parse_main(&line) {
            prop_assert_eq!(
                &output, &line,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                line, output
            );
        }
    }

    /// Main tier with just words and terminator.
    #[test]
    fn plain_words_main_tier_roundtrip(
        speaker in speaker_code(),
        words in prop::collection::vec(word(), 1..5),
        term in terminator()
    ) {
        let line = format!("*{}:\t{} {}", speaker, words.join(" "), term);
        if let Some(output) = parse_main(&line) {
            prop_assert_eq!(
                &output, &line,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                line, output
            );
        }
    }

    /// Main tier with fillers roundtrips.
    #[test]
    fn filler_main_tier_roundtrip(
        filler_word in word(),
        context_word in word(),
        term in terminator()
    ) {
        let line = format!("*CHI:\t&-{} {} {}", filler_word, context_word, term);
        if let Some(output) = parse_main(&line) {
            prop_assert_eq!(
                &output, &line,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                line, output
            );
        }
    }

    /// Double roundtrip is stable for main tiers.
    #[test]
    fn main_tier_double_roundtrip_stable(line in main_tier_line()) {
        if let Ok(ts) = TreeSitterParser::new() {
            let e1 = ErrorCollector::new();
            if let Some(tier1) = ts.parse_main_tier_fragment(&line, 0, &e1).into_option() {
                let chat1 = tier1.to_chat();
                let e2 = ErrorCollector::new();
                if let Some(tier2) = ts.parse_main_tier_fragment(&chat1, 0, &e2).into_option() {
                    let chat2 = tier2.to_chat();
                    prop_assert_eq!(
                        &chat1, &chat2,
                        "double roundtrip unstable: '{}' -> '{}' -> '{}'",
                        line, chat1, chat2
                    );
                }
            }
        }
    }
}
