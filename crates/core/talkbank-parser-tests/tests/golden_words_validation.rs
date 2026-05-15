//! Test module for golden words validation in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.\n
use talkbank_parser::TreeSitterParser;
/// Test that ensures golden_words_minimal.txt stays valid according to TreeSitterParser.
///
/// This test fails if:
/// - Any word in golden_words_minimal.txt cannot be parsed by TreeSitterParser
/// - Any word produces parsing errors
///
/// If this test fails after grammar/parser changes, regenerate golden words:
/// ```bash
/// cargo run --release -p talkbank-parser-tests --bin audit_golden_words
/// ```
use talkbank_parser_tests::test_error::TestError;

/// Verifies all minimal golden words remain parse-valid for the TreeSitter parser.
#[test]
fn golden_words_are_valid_for_tree_sitter() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let golden_words = talkbank_parser_tests::golden::golden_words_minimal();

    let mut invalid_count = 0;

    for word in &golden_words {
        let result = parser.parse_word(word);

        match result {
            Ok(_) => {
                // Valid word
            }
            Err(parse_errors) => {
                invalid_count += 1;
                eprintln!("INVALID WORD: {:?}", word);
                for err in &parse_errors.errors {
                    eprintln!("  {}", err.message);
                }
            }
        }
    }

    if invalid_count > 0 {
        return Err(TestError::Failure(format!(
            "{} out of {} minimal golden words are invalid!\n\
             \n\
             This means the grammar/parser changed and golden_words_minimal.txt is out of sync.\n\
             \n\
             To fix:\n\
             1. Verify the parser changes are intentional\n\
             2. Regenerate minimal golden words:\n\
                cargo run --release -p talkbank-parser-tests --bin audit_golden_words\n\
             3. Review the diff to ensure it makes sense\n\
             4. Commit the updated golden_words_minimal.txt\n\
             \n\
             See stderr for list of invalid words.",
            invalid_count,
            golden_words.len()
        )));
    }

    Ok(())
}
