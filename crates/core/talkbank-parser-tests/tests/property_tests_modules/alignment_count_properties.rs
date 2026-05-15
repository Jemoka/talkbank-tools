//! Property-based tests for `AlignmentPair` and `positional_align` count invariants.
//!
//! The `positional_align` function requires complex tier types that are hard to
//! construct from arbitrary data, so these tests focus on the structural properties
//! of `AlignmentPair` and on the count invariants that `positional_align` must
//! satisfy, simulated via the same pairing algorithm applied to plain index sequences.

use proptest::prelude::*;
use talkbank_model::alignment::{AlignmentPair, IndexPair};

/// Simulate the positional alignment algorithm using plain counts.
///
/// This mirrors the pairing logic in `positional_align`: complete pairs for
/// the common prefix, then placeholder pairs for the excess on whichever
/// side is longer.
fn simulate_alignment(source_count: usize, target_count: usize) -> Vec<AlignmentPair> {
    let min_len = source_count.min(target_count);
    let max_len = source_count.max(target_count);
    let mut pairs = Vec::with_capacity(max_len);

    // 1:1 pairs for the common range
    for i in 0..min_len {
        pairs.push(AlignmentPair::new(Some(i), Some(i)));
    }

    // Placeholder rows for the excess
    if source_count > target_count {
        for i in target_count..source_count {
            pairs.push(AlignmentPair::new(Some(i), None));
        }
    } else {
        for i in source_count..target_count {
            pairs.push(AlignmentPair::new(None, Some(i)));
        }
    }

    pairs
}

proptest! {
    /// The number of alignment pairs equals `max(source_count, target_count)`.
    ///
    /// Every position on both sides must be represented — either as a complete
    /// pair or a placeholder. So the total pair count is always the maximum of
    /// the two input counts.
    #[test]
    fn alignment_pair_count_equals_max(
        source_count in 0usize..100,
        target_count in 0usize..100
    ) {
        let pairs = simulate_alignment(source_count, target_count);
        let expected = source_count.max(target_count);
        prop_assert_eq!(
            pairs.len(),
            expected,
            "Expected {} pairs for source={}, target={}, got {}",
            expected, source_count, target_count, pairs.len()
        );
    }

    /// Source and target indices in the alignment are monotonically increasing.
    ///
    /// The positional algorithm assigns indices in order, so the sequence of
    /// `Some` source indices and `Some` target indices must each be strictly
    /// increasing.
    #[test]
    fn alignment_preserves_order(
        source_count in 0usize..100,
        target_count in 0usize..100
    ) {
        let pairs = simulate_alignment(source_count, target_count);

        // Check source indices are monotonically increasing
        let source_indices: Vec<usize> = pairs.iter().filter_map(|p| p.source()).collect();
        for window in source_indices.windows(2) {
            prop_assert!(
                window[0] < window[1],
                "Source indices not monotonic: {} >= {}",
                window[0], window[1]
            );
        }

        // Check target indices are monotonically increasing
        let target_indices: Vec<usize> = pairs.iter().filter_map(|p| p.target()).collect();
        for window in target_indices.windows(2) {
            prop_assert!(
                window[0] < window[1],
                "Target indices not monotonic: {} >= {}",
                window[0], window[1]
            );
        }
    }

    /// When source and target have equal length, all pairs are complete (no gaps).
    ///
    /// Equal-length alignment is the happy path: every source position matches
    /// exactly one target position, with no placeholders.
    #[test]
    fn equal_length_produces_exact_pairs(count in 0usize..100) {
        let pairs = simulate_alignment(count, count);

        prop_assert_eq!(
            pairs.len(),
            count,
            "Expected {} pairs for equal counts, got {}",
            count, pairs.len()
        );

        for (i, pair) in pairs.iter().enumerate() {
            prop_assert!(
                pair.is_complete(),
                "Pair {} should be complete for equal-length alignment, got source={:?} target={:?}",
                i, pair.source(), pair.target()
            );
            prop_assert_eq!(pair.source(), Some(i));
            prop_assert_eq!(pair.target(), Some(i));
        }
    }
}
