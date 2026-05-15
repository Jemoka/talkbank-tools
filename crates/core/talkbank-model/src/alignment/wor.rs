//! Main-tier to `%wor` timing sidecar.
//!
//! `%wor` is **not** a structural tier alignment. It is a timing-annotation
//! sidecar: a record of bullets attached to the subset of main-tier words
//! that passed the Wor-domain filter at the moment `batchalign3 align` ran.
//! See KIB-016 in
//! `talkbank-tools/book/src/apps/vscode-extension/developer/known-issues-and-backlog.md`
//! for the full reclassification rationale.
//!
//! The older `WorAlignment` / `align_main_to_wor` /
//! `WorTier: AlignableTier` design modeled `%wor` as a fifth positional
//! alignment alongside `%mor`, `%gra`, `%pho`, `%sin`. It didn't fit: count
//! mismatches are tolerated (stale `%wor` after main-tier edits is
//! legitimate), error codes were never surfaced, and there is no consumer
//! anywhere that indexes `%wor` by position for any purpose other than
//! timing recovery — and that single consumer already fails closed on any
//! count mismatch. This module replaces that machinery with the narrower
//! [`WorTimingSidecar`] model that describes what `%wor` actually is.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use super::helpers::{TierDomain, count_tier_positions};
use crate::model::{MainTier, WorTier};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Describes the correspondence between the main tier's Wor-filtered words
/// and the entries in a `%wor` tier, at the moment of read.
///
/// Variants express *what is safely recoverable*, not *whether validation
/// passes*. `%wor` has no validation contract against the main tier; the
/// presence of drift is a fact to report to callers, not an error.
///
/// - [`Positional`](Self::Positional) — filtered counts matched, so the
///   i-th Wor-filtered main-tier word corresponds to the i-th entry in
///   `%wor.words()`. Callers may zip positionally; this is the only mode
///   in which timing recovery is defined.
/// - [`Drifted`](Self::Drifted) — filtered counts differ (typically a main
///   tier edit after `align` without a re-run). No positional correspondence
///   is available; timing recovery must be skipped or the file must be
///   re-aligned. Carries the two counts so callers can log or display them.
///
/// `None` at the containing [`Option<WorTimingSidecar>`] level on
/// [`AlignmentSet`](crate::model::AlignmentSet) means the utterance has no
/// `%wor` tier at all.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorTimingSidecar {
    /// Filtered main-tier and `%wor` word counts match. The i-th entry of
    /// each sequence corresponds to the other. `count` is the common length.
    Positional {
        /// Shared length of the Wor-filtered main sequence and `%wor.words()`.
        count: usize,
    },
    /// Filtered counts differ. No positional correspondence is defined.
    Drifted {
        /// Number of Wor-filtered main-tier words.
        main_count: usize,
        /// Number of `%wor.words()` entries.
        wor_count: usize,
    },
}

impl WorTimingSidecar {
    /// Returns `true` when positional timing recovery is defined.
    ///
    /// This is the single predicate consumers should call before zipping
    /// main-tier Wor-filtered words against `%wor.words()`.
    pub fn is_positional(&self) -> bool {
        matches!(self, Self::Positional { .. })
    }

    /// Returns the shared count when positional, otherwise `None`.
    pub fn positional_count(&self) -> Option<usize> {
        match self {
            Self::Positional { count } => Some(*count),
            Self::Drifted { .. } => None,
        }
    }
}

/// Resolve the correspondence between a main tier and its `%wor` tier.
///
/// Counts Wor-filtered alignable words on the main tier (via
/// [`TierDomain::Wor`]) and words on the `%wor` tier, then returns either
/// [`WorTimingSidecar::Positional`] (counts match; callers can zip) or
/// [`WorTimingSidecar::Drifted`] (counts differ; no positional recovery).
///
/// This function never produces a [`ParseError`](crate::ParseError) —
/// mismatch is a fact about the pair, not a validation failure.
pub fn resolve_wor_timing_sidecar(main: &MainTier, wor: &WorTier) -> WorTimingSidecar {
    let main_count = count_tier_positions(&main.content.content, TierDomain::Wor);
    let wor_count = wor.word_count();
    if main_count == wor_count {
        WorTimingSidecar::Positional { count: main_count }
    } else {
        WorTimingSidecar::Drifted {
            main_count,
            wor_count,
        }
    }
}

// =============================================================================
// Tests for the %wor timing sidecar
// =============================================================================

#[cfg(test)]
mod wor_sidecar_tests {
    use super::*;
    use crate::Span;
    use crate::model::{Terminator, UtteranceContent, Word};

    fn word(form: &str) -> UtteranceContent {
        UtteranceContent::Word(Box::new(Word::new_unchecked(form, form)))
    }

    fn wor_tier(forms: &[&str]) -> WorTier {
        WorTier::from_words(forms.iter().map(|f| Word::new_unchecked(*f, *f)).collect())
    }

    /// Perfect count match yields `Positional`.
    #[test]
    fn positional_when_counts_match() {
        let main = MainTier::new(
            "CHI",
            vec![word("hello"), word("world")],
            Terminator::Period { span: Span::DUMMY },
        );
        let wor = wor_tier(&["hello", "world"]);

        let sidecar = resolve_wor_timing_sidecar(&main, &wor);

        assert_eq!(sidecar, WorTimingSidecar::Positional { count: 2 });
        assert!(sidecar.is_positional());
        assert_eq!(sidecar.positional_count(), Some(2));
    }

    /// Main longer than `%wor` yields `Drifted` (not an error).
    ///
    /// Drift is the common case after a transcript edit without
    /// re-running `align`.
    #[test]
    fn drifted_when_main_longer() {
        let main = MainTier::new(
            "CHI",
            vec![word("one"), word("two"), word("three")],
            Terminator::Period { span: Span::DUMMY },
        );
        let wor = wor_tier(&["one", "two"]);

        let sidecar = resolve_wor_timing_sidecar(&main, &wor);

        assert_eq!(
            sidecar,
            WorTimingSidecar::Drifted {
                main_count: 3,
                wor_count: 2
            }
        );
        assert!(!sidecar.is_positional());
        assert_eq!(sidecar.positional_count(), None);
    }

    /// `%wor` longer than main yields `Drifted` symmetrically.
    #[test]
    fn drifted_when_wor_longer() {
        let main = MainTier::new(
            "CHI",
            vec![word("one")],
            Terminator::Period { span: Span::DUMMY },
        );
        let wor = wor_tier(&["one", "extra"]);

        let sidecar = resolve_wor_timing_sidecar(&main, &wor);

        assert_eq!(
            sidecar,
            WorTimingSidecar::Drifted {
                main_count: 1,
                wor_count: 2
            }
        );
    }

    /// Empty on both sides is still `Positional` with count 0.
    #[test]
    fn positional_when_both_empty() {
        let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
        let wor = wor_tier(&[]);

        assert_eq!(
            resolve_wor_timing_sidecar(&main, &wor),
            WorTimingSidecar::Positional { count: 0 }
        );
    }
}
