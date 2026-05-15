//! Property-based tests for `Span` type invariants.
//!
//! These tests verify that `Span` methods maintain their documented contracts
//! for all possible inputs, not just hand-picked examples. Properties cover
//! construction, containment, merge, and conversion.

use proptest::prelude::*;
use talkbank_model::Span;

/// Strategy for generating a `Span` with a bounded start and length.
///
/// Uses `saturating_add` to avoid overflow when computing `end`.
fn arb_span() -> impl Strategy<Value = Span> {
    (0u32..10_000, 0u32..1_000).prop_map(|(start, len)| Span::new(start, start.saturating_add(len)))
}

/// Strategy for generating a non-empty `Span` (start < end).
fn arb_nonempty_span() -> impl Strategy<Value = Span> {
    (0u32..10_000, 1u32..1_000).prop_map(|(start, len)| Span::new(start, start.saturating_add(len)))
}

proptest! {
    /// `Span::len()` never panics, regardless of start and end values.
    ///
    /// Uses `saturating_sub` internally, so even spans where start > end
    /// (which are structurally odd but not prevented by the constructor)
    /// should return 0 rather than panicking.
    #[test]
    fn span_len_never_panics(start in 0u32..=u32::MAX, end in 0u32..=u32::MAX) {
        // Should not panic — that is the entire assertion.
        let _ = Span::new(start, end).len();
    }

    /// A non-empty span contains its own start offset.
    ///
    /// `contains_offset` uses half-open `[start, end)` semantics, so the
    /// start byte is always included when the span is non-empty.
    #[test]
    fn span_contains_offset_boundary(span in arb_nonempty_span()) {
        prop_assert!(
            span.contains_offset(span.start),
            "Span({}, {}) should contain its start offset {}",
            span.start, span.end, span.start
        );
    }

    /// A span never contains its own end offset (exclusive upper bound).
    ///
    /// This is the half-open interval invariant: `end` is the first byte
    /// that is NOT part of the span.
    #[test]
    fn span_contains_offset_exclusive_end(span in arb_span()) {
        prop_assert!(
            !span.contains_offset(span.end),
            "Span({}, {}) should NOT contain its end offset {}",
            span.start, span.end, span.end
        );
    }

    /// `merge` is commutative: `a.merge(b) == b.merge(a)`.
    ///
    /// The merge operation computes `min(start)..max(end)`, which is
    /// symmetric by definition.
    #[test]
    fn span_merge_commutative(a in arb_span(), b in arb_span()) {
        let ab = a.merge(b);
        let ba = b.merge(a);
        prop_assert_eq!(ab, ba, "merge must be commutative");
    }

    /// The merged span contains both input spans.
    ///
    /// Since `merge` computes the smallest enclosing span, both originals
    /// must be fully contained within the result.
    #[test]
    fn span_merge_contains_both(a in arb_span(), b in arb_span()) {
        let merged = a.merge(b);
        prop_assert!(
            merged.contains_span(a),
            "merged({:?}) must contain a({:?})",
            merged, a
        );
        prop_assert!(
            merged.contains_span(b),
            "merged({:?}) must contain b({:?})",
            merged, b
        );
    }

    /// `Span::at(x)` always produces an empty (zero-width) span.
    ///
    /// A point span has `start == end`, so `is_empty()` must be true.
    #[test]
    fn span_at_is_empty(x in 0u32..=u32::MAX) {
        let span = Span::at(x);
        prop_assert!(
            span.is_empty(),
            "Span::at({}) should be empty, got start={} end={}",
            x, span.start, span.end
        );
    }

    /// `to_range()` produces a `Range<usize>` matching the span's offsets.
    ///
    /// The conversion simply widens `u32` to `usize`.
    #[test]
    fn span_to_range_matches(span in arb_span()) {
        let range = span.to_range();
        let expected = (span.start as usize)..(span.end as usize);
        prop_assert_eq!(range, expected);
    }

    /// Containment is transitive through offsets: if `a` contains span `b`
    /// and `b` contains offset `c`, then `a` must also contain `c`.
    ///
    /// This is a direct consequence of the interval nesting property.
    #[test]
    fn span_contains_transitivity(
        a in arb_nonempty_span(),
        b in arb_nonempty_span(),
        c_offset in 0u32..10_000
    ) {
        if a.contains_span(b) && b.contains_offset(c_offset) {
            prop_assert!(
                a.contains_offset(c_offset),
                "Transitivity violated: a={:?} contains b={:?}, b contains {}, but a does not",
                a, b, c_offset
            );
        }
    }
}
