//! `SemanticDiff` implementations for container and wrapper types.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! Design note: sequence containers intentionally prefer element-level diffs
//! before length-only diffs. This keeps reports focused on the first semantic
//! divergence instead of flooding callers with cascading size noise.

use indexmap::IndexMap;
use smallvec::SmallVec;
use std::sync::Arc;

use crate::model::semantic_diff::{
    SemanticDiff, SemanticDiffContext, SemanticDiffKind, SemanticDiffReport, SemanticPath,
};

// =============================================================================
// Option
// =============================================================================

impl<T: SemanticDiff> SemanticDiff for Option<T> {
    /// Compares `Some` payloads recursively and reports `Some/None` shape mismatches.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        match (self, other) {
            (Some(left), Some(right)) => left.semantic_diff_into(right, path, report, ctx),
            (None, None) => {}
            (Some(_), None) => {
                report.push_with_context(path, SemanticDiffKind::ValueMismatch, "Some", "None", ctx)
            }
            (None, Some(_)) => {
                report.push_with_context(path, SemanticDiffKind::ValueMismatch, "None", "Some", ctx)
            }
        }
    }
}

// =============================================================================
// Vec
// =============================================================================

impl<T: SemanticDiff> SemanticDiff for Vec<T> {
    /// Diffs vectors by shared prefix first, then reports extra/missing tail.
    ///
    /// This ordering makes diagnostics point at the first true divergence
    /// rather than always emitting a top-level length mismatch.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        // First, diff shared elements to find actual type/value differences
        let shared = self.len().min(other.len());
        for idx in 0..shared {
            if report.is_truncated() {
                return;
            }
            path.push_index(idx);
            self[idx].semantic_diff_into(&other[idx], path, report, ctx);
            path.pop();
        }

        // If we found a diff in shared elements, don't also report length difference
        if report.is_truncated() {
            return;
        }

        // Only report length difference if all shared elements matched
        // (one vec is a prefix of the other)
        if self.len() != other.len() {
            // Report what's extra/missing at the divergence point
            let diverge_idx = shared;
            path.push_index(diverge_idx);
            if self.len() > other.len() {
                report.push_with_context(
                    path,
                    SemanticDiffKind::ExtraKey,
                    format!("item at [{}]", diverge_idx),
                    "missing",
                    ctx,
                );
            } else {
                report.push_with_context(
                    path,
                    SemanticDiffKind::MissingKey,
                    "missing",
                    format!("item at [{}]", diverge_idx),
                    ctx,
                );
            }
            path.pop();
        }
    }
}

// =============================================================================
// SmallVec
// =============================================================================

impl<A: smallvec::Array> SemanticDiff for SmallVec<A>
where
    A::Item: SemanticDiff,
{
    /// Mirrors `Vec<T>` diff semantics for `SmallVec` containers.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        // First, diff shared elements to find actual type/value differences
        let shared = self.len().min(other.len());
        for idx in 0..shared {
            if report.is_truncated() {
                return;
            }
            path.push_index(idx);
            self[idx].semantic_diff_into(&other[idx], path, report, ctx);
            path.pop();
        }

        // If we found a diff in shared elements, don't also report length difference
        if report.is_truncated() {
            return;
        }

        // Only report length difference if all shared elements matched
        if self.len() != other.len() {
            let diverge_idx = shared;
            path.push_index(diverge_idx);
            if self.len() > other.len() {
                report.push_with_context(
                    path,
                    SemanticDiffKind::ExtraKey,
                    format!("item at [{}]", diverge_idx),
                    "missing",
                    ctx,
                );
            } else {
                report.push_with_context(
                    path,
                    SemanticDiffKind::MissingKey,
                    "missing",
                    format!("item at [{}]", diverge_idx),
                    ctx,
                );
            }
            path.pop();
        }
    }
}

// =============================================================================
// Box
// =============================================================================

impl<T: SemanticDiff> SemanticDiff for Box<T> {
    /// Delegates to the boxed value.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        (**self).semantic_diff_into(&**other, path, report, ctx);
    }
}

// =============================================================================
// Arc
// =============================================================================

impl<T: SemanticDiff + ?Sized> SemanticDiff for Arc<T> {
    /// Delegates to the shared value instead of pointer identity.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        (**self).semantic_diff_into(&**other, path, report, ctx);
    }
}

// =============================================================================
// Tuples
// =============================================================================

impl<A: SemanticDiff, B: SemanticDiff> SemanticDiff for (A, B) {
    /// Diffs tuple fields by index order (`0`, then `1`).
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if report.is_truncated() {
            return;
        }
        path.push_index(0);
        self.0.semantic_diff_into(&other.0, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(1);
        self.1.semantic_diff_into(&other.1, path, report, ctx);
        path.pop();
    }
}

impl<A: SemanticDiff, B: SemanticDiff, C: SemanticDiff> SemanticDiff for (A, B, C) {
    /// Diffs tuple fields by index order (`0`, `1`, then `2`).
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if report.is_truncated() {
            return;
        }
        path.push_index(0);
        self.0.semantic_diff_into(&other.0, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(1);
        self.1.semantic_diff_into(&other.1, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(2);
        self.2.semantic_diff_into(&other.2, path, report, ctx);
        path.pop();
    }
}

// =============================================================================
// IndexMap
// =============================================================================

impl<K: SemanticDiff, V: SemanticDiff> SemanticDiff for IndexMap<K, V> {
    /// Diffs ordered map entries while preserving insertion-order semantics.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self.len() != other.len() {
            report.push_with_context(
                path,
                SemanticDiffKind::LengthMismatch,
                format!("len={}", self.len()),
                format!("len={}", other.len()),
                ctx,
            );
        }

        let shared = self.len().min(other.len());
        for idx in 0..shared {
            if report.is_truncated() {
                return;
            }
            let (left_key, left_value) = match self.get_index(idx) {
                Some(entry) => entry,
                None => {
                    return;
                }
            };
            let (right_key, right_value) = match other.get_index(idx) {
                Some(entry) => entry,
                None => {
                    return;
                }
            };

            path.push_index(idx);
            path.push_field("key");
            left_key.semantic_diff_into(right_key, path, report, ctx);
            path.pop();
            if report.is_truncated() {
                path.pop();
                return;
            }
            path.push_field("value");
            left_value.semantic_diff_into(right_value, path, report, ctx);
            path.pop();
            path.pop();
        }
    }
}
