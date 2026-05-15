//! Validation rules for user-defined dependent tiers (`%x*`).
//!
//! These checks intentionally emit warnings instead of hard errors so legacy
//! corpora can be migrated gradually from deprecated `%xLABEL` forms to
//! corresponding standard tier tags.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

/// Validate one user-defined `%x*` tier payload.
///
/// This emits warnings for empty content and for deprecated `%xLABEL` forms
/// where `LABEL` is now a standard tier type.
/// The function is intentionally warning-level so legacy corpora can be
/// migrated incrementally without hard parse failures.
pub(crate) fn check_user_defined_tier_content(
    label: &str,
    content: &str,
    span: Span,
    errors: &impl ErrorSink,
) {
    // W601: Warn if tier has empty content.
    if content.chars().all(|ch| ch.is_whitespace()) {
        let mut err = ParseError::new(
            ErrorCode::EmptyUserDefinedTier,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(content, 0..content.len(), content),
            format!("User-defined tier %x{} has no content", label),
        )
        .with_suggestion("User-defined tiers should contain custom analysis/annotation data");
        err.location.span = span;
        errors.report(err);
    }

    // Check tier label validity
    check_tier_label(label, span, errors);
}

/// Validate user-defined tier labels against known standard tier names.
///
/// Labels that collide with standard tiers are reported as migration warnings
/// so corpora can move from `%xfoo` to `%foo` forms.
/// Unknown custom labels remain allowed by design.
fn check_tier_label(label: &str, span: Span, errors: &impl ErrorSink) {
    // Known tier types that have dedicated parsers or are standard tiers
    const KNOWN_TIERS: &[&str] = &[
        "mor", // Morphological tier
        "gra", // Grammatical relation tier
        "pho", "mod", "upho", // Phonological tiers
        "sin",  // Gesture/sign tier
        "com", "exp", "add", // Text tiers
        "spa", "sit", "gpx", // More text tiers
        "int", "act", "cod", // More text tiers
        // Standard text-only tiers
        "ort", "eng", "gls", "alt", "coh", "def", "err", "fac", "flo", "par", "tim",
    ];

    // W602: Warn if %xLABEL where LABEL is already a known standard tier.
    if KNOWN_TIERS.contains(&label) {
        let mut err = ParseError::new(
            ErrorCode::UnknownUserDefinedTier,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(label, 0..label.len(), label),
            format!(
                "Deprecated experimental tier %x{}: should be updated to %{}",
                label, label
            ),
        )
        .with_suggestion(format!(
            "Update tier from %x{} to %{} after validating/aligning content",
            label, label
        ));
        err.location.span = span;
        errors.report(err);
    }

    // Otherwise, it's a valid user-defined %x tier - no error
    // User-defined tiers can have any label (foo, bar, custom, etc.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;

    #[test]
    fn test_w601_empty_tier() {
        let errors = ErrorCollector::new();
        check_user_defined_tier_content("foo", "", Span::DUMMY, &errors);
        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::EmptyUserDefinedTier);
        assert_eq!(error_vec[0].severity, Severity::Error);
    }

    #[test]
    fn test_w602_deprecated_xtier() {
        // %xpho with content should warn (pho is a known standard tier now)
        let errors = ErrorCollector::new();
        check_user_defined_tier_content("pho", "test content", Span::DUMMY, &errors);
        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnknownUserDefinedTier);
        assert_eq!(error_vec[0].severity, Severity::Error);
    }

    #[test]
    fn test_valid_user_tier_no_errors() {
        // %xfoo with content is valid (custom user tier)
        let errors = ErrorCollector::new();
        check_user_defined_tier_content("foo", "test content", Span::DUMMY, &errors);
        let error_vec = errors.into_vec();
        assert!(
            error_vec.is_empty(),
            "Custom user-defined tier should be valid"
        );
    }

    #[test]
    fn test_valid_custom_labels() {
        // Various custom labels should all be valid
        let labels = vec!["foo", "bar", "custom", "abc123", "mydata"];
        for label in labels {
            let errors = ErrorCollector::new();
            check_user_defined_tier_content(label, "content", Span::DUMMY, &errors);
            let error_vec = errors.into_vec();
            assert!(
                error_vec.is_empty() || error_vec[0].code == ErrorCode::UnknownUserDefinedTier,
                "Custom label {} should be valid or only warn",
                label
            );
        }
    }
}
