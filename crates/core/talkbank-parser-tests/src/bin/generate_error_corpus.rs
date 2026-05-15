//! Error Corpus Generator
//!
//! Programmatically generates test files for all error codes to ensure 100% coverage.
//! Uses ChatFileBuilder to create valid CHAT files with specific errors for validation testing.
//!
//! ## Usage
//! ```bash
//! cargo run -p talkbank-parser-tests --bin generate_error_corpus
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;

use talkbank_parser_tests::error_corpus_gen::{
    generate_e0_e1xx_internal_errors, generate_e2xx_word_errors, generate_e3xx_parser_errors,
    generate_e4xx_dependent_tier_errors, generate_e5xx_header_errors, generate_e6xx_tier_errors,
    generate_e7xx_alignment_errors, generate_wxxx_warnings,
};

/// Entry point for this binary target.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let error_corpus_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or("manifest dir missing great-grandparent")?
        .join("tests/error_corpus");

    println!("Generating error corpus files...\n");

    let mut generated_count = 0;

    // E2xx: Word Errors (validation errors - parse succeeds, validation catches)
    generated_count += generate_e2xx_word_errors(&error_corpus_root)?;

    // E3xx: Parser Errors (parse errors - tree-sitter rejects)
    generated_count += generate_e3xx_parser_errors(&error_corpus_root)?;

    // E4xx: Dependent Tier Errors
    generated_count += generate_e4xx_dependent_tier_errors(&error_corpus_root)?;

    // E5xx: Header Errors
    generated_count += generate_e5xx_header_errors(&error_corpus_root)?;

    // E6xx: Tier Validation Errors
    generated_count += generate_e6xx_tier_errors(&error_corpus_root)?;

    // E7xx: Alignment/Temporal Errors
    generated_count += generate_e7xx_alignment_errors(&error_corpus_root)?;

    // Wxxx: Warnings
    generated_count += generate_wxxx_warnings(&error_corpus_root)?;

    // E0-E1xx: Internal/Structural Errors
    generated_count += generate_e0_e1xx_internal_errors(&error_corpus_root)?;

    println!("\n✓ Generated {} error corpus files", generated_count);
    println!("  Parse errors: tests/error_corpus/parse_errors/");
    println!("  Validation errors: tests/error_corpus/validation_errors/");
    println!("  Warnings: tests/error_corpus/warnings/");

    Ok(())
}
