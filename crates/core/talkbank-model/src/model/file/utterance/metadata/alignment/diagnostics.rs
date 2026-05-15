use crate::{ErrorCode, ErrorContext, ErrorLabel, ParseError, Severity, SourceLocation, Span};

/// Build a warning emitted when parse-health taint blocks one alignment pass.
pub(super) fn skipped_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_clean: bool,
    left_span: Span,
    right_label: &str,
    right_clean: bool,
    right_span: Span,
) -> ParseError {
    let tainted = match (!left_clean, !right_clean) {
        (true, true) => format!("{left_label} and {right_label}"),
        (true, false) => left_label.to_string(),
        (false, true) => right_label.to_string(),
        (false, false) => "an internal parse-health gate".to_string(),
    };

    let location = first_non_dummy_span([left_span, right_span]);
    let mut error = ParseError::new(
        ErrorCode::TierValidationError,
        Severity::Warning,
        SourceLocation::new(location),
        ErrorContext::new("", location.to_range(), ""),
        format!(
            "Tier validation warning: skipped {} alignment because {} had parse errors during recovery",
            alignment_name, tainted
        ),
    )
    .with_suggestion("Fix parse errors in the affected tier(s) first, then rerun validation");

    if !left_clean && !left_span.is_dummy() {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if !right_clean && !right_span.is_dummy() {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }

    error
}

/// Build a warning emitted when alignment is blocked by missing parse provenance.
pub(super) fn unknown_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_span: Span,
    right_label: &str,
    right_span: Span,
) -> ParseError {
    let location = first_non_dummy_span([left_span, right_span]);
    let mut error = ParseError::new(
        ErrorCode::TierValidationError,
        Severity::Warning,
        SourceLocation::new(location),
        ErrorContext::new("", location.to_range(), ""),
        format!(
            "Tier validation warning: skipped {} alignment because parse provenance is unknown for {} and {}",
            alignment_name, left_label, right_label
        ),
    )
    .with_suggestion(
        "Run parser-backed validation or explicitly mark parse provenance before alignment checks",
    );

    if !left_span.is_dummy() {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if !right_span.is_dummy() {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }

    error
}

pub(super) fn first_non_dummy_span(spans: [Span; 2]) -> Span {
    for span in spans {
        if !span.is_dummy() {
            return span;
        }
    }
    Span::DUMMY
}

pub(super) fn build_count_mismatch_error(
    source_count: usize,
    source_span: Span,
    source_label: &str,
    target_count: usize,
    _target_span: Span,
    target_label: &str,
    code: ErrorCode,
) -> ParseError {
    ParseError::new(
        code,
        Severity::Error,
        SourceLocation::new(source_span),
        ErrorContext::new("", source_span.to_range(), ""),
        format!(
            "{} has {} words but {} has {}: word counts must match",
            source_label, source_count, target_label, target_count
        ),
    )
    .with_suggestion(format!(
        "Ensure {} and {} have the same number of words",
        source_label, target_label
    ))
}
