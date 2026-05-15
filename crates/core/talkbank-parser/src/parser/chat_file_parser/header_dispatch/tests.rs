//! Snapshot tests for single-header parsing entrypoints.

use crate::parser::TreeSitterParser;

/// Parses one header line via `TreeSitterParser::parse_header`.
fn parse_header(input: &str) -> crate::error::ParseResult<crate::model::Header> {
    let parser = TreeSitterParser::new().map_err(|err| {
        crate::error::ParseErrors::from(vec![crate::error::ParseError::new(
            crate::error::ErrorCode::InternalError,
            crate::error::Severity::Error,
            crate::error::SourceLocation::at_offset(0),
            crate::error::ErrorContext::new(input, 0..input.len(), input),
            format!("Failed to create parser: {err}"),
        )])
    })?;
    parser.parse_header(input)
}

/// Executes a test body with consistent snapshot output configuration.
fn with_snapshot_settings<F: FnOnce()>(f: F) {
    let mut settings = insta::Settings::new();
    settings.set_snapshot_path("tests/snapshots");
    settings.set_prepend_module_to_snapshot(false);
    let _guard = settings.bind_to_scope();
    f();
}

// ✅ SUCCESS CASE - Simple header without content (@UTF8)
/// Accepts bare `@UTF8`.
#[test]
fn simple_header_utf8() {
    let result = parse_header("@UTF8");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @UTF8");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__simple_header_utf8", result);
    });
}

// ✅ SUCCESS CASE - Simple header (@Begin)
/// Accepts bare `@Begin`.
#[test]
fn simple_header_begin() {
    let result = parse_header("@Begin");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @Begin");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__simple_header_begin", result);
    });
}

// ✅ SUCCESS CASE - Simple header (@End)
/// Accepts bare `@End`.
#[test]
fn simple_header_end() {
    let result = parse_header("@End");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @End");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__simple_header_end", result);
    });
}

// ✅ SUCCESS CASE - Header with content (@Languages:\teng)
/// Accepts `@Languages` with one language code.
#[test]
fn header_with_content_languages() {
    let result = parse_header("@Languages:\teng");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @Languages");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(
            "header_parsing_tests__header_with_content_languages",
            result
        );
    });
}

// ✅ SUCCESS CASE - @Participants header
/// Accepts a `@Participants` line with two entries.
#[test]
fn header_participants() {
    let result = parse_header("@Participants:\tCHI Child, MOT Mother");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @Participants");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_participants", result);
    });
}

// ✅ SUCCESS CASE - @ID header (complex content)
/// Accepts a fully populated `@ID` line.
#[test]
fn header_id_complex() {
    let result = parse_header("@ID:\teng|bates|CHI|1;08.|female|normal||Child|||");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @ID");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_id_complex", result);
    });
}

// ✅ SUCCESS CASE - @Comment header
/// Accepts `@Comment` content.
#[test]
fn header_comment() {
    let result = parse_header("@Comment:\tThis is a test comment");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @Comment");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_comment", result);
    });
}

/// Accepts `@Comment` with a structured media bullet.
#[test]
fn header_comment_with_bullet() {
    use crate::model::{BulletContentSegment, Header};

    let result = parse_header("@Comment:\tThis is timed \u{0015}1234_1567\u{0015}");
    assert!(
        result.is_ok(),
        "Should successfully parse bullet-bearing @Comment"
    );

    let header = result.expect("header should parse");
    match header {
        Header::Comment { content } => {
            assert_eq!(content.segments.len(), 2, "expected text + bullet segments");
            assert!(matches!(
                &content.segments[0],
                BulletContentSegment::Text(text) if text.text == "This is timed "
            ));
            assert!(matches!(
                &content.segments[1],
                BulletContentSegment::Bullet(timing)
                    if timing.start_ms == 1234 && timing.end_ms == 1567
            ));
        }
        other => panic!("expected Header::Comment, got {:?}", other),
    }
}

// ✅ SUCCESS CASE - @Media header (simple filename, not URL)
// Note: CHAT @Media uses simple filenames, not full URLs
/// Accepts a simple filename-based `@Media` declaration.
#[test]
fn header_media() {
    let result = parse_header("@Media:\tmedia-file, video");

    // Phase 2: Header is now a typed enum, verify via snapshot
    if let Err(ref e) = result {
        eprintln!("Parse error: {:?}", e);
    }
    assert!(result.is_ok(), "Should successfully parse @Media");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_media", result);
    });
}

// ❌ ERROR CASE - Header with colon but no tab (grammar requires tab separator)
/// Rejects `@Languages` when `:` is not followed by a tab.
#[test]
fn header_no_tab_after_colon() {
    let result = parse_header("@Languages:eng");

    // Grammar requires tab after colon; missing tab is a parse error
    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_no_tab_after_colon", result);
    });
}

// ✅ SUCCESS CASE - Header with leading/trailing whitespace
/// Tolerates leading/trailing whitespace around a simple header.
#[test]
fn header_with_whitespace() {
    let result = parse_header("  @UTF8  ");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @UTF8");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_with_whitespace", result);
    });
}

// ❌ ERROR CASE - Missing @ symbol
/// Reports an error when header text is missing leading `@`.
#[test]
fn error_missing_at_symbol() {
    let result = parse_header("UTF8");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty(), "Expected error for missing @");
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__error_missing_at_symbol", result);
    });
}

// ❌ ERROR CASE - Empty input
/// Reports an error on empty input.
#[test]
fn error_empty_input() {
    let result = parse_header("");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty(), "Expected error for empty input");
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__error_empty_input", result);
    });
}

// ❌ ERROR CASE - Just @ symbol
/// Snapshots behavior for a lone `@` token.
#[test]
fn error_just_at_symbol() {
    let result = parse_header("@");

    // This may succeed with empty name (lenient parsing) or error
    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__error_just_at_symbol", result);
    });
}

// ✅ SUCCESS CASE - @Date header
/// Accepts a valid `@Date` header.
#[test]
fn header_date() {
    let result = parse_header("@Date:\t29-FEB-1996");

    // Phase 2: Header is now a typed enum, verify via snapshot
    assert!(result.is_ok(), "Should successfully parse @Date");

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("header_parsing_tests__header_date", result);
    });
}
