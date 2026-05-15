//! Parser snapshot tests.
//!
//! Captures insta snapshots of parser output for representative inputs
//! to detect unexpected changes in AST structure across both backends.

use insta::assert_snapshot;
use serde::Serialize;
use serde_json::to_string_pretty;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser_tests::GoldenBugs;
use talkbank_parser_tests::golden::golden_words;
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::{MOR_TIER_INPUT, SAMPLE_WORD_COUNT, UTTERANCE_INPUT, parser_suite};

/// Formats snapshot entry.
fn format_snapshot_entry<T: Serialize>(
    parser_name: &str,
    label: &str,
    value: &T,
) -> Result<String, TestError> {
    Ok(format!(
        "[{parser_name}] {label}\n{}",
        to_string_pretty(value)?
    ))
}

/// Captures parser snapshots for representative word inputs.
#[test]
fn parser_word_snapshot() -> Result<(), TestError> {
    let bugs = GoldenBugs::load().map_err(|err| TestError::Failure(err.to_string()))?;
    let words: Vec<&'static str> = golden_words().into_iter().take(SAMPLE_WORD_COUNT).collect();
    let mut rows = Vec::new();

    for parser in parser_suite()? {
        for word in &words {
            let sink = ErrorCollector::new();
            let parsed = match parser.parse_word_fragment(word, 0, &sink) {
                ParseOutcome::Parsed(parsed) => parsed,
                ParseOutcome::Rejected => {
                    return Err(TestError::Failure(format!(
                        "parser rejected `{}` despite no sink errors",
                        word
                    )));
                }
            };

            assert!(
                sink.is_empty(),
                "[{}] unexpected errors parsing `{}`: {:?}",
                "tree-sitter",
                word,
                sink.to_vec()
            );

            let mut label = format!("word `{word}`");
            if bugs.is_expected_wrong(word) {
                label.push_str(" [KNOWN BUG]");
            }

            rows.push(format_snapshot_entry("tree-sitter", &label, &parsed)?);
        }
    }

    assert_snapshot!(rows.join("\n\n"));
    Ok(())
}

/// Captures parser snapshots for representative `%mor` tier inputs.
#[test]
fn parser_mor_snapshot() -> Result<(), TestError> {
    let mut rows = Vec::new();

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let mor = match parser.parse_mor_tier_fragment(MOR_TIER_INPUT, 0, &sink) {
            ParseOutcome::Parsed(mor) => mor,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "parser {} rejected %mor input despite no sink errors",
                    "tree-sitter"
                )));
            }
        };

        assert!(
            sink.is_empty(),
            "[{}] %mor parsing errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        rows.push(format_snapshot_entry("tree-sitter", "%mor tier", &mor)?);
    }

    assert_snapshot!(rows.join("\n\n"));
    Ok(())
}

/// Captures parser snapshots for representative utterance inputs.
#[test]
fn parser_utterance_snapshot() -> Result<(), TestError> {
    let mut rows = Vec::new();

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let utterance = match parser.parse_utterance_fragment(UTTERANCE_INPUT, 0, &sink) {
            ParseOutcome::Parsed(utterance) => utterance,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "parser {} rejected utterance despite no sink errors",
                    "tree-sitter"
                )));
            }
        };

        assert!(
            sink.is_empty(),
            "[{}] utterance parsing errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        rows.push(format_snapshot_entry(
            "tree-sitter",
            "utterance",
            &utterance,
        )?);
    }

    assert_snapshot!(rows.join("\n\n"));
    Ok(())
}
