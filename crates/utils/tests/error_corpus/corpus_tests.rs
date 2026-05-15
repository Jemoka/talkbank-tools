//! Test module for corpus tests in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{
    discover_error_files, error_corpus_relative_path, load_expectations_manifest, parser_suite,
};
use talkbank_utils::test_error::TestError;

/// Tests error corpus.
#[test]
fn test_error_corpus() -> Result<(), TestError> {
    let manifest = load_expectations_manifest()?;
    let files = discover_error_files()?;
    if files.is_empty() {
        return Err(TestError::Failure(
            "No error corpus files found".to_string(),
        ));
    }

    let mut failures = Vec::new();
    // Test BOTH parsers
    for parser in parser_suite()? {
        println!("\n========== Testing with {} ==========", parser.name());

        for file_path in &files {
            let content = std::fs::read_to_string(file_path)?;
            let relative_path = error_corpus_relative_path(file_path)?;

            let Some(expected_outcomes) = manifest.files.get(&relative_path) else {
                // Skip files not yet in the expectations manifest.
                // Spec-generated tests in crates/talkbank-parser-tests are the
                // canonical coverage mechanism; this legacy corpus is supplementary.
                continue;
            };

            let expected_outcome =
                expected_outcomes.for_parser(parser.name()).ok_or_else(|| {
                    TestError::Failure(format!(
                        "{} missing expected outcome for {} parser",
                        relative_path,
                        parser.name()
                    ))
                })?;

            println!(
                "[{}] Testing {}: expecting {}",
                parser.name(),
                file_path.display(),
                expected_outcome.description()
            );

            let all_errors = parser.collect_all_errors(&content);

            let error_codes: Vec<String> = all_errors.iter().map(|e| e.code.to_string()).collect();

            let passed = expected_outcome.matches_codes(&error_codes);

            if passed {
                println!("  ✓ PASSED");
            } else {
                let detail = expected_outcome.description();
                failures.push(format!(
                    "[{}] FAILED {}: {}. Found: {:?}",
                    parser.name(),
                    relative_path,
                    detail,
                    error_codes
                ));
            }
        }

        println!("\n[{}] Test completed", parser.name());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(failures.join("\n")))
    }
}
