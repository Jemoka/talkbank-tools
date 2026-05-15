//! Validators for metadata header formats (`@Date`, `@Time Duration`, `@Time Start`).
//!
//! This module houses header-field format validators that are easier to keep
//! independent from structural header-order logic in `header/structure`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

/// Validate a `DD-MMM-YYYY` date value against CLAN `depfile.cut`'s
/// `@d<dd-lll-yyyy>` template. Used by both `@Date` (E518) and
/// `@Birth of` (E545) — the two headers share the same date format
/// rule per depfile, so the same component-level diagnostic logic
/// applies; only the emitted error code differs.
///
/// The checker reports granular diagnostics per component (day/month/year) so
/// users get actionable fixes instead of a single generic format error.
pub(super) fn check_date_format(
    date: &str,
    span: Span,
    errors: &impl ErrorSink,
    error_code: ErrorCode,
) {
    const VALID_MONTHS: &[&str] = &[
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];

    let make_err = |context_label: &str, message: String| {
        let mut err = ParseError::new(
            error_code,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(date, 0..date.len(), context_label),
            message,
        );
        err.location.span = span;
        err
    };

    let parts: Vec<&str> = date.split('-').collect();

    if parts.len() != 3 {
        errors.report(
            make_err(
                date,
                format!(
                    "Invalid @Date format '{}': expected DD-MMM-YYYY with hyphens",
                    date
                ),
            )
            .with_suggestion(
                "Use format: 01-JAN-2024 (two-digit day, uppercase month, four-digit year)",
            ),
        );
        return;
    }

    let (day_str, month_str, year_str) = (parts[0], parts[1], parts[2]);

    if day_str.len() != 2 {
        errors.report(
            make_err(
                day_str,
                format!(
                    "Invalid @Date day '{}': must be exactly two digits",
                    day_str
                ),
            )
            .with_suggestion("Use two-digit day (e.g., 01, 02, 15)"),
        );
    } else if let Ok(day) = day_str.parse::<u8>() {
        if !(1..=31).contains(&day) {
            errors.report(
                make_err(
                    day_str,
                    format!("Invalid @Date day '{}': must be between 01 and 31", day_str),
                )
                .with_suggestion("Use a valid day between 01 and 31"),
            );
        }
    } else {
        errors.report(
            make_err(
                day_str,
                format!("Invalid @Date day '{}': not a number", day_str),
            )
            .with_suggestion("Day must be a number (01-31)"),
        );
    }

    if !VALID_MONTHS.contains(&month_str) {
        let suggestion = if month_str.len() == 3 {
            let upper = month_str.to_uppercase();
            if VALID_MONTHS.contains(&upper.as_str()) {
                format!("Use uppercase month: {}", upper)
            } else {
                "Valid months: JAN, FEB, MAR, APR, MAY, JUN, JUL, AUG, SEP, OCT, NOV, DEC"
                    .to_string()
            }
        } else {
            "Month must be three-letter uppercase abbreviation (e.g., JAN, FEB, MAR)".to_string()
        };

        errors.report(
            make_err(
                month_str,
                format!(
                    "Invalid @Date month '{}': must be an uppercase three-letter abbreviation",
                    month_str
                ),
            )
            .with_suggestion(suggestion),
        );
    }

    if year_str.len() != 4 {
        errors.report(
            make_err(
                year_str,
                format!(
                    "Invalid @Date year '{}': must be exactly four digits",
                    year_str
                ),
            )
            .with_suggestion("Use four-digit year (e.g., 2024)"),
        );
    } else if year_str.parse::<u16>().is_err() {
        errors.report(
            make_err(
                year_str,
                format!("Invalid @Date year '{}': not a number", year_str),
            )
            .with_suggestion("Year must be a four-digit number"),
        );
    }
}

// ── Time format validators (E540, E541) ───────────────────────────────
//
// The validator functions below are the emission surface; the shape
// decisions live on the typed `TimeDurationValue` /
// `TimeStartValue::violates_depfile_pattern()` methods. The
// dispatcher in `validate.rs` gates these calls on either
// `has_validation_issue()` (Unsupported variant — raw string failed
// to structurally parse) or `violates_depfile_pattern()` (parsed,
// but the raw string doesn't match one of CLAN depfile.cut's legal
// shapes), so by the time we get here we know the value is invalid.
// The function just renders the diagnostic.

/// E540: Emit an error for an invalid `@Time Duration` value.
///
/// The dispatcher has already determined that the raw string fails
/// either the model-layer parse or the depfile-pattern check, so
/// this function unconditionally reports E540 with a depfile-rooted
/// remediation suggestion.
pub(super) fn check_time_duration_format(duration: &str, span: Span, errors: &impl ErrorSink) {
    if duration.is_empty() {
        return;
    }
    let mut err = ParseError::new(
        ErrorCode::InvalidTimeDuration,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new(duration, 0..duration.len(), "time_duration"),
        format!(
            "Invalid @Time Duration format: '{}'. Legal forms per CLAN depfile.cut: HH:MM-HH:MM, HH:MM:SS-HH:MM:SS, or HH:MM:SS",
            duration
        ),
    )
    .with_suggestion(
        "Use one of: HH:MM:SS (single), HH:MM-HH:MM (range), HH:MM:SS-HH:MM:SS (range). No comma-joined segments, no semicolon separator.",
    );
    err.location.span = span;
    errors.report(err);
}

/// E541: Emit an error for an invalid `@Time Start` value.
///
/// Mirrors `check_time_duration_format`: the dispatcher already
/// decided this value is invalid, so this function just renders
/// the E541 diagnostic with depfile-rooted guidance.
pub(super) fn check_time_start_format(start: &str, span: Span, errors: &impl ErrorSink) {
    if start.is_empty() {
        return;
    }
    let mut err = ParseError::new(
        ErrorCode::InvalidTimeStart,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new(start, 0..start.len(), "time_start"),
        format!(
            "Invalid @Time Start format: '{}'. Legal forms per CLAN depfile.cut: HH:MM:SS or MM:SS",
            start
        ),
    )
    .with_suggestion("Use HH:MM:SS or MM:SS — no millisecond suffix, no range.");
    err.location.span = span;
    errors.report(err);
}

#[cfg(test)]
mod time_format_tests {
    // The old unit tests asserted that `check_time_*_format` accepted
    // semicolon separators, millisecond suffixes, etc. Those shapes
    // are no longer legal under depfile.cut conformance (E540/E541
    // landed 2026-04-21); pattern decisions now live on the typed
    // `TimeDurationValue::violates_depfile_pattern` /
    // `TimeStartValue::violates_depfile_pattern` model methods,
    // and end-to-end coverage flows through the spec-driven harness
    // at `resources/spec/errors/E540_invalid_time_duration.md` and
    // `resources/spec/errors/E541_invalid_time_start.md`.
    use super::*;
    use crate::ErrorCollector;

    #[test]
    fn duration_validator_emits_once_for_non_empty_input() {
        let errors = ErrorCollector::new();
        check_time_duration_format("anything", Span::DUMMY, &errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::InvalidTimeDuration);
    }

    #[test]
    fn duration_validator_skips_empty_input() {
        let errors = ErrorCollector::new();
        check_time_duration_format("", Span::DUMMY, &errors);
        assert!(errors.into_vec().is_empty());
    }

    #[test]
    fn start_validator_emits_once_for_non_empty_input() {
        let errors = ErrorCollector::new();
        check_time_start_format("anything", Span::DUMMY, &errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::InvalidTimeStart);
    }

    #[test]
    fn start_validator_skips_empty_input() {
        let errors = ErrorCollector::new();
        check_time_start_format("", Span::DUMMY, &errors);
        assert!(errors.into_vec().is_empty());
    }
}
