//! Validation error corpus - tests semantic validation rules.
//!
//! These files are **syntactically valid CHAT** that parse successfully,
//! but violate semantic rules detected by validation.
//!
//! ## Test Strategy
//! 1. Parse file (should succeed - produces ChatFile model)
//! 2. Run `chat_file.validate()` (should produce expected validation error)
//!
//! ## Examples
//! - `&+fri [: friend]` - Replacement on phonological fragment
//! - `hello [: xxx]` - Untranscribed in replacement text
//! - `hello [& text]` - Unknown scoped annotation marker
//!
//! ## Usage
//! ```bash
//! cargo test validation_error_corpus -- --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Returns a UTF-8 filename for corpus diagnostics.
fn file_name_str(path: &Path) -> Result<&str, TestError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TestError::Failure(format!("Invalid filename for {}", path.display())))
}

/// Finds validation error files.
fn find_validation_error_files() -> Result<Vec<(String, PathBuf)>, TestError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| TestError::Failure("manifest dir missing expected grandparent".to_string()))?
        .join("tests/error_corpus/validation_errors");

    let mut files = Vec::new();
    let not_implemented = corpus_root.join("not_implemented");
    for entry in walkdir::WalkDir::new(&corpus_root).into_iter() {
        let entry = entry
            .map_err(|err| TestError::Failure(format!("Failed to read corpus entry: {err}")))?;
        let path = entry.path();
        // Skip not_implemented subdirectory (validation rules not yet coded)
        if path.starts_with(&not_implemented) {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            let filename = file_name_str(path)?;
            let error_code = filename
                .split_once('_')
                .map(|(code, _)| code)
                .ok_or_else(|| {
                    TestError::Failure(format!("Missing error code in filename {}", filename))
                })?
                .to_string();
            files.push((error_code, path.to_path_buf()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Verifies each validation-error corpus file produces its expected diagnostic code.
#[test]
fn validation_errors_detected() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let error_files = find_validation_error_files()?;

    if error_files.is_empty() {
        return Err(TestError::Failure(
            "Validation error corpus is empty!".to_string(),
        ));
    }

    println!(
        "
Testing {} validation error files...\n",
        error_files.len()
    );

    let mut parse_failures = Vec::new();
    let mut validation_failures = Vec::new();

    // Skip validation checks that are not yet implemented.
    // These error codes have specs but the validator doesn't detect them yet.
    const NOT_IMPLEMENTED: &[&str] = &["E230", "E252", "E404"];

    for (expected_code, path) in &error_files {
        if NOT_IMPLEMENTED.contains(&expected_code.as_str()) {
            let filename = file_name_str(path)?;
            println!(
                "  ⊘ {} - {} → Skipped (validation not implemented)",
                expected_code, filename
            );
            continue;
        }
        let content = fs::read_to_string(path)?;

        // Parse with streaming diagnostics so recovered parser errors are visible.
        let parse_errors = ErrorCollector::new();
        let parse_result = parser.parse_chat_file_fragment(&content, 0, &parse_errors);
        let parse_codes: Vec<String> = parse_errors
            .to_vec()
            .iter()
            .map(|e| e.code.to_string())
            .collect();

        let mut chat_file = match parse_result {
            ParseOutcome::Parsed(file) => file,
            ParseOutcome::Rejected => {
                let filename = file_name_str(path)?;
                if parse_codes.iter().any(|code| code == expected_code) {
                    println!(
                        "  ✓ {} - {} → Parse-level diagnostic {:?}",
                        expected_code, filename, parse_codes
                    );
                    continue;
                }
                parse_failures.push(format!(
                    "{} - {} (Parse failed: parser returned None, parse codes: {:?})",
                    expected_code, filename, parse_codes
                ));
                println!(
                    "  ✗ {} - {} → Parse failed (codes: {:?})",
                    expected_code, filename, parse_codes
                );
                continue;
            }
        };

        // Validate (should produce expected error)
        let validation_errors = ErrorCollector::new();
        let file_stem = path.file_stem().and_then(|stem| stem.to_str());
        chat_file.validate_with_alignment(&validation_errors, file_stem);

        let mut error_codes: Vec<String> = parse_codes;
        error_codes.extend(
            validation_errors
                .to_vec()
                .iter()
                .map(|e| e.code.to_string())
                .collect::<Vec<_>>(),
        );

        let found = error_codes.iter().any(|code| code == expected_code);

        let filename = file_name_str(path)?;
        if found {
            println!(
                "  ✓ {} - {} → {}",
                expected_code,
                filename,
                error_codes.join(", ")
            );
        } else {
            validation_failures.push(format!(
                "{} - {} (Expected {}, got: {:?})",
                expected_code, filename, expected_code, error_codes
            ));
            println!(
                "  ✗ {} - {} → {:?} (expected {})",
                expected_code, filename, error_codes, expected_code
            );
        }
    }

    let mut failed = false;

    if !parse_failures.is_empty() {
        eprintln!(
            "\n❌ {} files failed parsing (should be syntactically valid):\n  {}",
            parse_failures.len(),
            parse_failures.join("\n  ")
        );
        failed = true;
    }

    if !validation_failures.is_empty() {
        eprintln!(
            "\n❌ {} files did not produce expected validation errors:\n  {}",
            validation_failures.len(),
            validation_failures.join("\n  ")
        );
        failed = true;
    }

    if failed {
        return Err(TestError::Failure(
            "Validation error corpus test failed!".to_string(),
        ));
    }

    println!(
        "\n✓ All {} files produced expected validation errors",
        error_files.len()
    );

    Ok(())
}
