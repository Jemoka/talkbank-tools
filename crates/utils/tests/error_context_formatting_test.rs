//! Test that all errors have proper source context for miette-style formatting

use std::fs;
use std::path::PathBuf;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate_streaming;
use tempfile::TempDir;
use thiserror::Error;

/// Enum variants for TestError.
#[derive(Debug, Error)]
enum TestError {
    #[error("Tempdir creation failed")]
    TempDir { source: std::io::Error },
    #[error("IO error on {path}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Test failure: {0}")]
    Failure(String),
}

/// Tests validation errors have source context.
#[test]
fn test_validation_errors_have_source_context() -> Result<(), TestError> {
    // This file has validation errors (missing @ID for CHI)
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
*CHI:	hello world .
@End
"#;

    let options = ParseValidateOptions::default().with_validation();
    let errors = ErrorCollector::new();

    // Parse and validate
    let _result = parse_and_validate_streaming(content, options, &errors);

    let mut error_vec = errors.into_vec();
    // Enhance errors with source context (this is what CLI/TUI do)
    talkbank_model::enhance_errors_with_source(&mut error_vec, content);

    // Should have at least one validation error (E522: missing @ID)
    if error_vec.is_empty() {
        return Err(TestError::Failure(
            "Should have validation errors".to_string(),
        ));
    }

    // ALL errors should have non-empty source_text for miette display
    for error in &error_vec {
        let Some(context) = error.context.as_ref() else {
            return Err(TestError::Failure(format!(
                "Error {} should include context for miette display",
                error.code
            )));
        };

        if context.source_text.is_empty() {
            return Err(TestError::Failure(format!(
                "Error {} should have source context for miette display. Got empty source_text.\n\
                 Error: {}\n\
                 Location: line={:?}, col={:?}",
                error.code, error.message, error.location.line, error.location.column
            )));
        }

        if context.span.end as usize > context.source_text.len() {
            return Err(TestError::Failure(format!(
                "Error {} span should be within source_text. Got span end={}, source_text len={}",
                error.code,
                context.span.end,
                context.source_text.len()
            )));
        }

        if context.line_offset.is_none() {
            return Err(TestError::Failure(format!(
                "Error {} should have line_offset set for correct line numbers",
                error.code
            )));
        }
    }
    Ok(())
}

/// Tests parse errors have source context.
#[test]
fn test_parse_errors_have_source_context() -> Result<(), TestError> {
    // This file has a tree-sitter parse error
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
*CHI:	hello [:::malformed] world .
@End
"#;

    let options = ParseValidateOptions::default();
    let errors = ErrorCollector::new();

    // Parse only (should catch tree-sitter errors)
    let _result = parse_and_validate_streaming(content, options, &errors);

    let mut error_vec = errors.into_vec();
    // Enhance errors with source context (this is what CLI/TUI do)
    talkbank_model::enhance_errors_with_source(&mut error_vec, content);

    // Should have parse errors
    if error_vec.is_empty() {
        return Err(TestError::Failure("Should have parse errors".to_string()));
    }

    // ALL errors should have source context
    for error in &error_vec {
        let Some(context) = error.context.as_ref() else {
            return Err(TestError::Failure(format!(
                "Parse error {} should include context",
                error.code
            )));
        };

        if context.source_text.is_empty() {
            return Err(TestError::Failure(format!(
                "Parse error {} should have source context. Got empty source_text.",
                error.code
            )));
        }

        if context.span.end as usize > context.source_text.len() {
            return Err(TestError::Failure(format!(
                "Parse error {} span should be within source_text",
                error.code
            )));
        }

        if context.line_offset.is_none() {
            return Err(TestError::Failure(format!(
                "Parse error {} should have line_offset set",
                error.code
            )));
        }
    }
    Ok(())
}

/// Tests tui can display all errors.
#[test]
fn test_tui_can_display_all_errors() -> Result<(), TestError> {
    // Integration test: errors from CLI/TUI validation should be displayable
    let temp_dir = TempDir::new().map_err(|source| TestError::TempDir { source })?;
    let test_file = temp_dir.path().join("test.cha");

    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
*CHI:	hello [:::bad] world .
@End
"#;

    fs::write(&test_file, content).map_err(|source| TestError::Io {
        path: test_file.clone(),
        source,
    })?;

    // Validate using the same path as CLI would
    let file_content = fs::read_to_string(&test_file).map_err(|source| TestError::Io {
        path: test_file.clone(),
        source,
    })?;
    let options = ParseValidateOptions::default().with_validation();
    let errors = ErrorCollector::new();

    let _result = parse_and_validate_streaming(&file_content, options, &errors);
    let mut error_vec = errors.into_vec();
    // Enhance errors with source context (this is what CLI/TUI do)
    talkbank_model::enhance_errors_with_source(&mut error_vec, &file_content);

    // Every error should be displayable in TUI (requires source_text)
    for error in &error_vec {
        // This is the condition the TUI checks (validation_tui.rs:447)
        let Some(context) = error.context.as_ref() else {
            return Err(TestError::Failure(format!(
                "TUI cannot display error {} because context is missing.",
                error.code
            )));
        };
        if context.source_text.is_empty() {
            return Err(TestError::Failure(format!(
                "TUI cannot display error {} because source_text is empty.\n\
                 Error: {}\n\
                 This means the TUI will skip displaying the source code context.",
                error.code, error.message
            )));
        }
    }
    Ok(())
}
