//! Legacy word-level parser equivalence tests.
//!
//! Tests each minimal golden word individually, reporting which specific words fail.
//! Uses the minimal list (~47 words) with ONE word per feature signature for fast core tests.
//!
//! This file is still useful for spotting divergence, but it should not be
//! treated as the oracle for direct-parser fragment semantics. Tree-sitter
//! fragment behavior is itself synthetic in some paths.
//!
//! ## Usage
//!
//! ```bash
//! # Run all word tests
//! cargo test parser_equivalence_words
//!
//! # Show failing words
//! cargo test parser_equivalence_words -- --nocapture
//! ```

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::golden::golden_words_minimal;
use talkbank_parser_tests::test_error::TestError;

/// Verify TreeSitterParser parses all golden words successfully.
///
/// ## Test Strategy
///
/// For each word in golden_words_minimal():
/// 1. Parse with TreeSitterParser
/// 2. Report EACH failure individually (not just a count)
#[test]
fn all_words_equivalence() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;

    let words = golden_words_minimal();
    let mut failures = Vec::new();

    for word in &words {
        let errors = ErrorCollector::new();
        let result = parser.parse_word_fragment(word, 0, &errors);

        match result {
            ParseOutcome::Parsed(_) => {
                // Successfully parsed
            }
            ParseOutcome::Rejected => {
                let error_list = errors.to_vec();
                failures.push(format!(
                    "Word: {}
  ERROR: Failed to parse
  Errors: {:?}",
                    word, error_list
                ));
            }
        }
    }

    if !failures.is_empty() {
        return Err(TestError::Failure(format!(
            "{} out of {} words failed parsing:\n\n{}\n",
            failures.len(),
            words.len(),
            failures.join("\n\n")
        )));
    }

    Ok(())
}
