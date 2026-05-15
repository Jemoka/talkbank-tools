//! Alignment trait abstractions.
//!
//! These traits formalize the shared patterns across all tier alignment passes:
//!
//! - [`IndexPair`] — positional index pair (source position ↔ target position)
//! - [`TierAlignmentResult`] — result accumulator with pairs and diagnostics
//! - [`AlignableTier`] — what a dependent tier must provide for generic positional alignment
//! - [`TierCountable`] — domain-gated counting and extraction on utterance content
//!
//! The [`positional_align`] function implements the shared 1:1 alignment algorithm
//! used by `%pho`, `%sin`, and `%wor` tier alignment. `%mor` and `%gra` have extra
//! domain-specific logic and use the traits without the generic function.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::format::format_positional_mismatch;
use super::helpers::{TierDomain, TierPosition, collect_tier_items, count_tier_positions};
use super::types::AlignmentPair;
use crate::model::{MainTier, UtteranceContent};
use crate::{ErrorCode, ErrorLabel, ParseError, Severity, Span};

// ---------------------------------------------------------------------------
// IndexPair
// ---------------------------------------------------------------------------

/// A positional index pair mapping one source position to one target position.
///
/// Both [`AlignmentPair`] (main↔dependent) and
/// [`GraAlignmentPair`](super::GraAlignmentPair) (%mor↔%gra) implement this
/// trait, enabling generic code that operates on any alignment pair type.
pub trait IndexPair: Clone {
    /// Source-side index, or `None` for placeholder rows (extra target items).
    fn source(&self) -> Option<usize>;

    /// Target-side index, or `None` for placeholder rows (extra source items).
    fn target(&self) -> Option<usize>;

    /// Construct a pair from optional indices.
    fn from_indices(source: Option<usize>, target: Option<usize>) -> Self;

    /// Returns `true` when both indices are present (concrete 1:1 match).
    fn is_complete(&self) -> bool {
        self.source().is_some() && self.target().is_some()
    }

    /// Returns `true` when at least one index is `None` (mismatch placeholder).
    fn is_placeholder(&self) -> bool {
        !self.is_complete()
    }
}

// ---------------------------------------------------------------------------
// TierAlignmentResult
// ---------------------------------------------------------------------------

/// Accumulator for tier alignment results: pairs plus diagnostics.
///
/// The four structural alignment result types
/// ([`MorAlignment`](super::MorAlignment),
/// [`PhoAlignment`](super::PhoAlignment),
/// [`SinAlignment`](super::SinAlignment),
/// [`GraAlignment`](super::GraAlignment)) implement this trait with
/// identical structure. `%wor` is intentionally not a member of this
/// family — see [`WorTimingSidecar`](super::WorTimingSidecar) and
/// KIB-016.
pub trait TierAlignmentResult: Default {
    /// The index pair type used by this alignment.
    type Pair: IndexPair;

    /// Immutable access to the accumulated alignment pairs.
    fn pairs(&self) -> &[Self::Pair];

    /// Immutable access to the accumulated diagnostic errors.
    fn errors(&self) -> &[ParseError];

    /// Append one pair to the result.
    fn push_pair(&mut self, pair: Self::Pair);

    /// Append one diagnostic to the result.
    fn push_error(&mut self, error: ParseError);

    /// Returns `true` when no diagnostics were emitted.
    ///
    /// A `true` value implies every row in `pairs()` is a complete 1:1 match.
    fn is_error_free(&self) -> bool {
        self.errors().is_empty()
    }
}

// ---------------------------------------------------------------------------
// AlignableTier
// ---------------------------------------------------------------------------

/// What a dependent tier must provide for generic 1:1 positional alignment
/// against a main tier.
///
/// Implementors provide the domain, target item count, diagnostic metadata,
/// and error codes. The [`positional_align`] function handles the shared
/// alignment algorithm.
///
/// # Example
///
/// ```ignore
/// impl AlignableTier for PhoTier {
///     const DOMAIN: TierDomain = TierDomain::Pho;
///     fn tier_name(&self) -> &str { "%pho tier" }
///     fn target_count(&self) -> usize { self.len() }
///     // ...
/// }
///
/// let (pairs, errors) = positional_align(&main, &pho);
/// ```
pub trait AlignableTier {
    /// Typed source (main-tier side) index carried by alignment pairs.
    ///
    /// Every tier-alignment domain binds the source to the same
    /// main-tier position space, so this is almost always
    /// [`MainWordIndex`](super::indices::MainWordIndex). It is kept as
    /// an associated type (rather than fixed) so that a future
    /// extension can introduce alternate source newtypes without
    /// rewriting the trait.
    type Source: Copy + From<usize> + Into<usize>;

    /// Typed target (dependent-tier side) index carried by alignment
    /// pairs — e.g. [`MorItemIndex`](super::indices::MorItemIndex),
    /// [`PhoItemIndex`](super::indices::PhoItemIndex),
    /// [`SinItemIndex`](super::indices::SinItemIndex). Binding the target
    /// per-tier is what lets the compiler reject cross-tier index
    /// confusion (e.g. a `MorItemIndex` reaching a `%pho` consumer).
    type Target: Copy + From<usize> + Into<usize>;

    /// The alignment domain used for counting main-tier items.
    const DOMAIN: TierDomain;

    /// Display name for diagnostic messages (e.g., `"%pho tier"`).
    fn tier_name(&self) -> &str;

    /// Number of items on the target (dependent) side.
    fn target_count(&self) -> usize;

    /// Convert target items to diagnostic text for mismatch rendering.
    fn extract_target_items(&self) -> Vec<TierPosition>;

    /// Source span for error labels.
    fn span(&self) -> Span;

    /// Error code when the main tier has more alignable items than this tier.
    fn error_code_too_few(&self) -> ErrorCode;

    /// Error code when this tier has more items than the main tier.
    fn error_code_too_many(&self) -> ErrorCode;

    /// Suggestion text for the "too few target items" error.
    fn suggestion_too_few(&self) -> &str;

    /// Suggestion text for the "too many target items" error.
    fn suggestion_too_many(&self) -> &str;
}

// ---------------------------------------------------------------------------
// positional_align
// ---------------------------------------------------------------------------

/// Output of [`positional_align`]: the aligned `(source, target)` pairs
/// plus any `ParseError` diagnostics surfaced during matching. Callers
/// wrap this into their domain-specific result (`PhoAlignment`,
/// `SinAlignment`, `WorAlignment`).
pub type PositionalAlignment<T> = (
    Vec<AlignmentPair<<T as AlignableTier>::Source, <T as AlignableTier>::Target>>,
    Vec<ParseError>,
);

/// Generic 1:1 positional alignment of a main tier against any [`AlignableTier`].
///
/// Implements the shared algorithm used by `%pho`, `%sin`, and `%wor` alignment:
///
/// 1. Count alignable items on the main tier (using `T::DOMAIN`).
/// 2. Count target items via [`AlignableTier::target_count`].
/// 3. Emit complete `(Some(i), Some(i))` pairs for the common prefix.
/// 4. On mismatch: extract items for diagnostics, build error, add placeholders.
///
/// Returns `(pairs, errors)` which callers wrap in their domain-specific result type.
pub fn positional_align<T: AlignableTier>(main: &MainTier, tier: &T) -> PositionalAlignment<T> {
    let alignable_count = count_tier_positions(&main.content.content, T::DOMAIN);
    let target_count = tier.target_count();

    let mut pairs: Vec<AlignmentPair<T::Source, T::Target>> =
        Vec::with_capacity(alignable_count.max(target_count));
    let mut errors = Vec::new();

    // 1:1 pairs for the common range
    let min_len = alignable_count.min(target_count);
    for i in 0..min_len {
        pairs.push(AlignmentPair::new(
            Some(T::Source::from(i)),
            Some(T::Target::from(i)),
        ));
    }

    // Mismatch handling
    if alignable_count != target_count {
        let main_items = collect_tier_items(&main.content.content, T::DOMAIN);
        let target_items = tier.extract_target_items();

        let detailed_message =
            format_positional_mismatch("Main tier", tier.tier_name(), &main_items, &target_items);

        let (code, suggestion) = if alignable_count > target_count {
            (tier.error_code_too_few(), tier.suggestion_too_few())
        } else {
            (tier.error_code_too_many(), tier.suggestion_too_many())
        };

        let error = ParseError::at_span(code, Severity::Error, main.span, detailed_message)
            .with_label(ErrorLabel::new(main.span, "Main tier"))
            .with_label(ErrorLabel::new(tier.span(), tier.tier_name()))
            .with_suggestion(suggestion);

        errors.push(error);

        // Placeholder rows for the excess
        if alignable_count > target_count {
            for i in target_count..alignable_count {
                pairs.push(AlignmentPair::new(Some(T::Source::from(i)), None));
            }
        } else {
            for i in alignable_count..target_count {
                pairs.push(AlignmentPair::new(None, Some(T::Target::from(i))));
            }
        }
    }

    (pairs, errors)
}

// ---------------------------------------------------------------------------
// TierCountable
// ---------------------------------------------------------------------------

/// Domain-gated counting and extraction on utterance content sequences.
///
/// Provides method syntax for the operations in [`helpers::count`](super::helpers):
///
/// ```ignore
/// let count = content.count_alignable(TierDomain::Mor);
/// let items = content.extract_alignable(TierDomain::Pho);
/// ```
pub trait TierCountable {
    /// Count alignable items for the given domain.
    fn count_alignable(&self, domain: TierDomain) -> usize;

    /// Extract alignable items with display text for diagnostics.
    fn extract_alignable(&self, domain: TierDomain) -> Vec<TierPosition>;
}

impl TierCountable for [UtteranceContent] {
    fn count_alignable(&self, domain: TierDomain) -> usize {
        count_tier_positions(self, domain)
    }

    fn extract_alignable(&self, domain: TierDomain) -> Vec<TierPosition> {
        collect_tier_items(self, domain)
    }
}
