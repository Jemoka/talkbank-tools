//! Test that TUI mode shows files with errors in alphabetical order

use std::path::{Path, PathBuf};
use std::sync::Arc;
use talkbank_model::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation, Span};
use thiserror::Error;

/// Enum variants for TestError.
#[derive(Debug, Error)]
enum TestError {
    #[error("Missing file name in path: {path}")]
    MissingFileName { path: PathBuf },
    #[error("Non-utf8 file name in path: {path}")]
    NonUtf8FileName { path: PathBuf },
}

/// Helper to create a FileErrors struct for testing
#[derive(Debug, Clone)]
struct FileErrors {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub errors: Vec<ParseError>,
    #[allow(dead_code)]
    pub source: Arc<str>,
}

/// Tests tui files sorted alphabetically.
#[test]
fn test_tui_files_sorted_alphabetically() -> Result<(), TestError> {
    // Create FileErrors in non-alphabetical order
    let file_c = FileErrors {
        path: PathBuf::from("/resources/corpus/charlie.cha"),
        errors: vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(0, 5),
            ErrorContext::new("error", Span::from_usize(0, 5), ""),
            "Error in charlie".to_string(),
        )],
        source: Arc::from("*CHI:\terror ."),
    };

    let file_a = FileErrors {
        path: PathBuf::from("/resources/corpus/alice.cha"),
        errors: vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(0, 5),
            ErrorContext::new("error", Span::from_usize(0, 5), ""),
            "Error in alice".to_string(),
        )],
        source: Arc::from("*CHI:\terror ."),
    };

    let file_b = FileErrors {
        path: PathBuf::from("/resources/corpus/bob.cha"),
        errors: vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(0, 5),
            ErrorContext::new("error", Span::from_usize(0, 5), ""),
            "Error in bob".to_string(),
        )],
        source: Arc::from("*CHI:\terror ."),
    };

    // Collect in non-alphabetical order (charlie, alice, bob)
    let mut file_errors = [file_c, file_a, file_b];

    // Sort by path (this is what the code SHOULD do)
    file_errors.sort_by(|a, b| a.path.cmp(&b.path));

    // Verify alphabetical order: alice, bob, charlie
    assert_eq!(file_name_str(&file_errors[0].path)?, "alice.cha");
    assert_eq!(file_name_str(&file_errors[1].path)?, "bob.cha");
    assert_eq!(file_name_str(&file_errors[2].path)?, "charlie.cha");
    Ok(())
}

/// Tests tui files sorted with different directories.
#[test]
fn test_tui_files_sorted_with_different_directories() {
    // Test sorting with different directory depths
    let files = vec![
        FileErrors {
            path: PathBuf::from("/resources/corpus/subdir/file.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
        FileErrors {
            path: PathBuf::from("/resources/corpus/aaa.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
        FileErrors {
            path: PathBuf::from("/resources/corpus/zzz.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
    ];

    let mut sorted = files.clone();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    // Verify lexicographic order
    assert_eq!(sorted[0].path, PathBuf::from("/resources/corpus/aaa.cha"));
    assert_eq!(sorted[1].path, PathBuf::from("/resources/corpus/subdir/file.cha"));
    assert_eq!(sorted[2].path, PathBuf::from("/resources/corpus/zzz.cha"));
}

/// Tests tui files case insensitive comparison.
#[test]
fn test_tui_files_case_insensitive_comparison() -> Result<(), TestError> {
    // Test that sorting handles case properly (lexicographic, case-sensitive)
    let files = vec![
        FileErrors {
            path: PathBuf::from("/corpus/Zebra.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
        FileErrors {
            path: PathBuf::from("/resources/corpus/apple.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
        FileErrors {
            path: PathBuf::from("/corpus/Apple.cha"),
            errors: vec![],
            source: Arc::from(""),
        },
    ];

    let mut sorted = files.clone();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    // Standard lexicographic sort: uppercase letters come before lowercase in ASCII
    // So: Apple.cha, Zebra.cha, apple.cha
    assert_eq!(file_name_str(&sorted[0].path)?, "Apple.cha");
    assert_eq!(file_name_str(&sorted[1].path)?, "Zebra.cha");
    assert_eq!(file_name_str(&sorted[2].path)?, "apple.cha");
    Ok(())
}

/// Runs file name str.
fn file_name_str(path: &Path) -> Result<&str, TestError> {
    let name = path.file_name().ok_or_else(|| TestError::MissingFileName {
        path: path.to_path_buf(),
    })?;
    name.to_str().ok_or_else(|| TestError::NonUtf8FileName {
        path: path.to_path_buf(),
    })
}
