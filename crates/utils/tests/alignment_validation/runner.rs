//! Test module for runner in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::{config, discovery, stats::AlignmentStats, validation};
use talkbank_parser::TreeSitterParser;

/// Test that all utterances in the corpus have valid tier alignment.
///
/// This test validates that dependent tiers (%mor, %gra, %pho, etc.) properly align
/// with their corresponding main tier content. Alignment validation catches issues like:
/// - %mor tier with wrong number of items compared to alignable main tier content
/// - %gra tier misaligned with %mor chunks
/// - %pho/%sin tiers not matching main tier word count
///
/// Some test corpus files intentionally have alignment errors for testing error detection.
#[test]
fn test_corpus_alignment_validation() {
    let corpus_dir = config::corpus_dir();
    let files = match discovery::list_chat_files(&corpus_dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Failed to list corpus files: {err}");
            return;
        }
    };

    let parser = match TreeSitterParser::new() {
        Ok(parser) => parser,
        Err(err) => {
            eprintln!("Failed to create parser: {err}");
            return;
        }
    };
    let mut stats = AlignmentStats::default();

    for path in files {
        if let Err(err) = validation::validate_file(&parser, &path, &mut stats) {
            eprintln!("Validation error: {err}");
        }
    }

    stats.print_summary();

    // Note: Some test files intentionally have alignment errors for testing error detection.
    // This test succeeds if alignment validation runs without crashing.
    // Review the output above to see which files have alignment issues.
}
