//! Test that errors always show the full line as context, not just a snippet

use talkbank_model::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation, Span};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Could not find {label} in test source")]
    MissingNeedle { label: &'static str },
    #[error("Missing error context: {label}")]
    MissingContext { label: &'static str },
}

/// Finds offset.
fn find_offset(haystack: &str, needle: &str, label: &'static str) -> Result<usize, TestError> {
    haystack
        .find(needle)
        .ok_or(TestError::MissingNeedle { label })
}

/// Tests error with partial context gets full line.
#[test]
fn test_error_with_partial_context_gets_full_line() -> Result<(), TestError> {
    // This simulates an error that was created with only partial source text
    let full_source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n*CHI:\thello [:::bad] world .\n@End\n";

    // Find the actual byte offset of "[:::bad]" in the source
    let error_start = find_offset(full_source, "[:::bad]", "bad annotation token")?;
    let error_end = error_start + "[:::bad]".len();

    // Create error pointing to "[:::bad]" on line 5
    // But the error was created with only the bracketed content as source_text
    let mut errors = vec![ParseError::new(
        ErrorCode::new("E303"),
        Severity::Error,
        SourceLocation::from_offsets(error_start, error_end),
        ErrorContext::new("[:::bad]", Span::from_usize(0, 8), "[:::bad]"),
        "Parse error in annotation".to_string(),
    )];

    // Before enhancement
    let before_context = errors[0]
        .context
        .as_ref()
        .ok_or(TestError::MissingContext {
            label: "before enhancement",
        })?;
    assert_eq!(before_context.source_text, "[:::bad]");
    assert_eq!(before_context.span.start, 0);
    assert_eq!(before_context.span.end, 8);

    // Enhance
    talkbank_model::enhance_errors_with_source(&mut errors, full_source);

    // After enhancement - should show ENTIRE line, not just the error token
    let after_context = errors[0]
        .context
        .as_ref()
        .ok_or(TestError::MissingContext {
            label: "after enhancement",
        })?;
    // enhance_errors_with_source converts tabs to spaces for display
    assert_eq!(
        after_context.source_text, "*CHI:   hello [:::bad] world .",
        "Should show the entire line, not just the error token"
    );

    // Span should be relative to the full (display-formatted) line
    let full_line = "*CHI:   hello [:::bad] world .";
    let error_start_in_line = find_offset(full_line, "[:::bad]", "bad annotation token")?;
    assert_eq!(
        after_context.span.start as usize, error_start_in_line,
        "Span start should point to error within full line"
    );

    // Line offset should be set
    assert_eq!(after_context.line_offset, Some(5));

    Ok(())
}

/// Tests validation error gets full line context.
#[test]
fn test_validation_error_gets_full_line_context() -> Result<(), TestError> {
    // Validation errors often have reconstructed text as context, not the original line
    let full_source = "@UTF8\n@Begin\n@Participants:\tCHI Child\n@End\n";

    // Find the actual byte offset of "CHI Child"
    let error_start = find_offset(full_source, "CHI Child", "CHI participant")?;
    let error_end = error_start + "CHI Child".len();

    // Create validation error with reconstructed participant as context
    let mut errors = vec![ParseError::new(
        ErrorCode::new("E522"),
        Severity::Error,
        SourceLocation::from_offsets(error_start, error_end),
        ErrorContext::new("CHI Child", Span::from_usize(0, 9), "CHI Child"),
        "Missing @ID header for CHI".to_string(),
    )];

    // Before enhancement
    let before_context = errors[0]
        .context
        .as_ref()
        .ok_or(TestError::MissingContext {
            label: "before enhancement",
        })?;
    assert_eq!(before_context.source_text, "CHI Child");

    // Enhance
    talkbank_model::enhance_errors_with_source(&mut errors, full_source);

    // After enhancement - should show full @Participants line
    let after_context = errors[0]
        .context
        .as_ref()
        .ok_or(TestError::MissingContext {
            label: "after enhancement",
        })?;
    // enhance_errors_with_source converts tabs to spaces for display
    assert_eq!(
        after_context.source_text, "@Participants:  CHI Child",
        "Should show the entire @Participants line"
    );

    // Line offset should be set
    assert_eq!(after_context.line_offset, Some(3));

    Ok(())
}

/// Tests empty context gets full line.
#[test]
fn test_empty_context_gets_full_line() -> Result<(), TestError> {
    let full_source = "line 1\nerror here on line 2\nline 3\n";

    let mut errors = vec![ParseError::new(
        ErrorCode::new("E999"),
        Severity::Error,
        SourceLocation::from_offsets(7, 12), // "error" word
        ErrorContext::new("", Span::from_usize(0, 0), ""),
        "Test error".to_string(),
    )];

    // Enhance
    talkbank_model::enhance_errors_with_source(&mut errors, full_source);

    // Should show full line
    let after_context = errors[0]
        .context
        .as_ref()
        .ok_or(TestError::MissingContext {
            label: "after enhancement",
        })?;
    assert_eq!(after_context.source_text, "error here on line 2");
    assert_eq!(after_context.line_offset, Some(2));

    Ok(())
}

/// Tests multiline chat utterance shows main tier line.
#[test]
fn test_multiline_chat_utterance_shows_main_tier_line() -> Result<(), TestError> {
    // In CHAT format, errors in main tier should show the main tier line only
    let full_source = "@UTF8\n@Begin\n*CHI:\thello [:::bad] world .\n\t\tcontinuation .\n@End\n";

    // Find the actual byte offset of "[:::bad]"
    let error_start = find_offset(full_source, "[:::bad]", "bad annotation token")?;
    let error_end = error_start + "[:::bad]".len();

    // Error in main tier (line 3)
    let mut errors = vec![ParseError::new(
        ErrorCode::new("E303"),
        Severity::Error,
        SourceLocation::from_offsets(error_start, error_end),
        ErrorContext::new("[:::bad]", Span::from_usize(0, 8), "[:::bad]"),
        "Parse error".to_string(),
    )];

    // Enhance
    talkbank_model::enhance_errors_with_source(&mut errors, full_source);

    // Should show just the main tier line (line 3), not continuation
    let context = errors[0]
        .context
        .as_ref()
        .expect("error should have context");
    // enhance_errors_with_source converts tabs to spaces for display
    assert_eq!(
        context.source_text, "*CHI:   hello [:::bad] world .",
        "Should show only the main tier line where error occurred"
    );
    assert_eq!(context.line_offset, Some(3));

    Ok(())
}
