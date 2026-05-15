//! `SemanticDiff` implementations for scalar and string-like types.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! These impls provide leaf-level comparisons used by all higher-level model
//! diffs. They intentionally report `ValueMismatch` only, leaving structural
//! path context to container/object walkers.

use std::borrow::Cow;

use crate::Span;
use crate::model::semantic_diff::{
    SemanticDiff, SemanticDiffContext, SemanticDiffKind, SemanticDiffReport, SemanticPath,
};

// =============================================================================
// Critical Implementation: Span is always Semantically Equal
// =============================================================================

impl SemanticDiff for Span {
    /// Treats source spans as semantically equal.
    ///
    /// `Span` carries positional metadata, not linguistic content, so span-only
    /// differences should not fail semantic-equivalence checks.
    fn semantic_diff_into(
        &self,
        _other: &Self,
        _path: &mut SemanticPath,
        _report: &mut SemanticDiffReport,
        _ctx: &mut SemanticDiffContext,
    ) {
        // Spans are always semantically equal - they're metadata, not content
    }
}

// =============================================================================
// String implementations
// =============================================================================

impl SemanticDiff for String {
    /// Compares owned strings by exact byte content.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self != other {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{:?}", self),
                format!("{:?}", other),
                ctx,
            );
        }
    }
}

impl SemanticDiff for smol_str::SmolStr {
    /// Compares interned/small strings by exact byte content.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self != other {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{:?}", self),
                format!("{:?}", other),
                ctx,
            );
        }
    }
}

impl SemanticDiff for str {
    /// Compares borrowed string slices by exact byte content.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self != other {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{:?}", self),
                format!("{:?}", other),
                ctx,
            );
        }
    }
}

impl SemanticDiff for &str {
    /// Compares borrowed `&str` references by pointed-to content.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if *self != *other {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{:?}", self),
                format!("{:?}", other),
                ctx,
            );
        }
    }
}

impl<'a> SemanticDiff for Cow<'a, str> {
    /// Compares `Cow<str>` values by normalized borrowed view.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self.as_ref() != other.as_ref() {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{:?}", self.as_ref()),
                format!("{:?}", other.as_ref()),
                ctx,
            );
        }
    }
}

// =============================================================================
// Numeric and primitive implementations via macro
// =============================================================================

macro_rules! impl_semantic_diff_scalar {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl SemanticDiff for $ty {
                fn semantic_diff_into(
                    &self,
                    other: &Self,
                    path: &mut SemanticPath,
                    report: &mut SemanticDiffReport,
                    ctx: &mut SemanticDiffContext,
                ) {
                    if self != other {
                        report.push_with_context(
                            path,
                            SemanticDiffKind::ValueMismatch,
                            self.to_string(),
                            other.to_string(),
                            ctx,
                        );
                    }
                }
            }
        )+
    };
}

impl_semantic_diff_scalar!(bool, char, u8, u16, u32, u64, usize, i8, i16, i32, i64);
