//! Parse error corpus - tests that syntactically invalid CHAT fails to parse.
//!
//! These files contain **grammar-level errors** that should cause parsing to fail.
//! The parser should return `Err(ParseErrors)` for these files.
//!
//! ## Examples
//! - `hello@` - Missing form type after @
//! - `^test` - Caret at word start (should be mid-word)
//! - `hello [` - Unmatched bracket
//!
//! ## Usage
//! ```bash
//! cargo test parse_error_corpus -- --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Returns a UTF-8 filename for corpus diagnostics.
fn file_name_str(path: &Path) -> Result<&str, TestError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TestError::Failure(format!("Invalid filename for {}", path.display())))
}

/// Finds parse error files.
fn find_parse_error_files() -> Result<Vec<(String, PathBuf)>, TestError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| TestError::Failure("manifest dir missing expected grandparent".to_string()))?
        .join("tests/error_corpus/parse_errors");

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&corpus_root).into_iter() {
        let entry = entry
            .map_err(|err| TestError::Failure(format!("Failed to read corpus entry: {err}")))?;
        let path = entry.path();
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

/// Parses errors fail parsing.
#[test]
fn parse_errors_fail_parsing() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let error_files = find_parse_error_files()?;

    if error_files.is_empty() {
        return Err(TestError::Failure(
            "Parse error corpus is empty!".to_string(),
        ));
    }

    println!(
        "
Testing {} parse error files...\n",
        error_files.len()
    );

    let mut unexpected_success = Vec::new();

    for (error_code, path) in &error_files {
        let content = fs::read_to_string(path)?;

        let result = parser.parse_chat_file(&content);

        match result {
            Err(parse_errors) => {
                println!(
                    "  ✓ {} - {} → Parse failed ({} errors)",
                    error_code,
                    file_name_str(path)?,
                    parse_errors.errors.len()
                );
            }
            Ok(_) => {
                let filename = file_name_str(path)?;
                unexpected_success.push(format!(
                    "{} - {} (Parser accepted invalid syntax!)",
                    error_code, filename
                ));
                println!("  ✗ {} - {} → Unexpected success", error_code, filename);
            }
        }
    }

    if !unexpected_success.is_empty() {
        return Err(TestError::Failure(format!(
            "{} parse error files were accepted by parser:\n  {}\n",
            unexpected_success.len(),
            unexpected_success.join("\n  ")
        )));
    }

    println!(
        "\n✓ All {} files correctly rejected by parser",
        error_files.len()
    );

    Ok(())
}
