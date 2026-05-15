//! Cross-utterance balance checks for scoped begin/end markers.
//!
//! This module validates:
//! - LongFeatureBegin/End markers (&{l=LABEL / &}l=LABEL)
//! - NonvocalBegin/End markers (&{n=LABEL / &}n=LABEL)
//!
//! Both types can span multiple utterances and must have matching labels.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::model::{Utterance, UtteranceContent};
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

/// Validate that long feature markers are properly matched across all utterances.
///
/// Checks:
/// - E358: Every LongFeatureBegin has a matching LongFeatureEnd
/// - E359: Every LongFeatureEnd has a matching LongFeatureBegin
/// - E366: Labels match between paired begin/end markers
///
/// Matching is label-specific and uses LIFO behavior per label, so nested scopes
/// with distinct labels are handled independently.
pub fn check_long_feature_balance(utterances: &[Utterance], errors: &impl ErrorSink) {
    // Track open scopes by label, storing the span of each begin marker
    let mut open_scopes: Vec<(&str, Span)> = Vec::new();

    for utterance in utterances {
        for content in &utterance.main.content.content {
            match content {
                UtteranceContent::LongFeatureBegin(begin) => {
                    open_scopes.push((begin.label.as_str(), begin.span));
                }
                UtteranceContent::LongFeatureEnd(end) => {
                    let label = end.label.as_str();
                    // Find matching begin (last-in-first-out for same label)
                    if let Some(pos) = open_scopes.iter().rposition(|(l, _)| *l == label) {
                        open_scopes.remove(pos);
                    } else {
                        // End without matching begin
                        errors.report(
                            ParseError::at_span(
                                ErrorCode::UnmatchedLongFeatureEnd,
                                Severity::Error,
                                end.span,
                                format!("Unmatched long feature end marker for label '{}'", label),
                            )
                            .with_suggestion(format!(
                                "Add a matching &{{l={} marker before this &}}l={} marker",
                                label, label
                            )),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    // Check for unclosed scopes — report each one at its begin marker's span
    for (label, span) in open_scopes {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedLongFeatureBegin,
                Severity::Error,
                span,
                format!(
                    "Unmatched long feature begin marker: &{{l={} without matching &}}l={}",
                    label, label
                ),
            )
            .with_suggestion(format!("Add a matching &}}l={} marker", label)),
        );
    }
}

/// Validate that nonvocal markers are properly matched across all utterances.
///
/// Checks:
/// - E367: Every NonvocalBegin has a matching NonvocalEnd
/// - E368: Every NonvocalEnd has a matching NonvocalBegin
/// - E369: Labels match between paired begin/end markers
///
/// The algorithm mirrors long-feature balancing so both scoped-marker families
/// share consistent cross-utterance semantics and diagnostics.
pub fn check_nonvocal_balance(utterances: &[Utterance], errors: &impl ErrorSink) {
    // Track open scopes by label, storing the span of each begin marker
    let mut open_scopes: Vec<(&str, Span)> = Vec::new();

    for utterance in utterances {
        for content in &utterance.main.content.content {
            match content {
                UtteranceContent::NonvocalBegin(begin) => {
                    open_scopes.push((begin.label.as_str(), begin.span));
                }
                UtteranceContent::NonvocalEnd(end) => {
                    let label = end.label.as_str();
                    // Find matching begin (last-in-first-out for same label)
                    if let Some(pos) = open_scopes.iter().rposition(|(l, _)| *l == label) {
                        open_scopes.remove(pos);
                    } else {
                        // End without matching begin
                        errors.report(
                            ParseError::at_span(
                                ErrorCode::UnmatchedNonvocalEnd,
                                Severity::Error,
                                end.span,
                                format!("Unmatched nonvocal end marker for label '{}'", label),
                            )
                            .with_suggestion(format!(
                                "Add a matching &{{n={} marker before this &}}n={} marker",
                                label, label
                            )),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    // Check for unclosed scopes — report each one at its begin marker's span
    for (label, span) in open_scopes {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedNonvocalBegin,
                Severity::Error,
                span,
                format!(
                    "Unmatched nonvocal begin marker: &{{n={} without matching &}}n={}",
                    label, label
                ),
            )
            .with_suggestion(format!("Add a matching &}}n={} marker", label)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::{
        LongFeatureBegin, LongFeatureEnd, MainTier, NonvocalBegin, NonvocalEnd, Terminator,
    };

    /// Unmatched long-feature begin markers emit `E358`.
    ///
    /// The diagnostic should include the unmatched label for easier repair.
    #[test]
    fn test_e358_unmatched_long_feature_begin() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedLongFeatureBegin);
        assert!(errors[0].message.contains("singing"));
    }

    /// Unmatched long-feature end markers emit `E359`.
    ///
    /// This catches stray closing markers with no prior opening scope.
    #[test]
    fn test_e359_unmatched_long_feature_end() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedLongFeatureEnd);
        assert!(errors[0].message.contains("singing"));
    }

    /// Mismatched long-feature labels produce begin/end unmatched diagnostics.
    ///
    /// The current behavior surfaces both sides as unmatched when labels differ.
    #[test]
    fn test_e366_long_feature_label_mismatch() {
        let main1 = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );
        let main2 = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(
                "whisper",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main1), Utterance::new(main2)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 2); // Unmatched begin (singing) and unmatched end (whisper)
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::UnmatchedLongFeatureBegin && e.message.contains("singing")
        ));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::UnmatchedLongFeatureEnd
                    && e.message.contains("whisper"))
        );
    }

    /// Balanced long-feature scopes produce no errors.
    ///
    /// This is the baseline valid path for cross-utterance long-feature tracking.
    #[test]
    fn test_balanced_long_features() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::LongFeatureBegin(LongFeatureBegin::new("singing")),
                UtteranceContent::LongFeatureEnd(LongFeatureEnd::new("singing")),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 0);
    }

    /// Unmatched nonvocal begin markers emit `E367`.
    ///
    /// Label text should be preserved in the resulting error message.
    #[test]
    fn test_e367_unmatched_nonvocal_begin() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                "crying",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedNonvocalBegin);
        assert!(errors[0].message.contains("crying"));
    }

    /// Unmatched nonvocal end markers emit `E368`.
    ///
    /// This catches closing markers that do not correspond to an open scope.
    #[test]
    fn test_e368_unmatched_nonvocal_end() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalEnd(NonvocalEnd::new("crying"))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedNonvocalEnd);
        assert!(errors[0].message.contains("crying"));
    }

    /// Nonvocal label mismatches surface unmatched begin/end diagnostics.
    ///
    /// The checker treats differing labels as independent unclosed scopes.
    #[test]
    fn test_e369_nonvocal_label_mismatch() {
        let main1 = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                "crying",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );
        let main2 = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalEnd(NonvocalEnd::new("laughing"))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main1), Utterance::new(main2)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 2); // Unmatched begin (crying) and unmatched end (laughing)
        assert!(
            errors.iter().any(
                |e| e.code == ErrorCode::UnmatchedNonvocalBegin && e.message.contains("crying")
            )
        );
        assert!(
            errors.iter().any(
                |e| e.code == ErrorCode::UnmatchedNonvocalEnd && e.message.contains("laughing")
            )
        );
    }

    /// Balanced nonvocal scopes produce no errors.
    ///
    /// This confirms begin/end matching works for the happy path.
    #[test]
    fn test_balanced_nonvocal() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::NonvocalBegin(NonvocalBegin::new("crying")),
                UtteranceContent::NonvocalEnd(NonvocalEnd::new("crying")),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&utterances, &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 0);
    }
}
