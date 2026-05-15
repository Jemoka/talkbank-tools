//! Tests for this subsystem.
//!

use super::*;
use miette::Diagnostic;

/// Tests error code documentation url.
#[test]
fn test_error_code_documentation_url() {
    let code = ErrorCode::InternalError;
    assert_eq!(code.documentation_url(), "https://talkbank.org/errors/E001");
}

/// Tests error json serialization.
#[test]
fn test_error_json_serialization() -> Result<(), String> {
    let error = ParseError::new(
        ErrorCode::InternalError,
        Severity::Error,
        SourceLocation::from_offsets(0, 4),
        ErrorContext::new("*CHI:\thello", 0..4, "*CHI"),
        "Invalid speaker name",
    )
    .with_suggestion("Use only ASCII characters in speaker names");

    let json = serde_json::to_string_pretty(&error)
        .map_err(|err| format!("Failed to serialize ParseError: {err}"))?;
    assert!(json.contains("E001"));
    assert!(json.contains("Invalid speaker name"));
    assert!(json.contains("suggestion"));
    Ok(())
}

/// Tests parse error at span constructor.
#[test]
fn test_parse_error_at_span_constructor() {
    let span = Span::from_usize(2, 5);
    let error = ParseError::at_span(
        ErrorCode::InternalError,
        Severity::Error,
        span,
        "span-only error",
    );

    assert_eq!(error.location.span, span);
    assert_eq!(error.context, None);
    assert_eq!(error.message, "span-only error");
}

/// Tests parse error from source span constructor.
#[test]
fn test_parse_error_from_source_span_constructor() -> Result<(), String> {
    let span = Span::from_usize(1, 4);
    let error = ParseError::from_source_span(
        ErrorCode::InternalError,
        Severity::Error,
        span,
        "*CHI:\thello",
        "CHI",
        "source-backed error",
    );

    assert_eq!(error.location.span, span);
    assert_eq!(error.message, "source-backed error");
    let context = error
        .context
        .as_ref()
        .ok_or_else(|| "context should be present".to_string())?;
    assert_eq!(context.span, span);
    assert_eq!(context.found, "CHI");
    Ok(())
}

/// Tests parse errors separation.
#[test]
fn test_parse_errors_separation() {
    let mut errors = ParseErrors::new();

    errors.push(ParseError::new(
        ErrorCode::InternalError,
        Severity::Error,
        SourceLocation::from_offsets(0, 4),
        ErrorContext::new("line 1", 0..4, "text"),
        "Error message",
    ));

    errors.push(ParseError::new(
        ErrorCode::SpeakerNotFoundInParticipants,
        Severity::Warning,
        SourceLocation::from_offsets(5, 9),
        ErrorContext::new("line 2", 0..4, "text"),
        "Warning message",
    ));

    let (errs, warns) = errors.errors_and_warnings();
    assert_eq!(errs.len(), 1);
    assert_eq!(warns.len(), 1);
}

// =========================================================================
// ErrorSink Tests
// =========================================================================

/// Builds test error.
fn make_test_error(code: &str) -> ParseError {
    ParseError::new(
        ErrorCode::new(code),
        Severity::Error,
        SourceLocation::from_offsets(0, 4),
        ErrorContext::new("test", 0..4, "test"),
        format!("Test error {}", code),
    )
}

/// Tests parse tracker counting for errors and warnings.
#[test]
fn test_parse_tracker_counts_severities() {
    let tracker = ParseTracker::new();
    assert!(!tracker.has_error());
    assert!(!tracker.has_warning());

    tracker.report(make_test_error("E001"));
    tracker.report(ParseError::new(
        ErrorCode::SpeakerNotFoundInParticipants,
        Severity::Warning,
        SourceLocation::from_offsets(0, 4),
        ErrorContext::new("test", 0..4, "test"),
        "Warning",
    ));

    assert_eq!(tracker.error_count(), 1);
    assert_eq!(tracker.warning_count(), 1);
    assert!(tracker.has_error());
    assert!(tracker.has_warning());
}

/// Tests the canonical in-memory error collector.
#[test]
fn test_error_collector_basic() {
    let sink = ErrorCollector::new();
    assert!(sink.is_empty());
    assert_eq!(sink.len(), 0);

    sink.report(make_test_error("E001"));
    assert!(!sink.is_empty());
    assert_eq!(sink.len(), 1);

    sink.report(make_test_error("E002"));
    assert_eq!(sink.len(), 2);

    let errors = sink.into_vec();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].code.as_str(), "E001");
    assert_eq!(errors[1].code.as_str(), "E002");
}

/// Tests collector severity inspection.
#[test]
fn test_error_collector_has_errors() {
    let sink = ErrorCollector::new();
    assert!(!sink.has_errors());

    // Add a warning
    sink.report(ParseError::new(
        ErrorCode::SpeakerNotFoundInParticipants,
        Severity::Warning,
        SourceLocation::from_offsets(0, 4),
        ErrorContext::new("test", 0..4, "test"),
        "Warning",
    ));
    assert!(!sink.has_errors());

    // Add an error
    sink.report(make_test_error("E001"));
    assert!(sink.has_errors());
}

/// Tests collector batch reporting.
#[test]
fn test_error_collector_report_all() {
    let sink = ErrorCollector::new();
    let errors = vec![
        make_test_error("E001"),
        make_test_error("E002"),
        make_test_error("E003"),
    ];

    sink.report_all(errors);
    assert_eq!(sink.len(), 3);
}

/// Tests collector cloning without consuming stored diagnostics.
#[test]
fn test_error_collector_to_vec() {
    let sink = ErrorCollector::new();
    sink.report(make_test_error("E001"));

    // to_vec doesn't consume the sink
    let errors1 = sink.to_vec();
    assert_eq!(errors1.len(), 1);

    // Can still use the sink
    sink.report(make_test_error("E002"));
    let errors2 = sink.to_vec();
    assert_eq!(errors2.len(), 2);
}

/// Tests collecting a single diagnostic with the canonical collector name.
#[test]
fn test_error_collector_collects_single_error() {
    let sink = ErrorCollector::new();
    sink.report(make_test_error("E001"));

    let errors = sink.into_vec();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E001");
}

/// Tests channel error sink.
#[cfg(feature = "channels")]
#[test]
fn test_channel_error_sink() {
    let (tx, rx) = crossbeam_channel::bounded(10);
    let sink = ChannelErrorSink::new(tx);

    sink.report(make_test_error("E001"));
    sink.report(make_test_error("E002"));
    drop(sink); // Close the sender

    let mut received = Vec::new();
    while let Ok(error) = rx.recv() {
        received.push(error);
    }

    assert_eq!(received.len(), 2);
    assert_eq!(received[0].code.as_str(), "E001");
    assert_eq!(received[1].code.as_str(), "E002");
}

/// Tests channel error sink handles closed receiver.
#[cfg(feature = "channels")]
#[test]
fn test_channel_error_sink_handles_closed_receiver() {
    let (tx, rx) = crossbeam_channel::bounded(10);
    let sink = ChannelErrorSink::new(tx);

    drop(rx); // Close the receiver

    // Should not panic, just silently ignore
    sink.report(make_test_error("E001"));
}

/// Tests null error sink.
#[test]
fn test_null_error_sink() {
    let sink = NullErrorSink;

    // Should not panic, just discard
    sink.report(make_test_error("E001"));
    sink.report(make_test_error("E002"));
    sink.report_all(vec![make_test_error("E003")]);
}

/// Tests error sink reference impl.
#[test]
fn test_error_sink_reference_impl() {
    let sink = ErrorCollector::new();
    let sink_ref: &dyn ErrorSink = &sink;

    sink_ref.report(make_test_error("E001"));
    assert_eq!(sink.len(), 1);
}

/// Tests line offset adjustment in labels.
#[test]
fn test_line_offset_adjustment_in_labels() -> Result<(), String> {
    // Create an error at line 481 (simulating a deep line in a file)
    let context_text = "*PAR:\tno que no no me sa:<le> [>] .";
    let line_number = 481;

    // Span relative to context_text (error is at the '<' character, position 26)
    let span_in_context = Span::from_usize(26, 27);

    // Create location with line/column
    let location = SourceLocation::from_offsets_with_position(
        22542, // byte offset in full file
        22543,
        line_number,
        27, // column
    );

    // Create context with line_offset
    let context =
        ErrorContext::new(context_text, span_in_context, "<").with_line_offset(line_number);

    let error = ParseError::new(
        ErrorCode::new("E302"),
        Severity::Error,
        location,
        context,
        "Test error at line 481",
    );

    // Get the miette labels — spans are relative to source_text with NO adjustment.
    // SourceCodeWithOffset handles line numbering natively in read_span.
    let labels = error
        .labels()
        .ok_or_else(|| "Expected labels".to_string())?;
    let label_vec: Vec<_> = labels.collect();

    assert_eq!(label_vec.len(), 1);
    let primary_label = &label_vec[0];

    // Span stays as-is: (26, 27) — no offset adjustment
    assert_eq!(primary_label.offset(), 26);
    assert_eq!(primary_label.len(), 1);
    Ok(())
}

/// Tests line offset no adjustment for line 1.
#[test]
fn test_line_offset_no_adjustment_for_line_1() -> Result<(), String> {
    // Create an error at line 1 (no adjustment needed)
    let context_text = "*PAR:\thello";
    let span_in_context = Span::from_usize(6, 11);

    let location = SourceLocation::from_offsets_with_position(6, 11, 1, 7);
    let context = ErrorContext::new(context_text, span_in_context, "hello").with_line_offset(1);

    let error = ParseError::new(
        ErrorCode::new("E001"),
        Severity::Error,
        location,
        context,
        "Test error at line 1",
    );

    let labels = error
        .labels()
        .ok_or_else(|| "Expected labels".to_string())?;
    let label_vec: Vec<_> = labels.collect();

    // No adjustment for line 1
    assert_eq!(label_vec.len(), 1);
    let primary_label = &label_vec[0];
    assert_eq!(primary_label.offset(), 6);
    assert_eq!(primary_label.len(), 5);
    Ok(())
}

/// Tests miette output uses line offset.
#[test]
fn test_miette_output_uses_line_offset() -> Result<(), String> {
    let source = "alpha\nbeta\ngamma error\ndelta\n";
    let error_start = source
        .find("error")
        .ok_or_else(|| "Expected 'error' in source".to_string())?;
    let error_end = error_start + "error".len();

    let mut errors = vec![ParseError::new(
        ErrorCode::new("E999"),
        Severity::Error,
        SourceLocation::from_offsets(error_start, error_end),
        ErrorContext::new("", Span::from_usize(0, 0), ""),
        "Test error".to_string(),
    )];

    enhance_errors_with_source(&mut errors, source);

    let output = format!("{:?}", miette::Report::new(errors[0].clone()));
    assert!(
        output.contains("input:3:") || output.contains("line 3"),
        "Miette output should reference line 3, got:\n{}",
        output
    );

    assert!(
        !(output.contains("input:1:") || output.contains("line 1")),
        "Miette output incorrectly references line 1 instead of line 3:\n{}",
        output
    );
    Ok(())
}
