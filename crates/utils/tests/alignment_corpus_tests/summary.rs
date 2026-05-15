//! Test module for summary in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::PathBuf;

use talkbank_parser::TreeSitterParser;

use super::helpers::{TestError, is_alignment_error, validate_chat_file_with_alignment};

/// Tests alignment corpus summary.
#[test]
fn test_alignment_corpus_summary() -> Result<(), TestError> {
    let happy_path_dir = PathBuf::from("tests/alignment_corpus/happy_path");
    let sad_path_dir = PathBuf::from("tests/alignment_corpus/sad_path");

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;

    let happy_files: Vec<_> = fs::read_dir(&happy_path_dir)
        .map_err(|source| TestError::ReadDir {
            path: happy_path_dir.display().to_string(),
            source,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("cha"))
        .collect();

    eprintln!("\n=== Happy Path Tests ===");
    for entry in &happy_files {
        let path = entry.path();
        let filename = path
            .file_name()
            .ok_or(TestError::MissingFileName {
                path: path.display().to_string(),
            })?
            .to_string_lossy();
        let content = fs::read_to_string(&path).map_err(|source| TestError::ReadError {
            path: path.display().to_string(),
            source,
        })?;

        let mut chat_file =
            parser
                .parse_chat_file(&content)
                .map_err(|errors| TestError::ParseErrors {
                    parser: "tree-sitter",
                    errors,
                })?;
        let errors = validate_chat_file_with_alignment(&mut chat_file);

        let alignment_errors: Vec<_> = errors
            .iter()
            .filter(|e| is_alignment_error(e.code))
            .collect();

        if alignment_errors.is_empty() {
            eprintln!("  ✓ {}", filename);
        } else {
            eprintln!(
                "  ✗ {} - Unexpected alignment errors: {}",
                filename,
                alignment_errors.len()
            );
        }
    }

    let sad_files: Vec<_> = fs::read_dir(&sad_path_dir)
        .map_err(|source| TestError::ReadDir {
            path: sad_path_dir.display().to_string(),
            source,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("cha"))
        .collect();

    eprintln!("\n=== Sad Path Tests ===");
    for entry in &sad_files {
        let path = entry.path();
        let filename = path
            .file_name()
            .ok_or(TestError::MissingFileName {
                path: path.display().to_string(),
            })?
            .to_string_lossy();
        let content = fs::read_to_string(&path).map_err(|source| TestError::ReadError {
            path: path.display().to_string(),
            source,
        })?;

        let mut chat_file =
            parser
                .parse_chat_file(&content)
                .map_err(|errors| TestError::ParseErrors {
                    parser: "tree-sitter",
                    errors,
                })?;
        let errors = validate_chat_file_with_alignment(&mut chat_file);

        let alignment_errors: Vec<_> = errors
            .iter()
            .filter(|e| is_alignment_error(e.code))
            .collect();

        if !alignment_errors.is_empty() {
            eprintln!(
                "  ✓ {} - Detected {} error(s)",
                filename,
                alignment_errors.len()
            );
        } else {
            eprintln!("  ✗ {} - Expected errors but found none", filename);
        }
    }

    eprintln!("\n=== Summary ===");
    eprintln!("Happy path files: {}", happy_files.len());
    eprintln!("Sad path files: {}", sad_files.len());
    eprintln!(
        "Total alignment test files: {}",
        happy_files.len() + sad_files.len()
    );

    Ok(())
}
