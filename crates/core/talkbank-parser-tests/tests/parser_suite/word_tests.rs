//! Golden word roundtrip tests.
//!
//! Verifies that every word in the golden corpus round-trips through
//! `parse_word` -> `WriteChat` for both parser backends.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::WriteChat;
use talkbank_parser_tests::GoldenBugs;
use talkbank_parser_tests::golden::golden_words;
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

/// Print a summary of bug annotations and their impact on testing
fn print_bug_summary(bugs: &GoldenBugs, words: &[&str]) {
    let skip_count = words.iter().filter(|w| bugs.should_skip(w)).count();
    let wrong_count = words.iter().filter(|w| bugs.is_expected_wrong(w)).count();

    eprintln!("\n=== Bug Annotation Summary ===");
    eprintln!("Total golden words: {}", words.len());
    eprintln!("Skipped (known bugs): {}", skip_count);
    eprintln!("Expected wrong: {}", wrong_count);
    eprintln!("Tested: {}", words.len() - skip_count);
    eprintln!("==============================\n");
}

/// Verifies golden words round-trip for every parser backend.
#[test]
fn golden_word_roundtrip_for_every_parser() -> Result<(), TestError> {
    let bugs = GoldenBugs::load().map_err(|err| TestError::Failure(err.to_string()))?;
    let words = golden_words();

    // Print summary at start
    print_bug_summary(&bugs, &words);

    for parser in parser_suite()? {
        for word in &words {
            // Skip words with known bugs
            if bugs.should_skip(word) {
                eprintln!("[{}] SKIP (known bug): {}", "tree-sitter", word);
                continue;
            }

            // Parse the word
            let sink = ErrorCollector::new();
            let parsed = parser.parse_word_fragment(word, 0, &sink);

            assert!(
                sink.is_empty(),
                "[{}] unexpected errors parsing `{}`: {:?}",
                "tree-sitter",
                word,
                sink.to_vec()
            );

            let parsed = match parsed {
                ParseOutcome::Parsed(parsed) => parsed,
                ParseOutcome::Rejected => {
                    return Err(TestError::Failure(format!(
                        "[{}] parser rejected word `{}` despite no sink errors",
                        "tree-sitter", word
                    )));
                }
            };

            // Roundtrip test
            let mut serialized = String::new();
            parsed.write_chat(&mut serialized)?;
            assert_eq!(
                serialized, *word,
                "[{}] word roundtrip changed representation",
                "tree-sitter"
            );
        }
    }
    Ok(())
}
