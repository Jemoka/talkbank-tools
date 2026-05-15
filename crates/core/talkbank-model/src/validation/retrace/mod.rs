//! Retrace validation for main-tier content streams.
//!
//! This pass enforces a structural invariant: retrace markers (`[/]`, `[//]`,
//! `[///]`) must be followed by substantive content (or a terminator) in leaf
//! traversal order.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

mod collection;
mod detection;
mod rendering;
mod types;

use crate::model::MainTier;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use collection::collect_retrace_checks;
use detection::contains_retrace_marker;
use rendering::render_with_spans;
use types::LeafKind;

/// Validate that retrace markers are followed by real content.
///
/// Retrace markers (`[/]`, `[//]`, `[///]`) must be followed by real content in
/// in-order leaf traversal. Annotations are ignored for traversal. A terminator
/// after the retrace is allowed.
///
/// The implementation short-circuits when no retrace marker exists, then runs:
/// retrace collection, suffix-acceptability computation, and span mapping for
/// precise diagnostics.
///
/// Example violations:
/// - `<the> [/] , .` - ERROR: no content after the retrace
///
/// Valid:
/// - `<the> [/] .` - OK: terminator ends the utterance
/// - `<I want> [/] I need cookie .` - OK: next leaf is "I"
pub(crate) fn check_retraces_have_content(main_tier: &MainTier, errors: &impl ErrorSink) {
    if !contains_retrace_marker(main_tier) {
        return;
    }

    let (leaf_kinds, retrace_checks) = collect_retrace_checks(main_tier);
    let suffix_has_ok = build_suffix_ok(&leaf_kinds);
    let violations: Vec<_> = retrace_checks
        .iter()
        .filter(|check| !has_ok_after(&suffix_has_ok, check.after_leaf_index))
        .collect();

    if violations.is_empty() {
        return;
    }

    let rendered = render_with_spans(main_tier);

    for check in violations {
        let retrace_span = match rendered.retrace_spans.get(check.retrace_index).copied() {
            Some(span) => span,
            None => Span::from_usize(0, 0),
        };
        let absolute_span = if !main_tier.span.is_dummy() {
            let start = main_tier.span.start.saturating_add(retrace_span.start);
            let end = main_tier.span.start.saturating_add(retrace_span.end);
            Span::new(start, end)
        } else {
            retrace_span
        };

        let mut error = ParseError::new(
            ErrorCode::StructuralOrderError,
            Severity::Error,
            SourceLocation::new(absolute_span),
            ErrorContext::new("", absolute_span, ""),
            "Retrace marker ([/], [//], or [///]) must be followed by content or a terminator",
        )
        .with_suggestion(
            "Add content after the retrace marker, or remove the retrace if it's not needed",
        );
        if !absolute_span.is_dummy() {
            error
                .labels
                .push(crate::ErrorLabel::new(absolute_span, "Retrace marker"));
        }
        error.help_url = None;
        errors.report(error);
    }
}

/// Build a suffix-acceptability table over collected leaf kinds.
///
/// Each slot answers whether there is any acceptable token (`RealContent` or
/// `Terminator`) at or after that position.
fn build_suffix_ok(leaf_kinds: &[LeafKind]) -> Vec<bool> {
    let mut suffix = Vec::with_capacity(leaf_kinds.len());
    let mut has_ok = false;
    for kind in leaf_kinds.iter().rev() {
        if matches!(kind, LeafKind::RealContent | LeafKind::Terminator) {
            has_ok = true;
        }
        suffix.push(has_ok);
    }
    suffix.reverse();
    suffix
}

/// Return whether acceptable content exists after a given leaf index.
///
/// The `after_index` value is taken from retrace checkpoints captured during
/// collection traversal.
fn has_ok_after(suffix_ok: &[bool], after_index: usize) -> bool {
    matches!(suffix_ok.get(after_index), Some(true))
}
