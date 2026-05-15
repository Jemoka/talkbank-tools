//! Warning corpus - tests non-fatal warning generation.
//!
//! These files contain CHAT that is **syntactically valid** and **semantically valid**,
//! but may trigger warnings (non-fatal issues) during validation.
//!
//! ## Test Strategy
//! 1. Parse file (should succeed - produces ChatFile model)
//! 2. Run `chat_file.validate()` (should produce expected warning, not error)
//!
//! ## Examples
//! - `*MOT: hello .` - Speaker not in @Participants (warning, not error)
//! - `hello.` - Missing whitespace before punctuation (stylistic warning)
//!
//! ## Usage
//! ```bash
//! cargo test warning_corpus -- --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use talkbank_model::{ErrorCollector, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Returns a UTF-8 filename for corpus diagnostics.
fn file_name_str(path: &Path) -> Result<&str, TestError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TestError::Failure(format!("Invalid filename for {}", path.display())))
}

/// Finds warning files.
fn find_warning_files() -> Result<Vec<(String, PathBuf)>, TestError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| TestError::Failure("manifest dir missing expected grandparent".to_string()))?
        .join("tests/error_corpus/warnings");

    let mut files = Vec::new();
    let not_implemented = corpus_root.join("not_implemented");
    for entry in walkdir::WalkDir::new(&corpus_root).into_iter() {
        let entry = entry
            .map_err(|err| TestError::Failure(format!("Failed to read corpus entry: {err}")))?;
        let path = entry.path();
        // Skip not_implemented subdirectory (warning rules not yet coded)
        if path.starts_with(&not_implemented) {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            let filename = file_name_str(path)?;
            let warning_code = filename
                .split_once('_')
                .map(|(code, _)| code)
                .ok_or_else(|| {
                    TestError::Failure(format!("Missing warning code in filename {}", filename))
                })?
                .to_string();
            files.push((warning_code, path.to_path_buf()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Verifies each warning-corpus file produces its expected warning code.
#[test]
fn warnings_generated() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let warning_files = find_warning_files()?;

    if warning_files.is_empty() {
        return Err(TestError::Failure("Warning corpus is empty!".to_string()));
    }

    println!(
        "
Testing {} warning files...\n",
        warning_files.len()
    );

    let mut parse_failures = Vec::new();
    let mut missing_warnings = Vec::new();

    for (expected_code, path) in &warning_files {
        let content = fs::read_to_string(path)?;

        // Parse (should succeed - these are syntactically valid)
        let chat_file = match parser.parse_chat_file(&content) {
            Ok(file) => file,
            Err(parse_errors) => {
                let filename = file_name_str(path)?;
                parse_failures.push(format!(
                    "{} - {} (Parse failed: should be syntactically valid!)",
                    expected_code, filename
                ));
                println!(
                    "  ✗ {} - {} → Parse failed (unexpected: {:?})",
                    expected_code, filename, parse_errors
                );
                continue;
            }
        };

        // Validate (should produce expected warning)
        let diagnostics = ErrorCollector::new();
        chat_file.validate(&diagnostics, None);

        let warnings: Vec<String> = diagnostics
            .to_vec()
            .iter()
            .filter(|e| e.severity == Severity::Warning)
            .map(|e| e.code.to_string())
            .collect();

        let found = warnings.iter().any(|code| code == expected_code);

        let filename = file_name_str(path)?;
        if found {
            println!(
                "  ✓ {} - {} → {}",
                expected_code,
                filename,
                warnings.join(", ")
            );
        } else {
            missing_warnings.push(format!(
                "{} - {} (Expected {}, got: {:?})",
                expected_code, filename, expected_code, warnings
            ));
            println!(
                "  ✗ {} - {} → {:?} (expected {})",
                expected_code, filename, warnings, expected_code
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

    if !missing_warnings.is_empty() {
        eprintln!(
            "\n❌ {} files did not produce expected warnings:\n  {}",
            missing_warnings.len(),
            missing_warnings.join("\n  ")
        );
        failed = true;
    }

    if failed {
        return Err(TestError::Failure(
            "Warning corpus test failed!".to_string(),
        ));
    }

    println!(
        "\n✓ All {} files produced expected warnings",
        warning_files.len()
    );

    Ok(())
}
