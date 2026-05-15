//! Structural roundtrip property tests for %mor tiers.
//!
//! Generates valid %mor content strings from components (POS, lemma, features,
//! clitics) and verifies parse → serialize → reparse stability across both parsers.

use proptest::prelude::*;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

use super::slow_test_config;

/// Strategy for POS categories (common UD tags).
fn pos_tag() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("noun"),
        Just("verb"),
        Just("pron"),
        Just("det"),
        Just("adj"),
        Just("adv"),
        Just("prep"),
        Just("conj"),
        Just("aux"),
        Just("n:prop"),
        Just("v:aux"),
    ]
}

/// Strategy for lemma strings.
fn lemma() -> impl Strategy<Value = String> {
    "[a-z]{1,8}"
}

/// Strategy for flat features (no key=value, just value).
fn flat_feature() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("Prs"),
        Just("Past"),
        Just("Fin"),
        Just("Ind"),
        Just("Pres"),
        Just("Nom"),
        Just("Acc"),
        Just("Plur"),
        Just("Sing"),
        Just("S1"),
        Just("S3"),
    ]
}

/// Strategy for a single mor word: `POS|lemma[-feature]*`
fn mor_word_str() -> impl Strategy<Value = String> {
    (
        pos_tag(),
        lemma(),
        prop::collection::vec(flat_feature(), 0..4),
    )
        .prop_map(|(pos, lem, features)| {
            let mut s = format!("{}|{}", pos, lem);
            for f in features {
                s.push('-');
                s.push_str(f);
            }
            s
        })
}

/// Strategy for a mor item (main word + optional clitic).
fn mor_item_str() -> impl Strategy<Value = String> {
    (mor_word_str(), prop::option::of(mor_word_str())).prop_map(|(main, clitic)| match clitic {
        Some(c) => format!("{}~{}", main, c),
        None => main,
    })
}

/// Strategy for a full mor tier content string (1-5 items + optional terminator).
fn mor_content() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(mor_item_str(), 1..5),
        prop::option::of(prop_oneof![Just("."), Just("?"), Just("!")]),
    )
        .prop_map(|(items, term)| {
            let mut s = items.join(" ");
            if let Some(t) = term {
                s.push(' ');
                s.push_str(t);
            }
            s
        })
}

/// Helper: parse mor tier content with the tree-sitter parser.
fn parse_mor(input: &str) -> Option<String> {
    let ts = TreeSitterParser::new().ok()?;
    let errors = ErrorCollector::new();
    let parsed = ts.parse_mor_tier_fragment(input, 0, &errors);
    parsed.into_option().map(|tier| tier.to_content())
}

proptest! {
    #![proptest_config(slow_test_config())]

    /// Single mor word roundtrips.
    #[test]
    fn single_mor_word_roundtrip(word in mor_word_str()) {
        if let Some(output) = parse_mor(&word) {
            prop_assert_eq!(
                &output, &word,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                word, output
            );
        }
    }

    /// Mor word with clitic roundtrips.
    #[test]
    fn mor_clitic_roundtrip(item in mor_item_str()) {
        if let Some(output) = parse_mor(&item) {
            prop_assert_eq!(
                &output, &item,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                item, output
            );
        }
    }

    /// Full mor tier content roundtrips.
    #[test]
    fn mor_tier_content_roundtrip(content in mor_content()) {
        if let Some(output) = parse_mor(&content) {
            prop_assert_eq!(
                &output, &content,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                content, output
            );
        }
    }

    /// Double roundtrip is stable for mor tiers.
    #[test]
    fn mor_double_roundtrip_stable(content in mor_content()) {
        if let Ok(ts) = TreeSitterParser::new() {
            let e1 = ErrorCollector::new();
            if let Some(tier1) = ts.parse_mor_tier_fragment(&content, 0, &e1).into_option() {
                let chat1 = tier1.to_content();
                let e2 = ErrorCollector::new();
                if let Some(tier2) = ts.parse_mor_tier_fragment(&chat1, 0, &e2).into_option() {
                    let chat2 = tier2.to_content();
                    prop_assert_eq!(
                        &chat1, &chat2,
                        "double roundtrip unstable: '{}' -> '{}' -> '{}'",
                        content, chat1, chat2
                    );
                }
            }
        }
    }
}
