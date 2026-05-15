//! Property-based tests for `SpanShift` trait on `Span`.
//!
//! The `SpanShift` trait shifts byte-offset spans after text insertions or
//! deletions. These tests verify identity, roundtrip, and width-preservation
//! properties directly on `Span`, which is the foundational implementor.

use proptest::prelude::*;
use talkbank_model::{Span, SpanShift};

/// Strategy for generating a non-dummy `Span` with bounded start and length.
///
/// Avoids `(0, 0)` because `SpanShift` treats dummy spans as no-ops,
/// which would trivially satisfy most properties.
fn arb_nondummy_span() -> impl Strategy<Value = Span> {
    (1u32..10_000, 1u32..1_000).prop_map(|(start, len)| Span::new(start, start.saturating_add(len)))
}

proptest! {
    /// Shifting by delta=0 is an identity operation.
    ///
    /// No matter what offset we choose, a zero-delta shift must leave the
    /// span unchanged.
    #[test]
    fn shift_zero_is_identity(
        span in arb_nondummy_span(),
        offset in 0u32..20_000
    ) {
        let original = span;
        let mut shifted = span;
        shifted.shift_spans_after(offset, 0);
        prop_assert_eq!(
            shifted, original,
            "Shifting by 0 should be identity: offset={}, span={:?}",
            offset, original
        );
    }

    /// Shifting by +delta moves spans at or after the offset forward.
    ///
    /// When the span starts at or after the shift offset, both `start` and
    /// `end` increase by `delta`.
    #[test]
    fn shift_positive_moves_forward(
        span in arb_nondummy_span(),
        delta in 1i32..500
    ) {
        // Use an offset that is at or before the span start, so the shift applies.
        let offset = span.start.saturating_sub(1);
        let mut shifted = span;
        shifted.shift_spans_after(offset, delta);

        // The span should have moved forward by exactly delta.
        prop_assert_eq!(
            shifted.start,
            (span.start as i64 + delta as i64) as u32,
            "start should increase by delta"
        );
        prop_assert_eq!(
            shifted.end,
            (span.end as i64 + delta as i64) as u32,
            "end should increase by delta"
        );
    }

    /// Shift(+d) then shift(-d) at the same offset is an identity (roundtrip).
    ///
    /// This holds when the positive shift doesn't cause overflow and the
    /// negative shift doesn't cause underflow (clamping to 0).
    #[test]
    fn shift_then_unshift_is_identity(
        span in arb_nondummy_span(),
        delta in 1i32..500
    ) {
        // Use an offset at or before span start so the shift applies.
        let offset = span.start.saturating_sub(1);
        let original = span;

        let mut shifted = span;
        shifted.shift_spans_after(offset, delta);
        shifted.shift_spans_after(offset, -delta);

        prop_assert_eq!(
            shifted, original,
            "shift(+{}) then shift(-{}) should roundtrip: offset={}, original={:?}, got={:?}",
            delta, delta, offset, original, shifted
        );
    }

    /// Shifting preserves span width when the entire span is affected.
    ///
    /// Since both start and end are shifted by the same delta, the
    /// difference `end - start` (the width) remains constant.
    #[test]
    fn shift_preserves_width(
        span in arb_nondummy_span(),
        delta in -500i32..500
    ) {
        let original_width = span.len();
        // Use an offset at or before span start so the full span is shifted.
        let offset = span.start.saturating_sub(1);

        // Guard: skip cases where clamping to 0 would distort the width.
        let new_start = (span.start as i64) + (delta as i64);
        let new_end = (span.end as i64) + (delta as i64);
        if new_start < 0 || new_end < 0 {
            // Clamping invalidates the width-preservation guarantee.
            return Ok(());
        }

        let mut shifted = span;
        shifted.shift_spans_after(offset, delta);

        prop_assert_eq!(
            shifted.len(),
            original_width,
            "Width should be preserved: offset={}, delta={}, original={:?}, shifted={:?}",
            offset, delta, span, shifted
        );
    }
}
