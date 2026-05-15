//! Test module for diagnostics in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::path::Path;

use talkbank_model::model::{ChatFile, SemanticEq};
use talkbank_transform::to_json_pretty_validated;

/// Prints header.
pub fn print_header(test_file: &str, file_path: &Path) {
    println!("\n============================================================");
    println!("Testing: {}", test_file);
    println!("Path: {}", file_path.display());
    println!("============================================================\n");
}

/// Prints original.
pub fn print_original(original_content: &str) {
    println!("=== ORIGINAL FILE ({} bytes) ===", original_content.len());
    println!("{}", original_content);
    println!("=== END ORIGINAL ===\n");
}

/// Parses chat file.
pub fn parse_chat_file(file_path: &Path, original_content: &str) -> ChatFile {
    match crate::test_utils::roundtrip::parse_for_roundtrip(original_content, true) {
        Ok(file) => {
            println!("✓ Parsing and validation succeeded");
            file
        }
        Err(err) => {
            crate::test_utils::diagnostics::print_pipeline_error(
                Some(file_path),
                original_content,
                &err,
            );
            panic!("Parse failed: {}", err);
        }
    }
}

/// Validates json serialization.
pub fn verify_json_serialization(chat_file: &ChatFile) {
    match to_json_pretty_validated(chat_file) {
        Ok(json) => {
            println!(
                "✓ JSON serialization and schema validation succeeded ({} bytes)",
                json.len()
            );
        }
        Err(e) => {
            panic!("✗ JSON serialization/validation FAILED: {}", e);
        }
    }
}

/// Prints serialized.
pub fn print_serialized(serialized: &str) {
    println!("\n=== SERIALIZED OUTPUT ({} bytes) ===", serialized.len());
    println!("{}", serialized);
    println!("=== END SERIALIZED ===\n");
}

/// Parse serialized CHAT content and compare semantically with original
pub fn assert_roundtrip_semantic(
    original: &ChatFile,
    serialized: &str,
    original_source: &str,
    file_path: &Path,
) {
    // Re-parse the serialized CHAT
    let reparsed = match crate::test_utils::roundtrip::parse_for_roundtrip(serialized, true) {
        Ok(file) => file,
        Err(err) => {
            crate::test_utils::diagnostics::print_pipeline_error(Some(file_path), serialized, &err);
            panic!("Failed to re-parse serialized CHAT: {}", err);
        }
    };

    // Semantic comparison - ignores spans and computed metadata
    if original.semantic_eq(&reparsed) {
        println!("✓ ROUNDTRIP SUCCESS!");
        return;
    }

    println!("✗ SEMANTIC ROUNDTRIP MISMATCH\n");

    // Find the first differing line for debugging
    for (i, (orig_line, reparse_line)) in
        original.lines.iter().zip(reparsed.lines.iter()).enumerate()
    {
        if !orig_line.semantic_eq(reparse_line) {
            println!("First difference at line {}:", i);
            println!("  Original:  {:?}", orig_line);
            println!("  Reparsed:  {:?}", reparse_line);
            break;
        }
    }

    if original.lines.len() != reparsed.lines.len() {
        println!(
            "Line count mismatch: original={}, reparsed={}",
            original.lines.len(),
            reparsed.lines.len()
        );
    }

    let diff_report = crate::test_utils::semantic_diff::analyze_semantic_diff(original, &reparsed);
    println!("\n{}", diff_report.render_with_source(original_source));

    panic!("Semantic roundtrip mismatch - see diff above");
}
