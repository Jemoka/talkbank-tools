//! Test module for coverage in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{
    discover_error_files, error_corpus_relative_path, load_expectations_manifest,
};
use talkbank_utils::test_error::TestError;

/// Tests error corpus coverage.
#[test]
fn test_error_corpus_coverage() -> Result<(), TestError> {
    let manifest = load_expectations_manifest()?;
    let files = discover_error_files()?;
    let mut tested_codes: Vec<String> = files
        .iter()
        .flat_map(|path| {
            let relative_path = error_corpus_relative_path(path).ok()?;
            let outcomes = manifest.files.get(&relative_path)?;
            let outcome = outcomes.for_parser("tree-sitter")?;
            if outcome.codes().is_empty() {
                None
            } else {
                Some(outcome.codes().to_vec())
            }
        })
        .flatten()
        .collect();
    tested_codes.sort();
    tested_codes.dedup();

    let e2xx = files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/E2xx"))
        .count();
    let e3xx = files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/E3xx"))
        .count();
    let e5xx = files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/E5xx"))
        .count();
    let e7xx = files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/E7xx"))
        .count();
    let w6xx = files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/W6xx"))
        .count();

    println!("Error corpus coverage:");
    println!("  E2xx (word errors): {}", e2xx);
    println!("  E3xx (main tier errors): {}", e3xx);
    println!("  E5xx (header errors): {}", e5xx);
    println!("  E7xx (tier parsing errors): {}", e7xx);
    println!("  W6xx (warnings): {}", w6xx);
    println!("  Total: {}", files.len());
    println!("\nTested error codes: {:?}", tested_codes);

    assert!(
        e2xx >= 9,
        "Expected at least 9 E2xx error tests, found {}",
        e2xx
    );
    assert!(
        e3xx >= 10,
        "Expected at least 10 E3xx error tests, found {}",
        e3xx
    );
    assert!(
        e5xx >= 13,
        "Expected at least 13 E5xx error tests, found {}",
        e5xx
    );
    assert!(
        e7xx >= 10,
        "Expected at least 10 E7xx error tests, found {}",
        e7xx
    );
    assert!(
        w6xx >= 3,
        "Expected at least 3 W6xx warning tests, found {}",
        w6xx
    );
    Ok(())
}
