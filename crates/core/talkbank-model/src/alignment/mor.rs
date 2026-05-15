//! Main-tier to `%mor` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::format::format_positional_mismatch;
use super::helpers::{
    TierDomain, TierPosition, collect_tier_items, count_tier_positions,
    to_chat_display_string as to_string,
};
use super::indices::{MainWordIndex, MorItemIndex};
use super::types::AlignmentPair;
use crate::model::{MainTier, MorTier, WriteChat};
use crate::{ErrorCode, ErrorLabel, ParseError, Severity};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for the main↔`%mor` alignment.
///
/// Target is a `%mor` **item** position ([`MorItemIndex`]), not a
/// `%mor` **chunk** position — the compiler rejects mixing with
/// [`MorChunkIndex`](crate::alignment::indices::MorChunkIndex), which
/// lives in `%gra` space and is a structurally different sequence
/// (items plus post-clitics plus terminator).
pub type MorAlignmentPair = AlignmentPair<MainWordIndex, MorItemIndex>;

/// Result of aligning main tier words to %mor tier morphological items.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct MorAlignment {
    /// Alignment pairs
    /// ([`MainWordIndex`]↔[`MorItemIndex`](crate::alignment::indices::MorItemIndex)).
    ///
    /// Indices are positions in the alignable content sequence.
    /// `None` in either position indicates a placeholder due to misalignment.
    pub pairs: Vec<MorAlignmentPair>,

    /// Errors produced while checking `%mor` alignment invariants.
    ///
    /// Includes count mismatches, terminator mismatches, and terminator
    /// presence/absence inconsistencies.
    pub errors: Vec<ParseError>,
}

impl MorAlignment {
    /// Create an empty alignment with no pairs or diagnostics.
    ///
    /// Useful as a neutral accumulator in builder-style alignment assembly.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Append one index-pair mapping to the alignment result.
    pub fn with_pair(mut self, pair: MorAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Append one diagnostic to the alignment result.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Return `true` when the alignment contains no diagnostics.
    ///
    /// Callers typically use this as a fast pass/fail check after alignment.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for MorAlignment {
    /// Builds an empty main-to-`%mor` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl super::traits::TierAlignmentResult for MorAlignment {
    type Pair = MorAlignmentPair;

    fn pairs(&self) -> &[MorAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: MorAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

/// Align main tier to %mor tier.
///
/// Validates 1-1 alignment between main tier alignable words and %mor items,
/// including terminator presence and value matching.
/// The result includes placeholder pairs (`None` on one side) when lengths
/// differ so UI tools can still render positional mismatch tables.
pub fn align_main_to_mor(main: &MainTier, mor: &MorTier) -> MorAlignment {
    let mut alignment = MorAlignment::new();

    // %mor terminator is type-enforced to always be present (see
    // `MorTier.terminator: Terminator` on the model). Main-tier
    // terminator remains `Option<Terminator>` because CA-mode files
    // legitimately omit it (~0.16% of corpus utterances). When main
    // has no terminator, the morphotag pipeline synthesizes one for
    // %mor (typically `.`). That synthesis is intentional and not
    // flagged here as a presence-mismatch — the prior E707
    // ("presence-mismatch") check no longer fires because the
    // Validate terminator value: %mor terminator must match the main
    // tier terminator exactly.
    if let Some(main_term) = &main.content.terminator {
        let main_str = main_term.to_chat_string();
        let mor_str = mor.terminator.to_chat_string();
        if main_str != mor_str {
            let error = build_alignment_error(
                main,
                mor,
                "E716",
                format!(
                    "Main tier terminator \"{}\" does not match %mor terminator \"{}\"",
                    main_str, mor_str,
                ),
                "The %mor tier terminator must match the main tier terminator exactly",
                mor.span,
            );
            alignment = alignment.with_error(error);
        }
    }

    // Extract alignable content indices from main tier
    let alignable_count = count_tier_positions(&main.content.content, TierDomain::Mor);

    // Terminator is now a separate field, not counted in items
    let expected_mor_count = alignable_count;

    let mor_count = mor.items.len();

    // Create 1-1 pairs for the common range
    let min_len = expected_mor_count.min(mor_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(AlignmentPair::new(
            Some(MainWordIndex::new(i)),
            Some(MorItemIndex::new(i)),
        ));
    }

    // Handle length mismatch
    if expected_mor_count > mor_count {
        let main_items = collect_tier_items(&main.content.content, TierDomain::Mor);
        let mor_items: Vec<TierPosition> = mor
            .items
            .iter()
            .map(|item| TierPosition {
                text: to_string(item),
                description: None,
            })
            .collect();

        let detailed_message =
            format_positional_mismatch("Main tier", "%mor tier", &main_items, &mor_items);

        let error = ParseError::at_span(
            ErrorCode::new("E705"),
            Severity::Error,
            main.span,
            detailed_message,
        )
        .with_label(ErrorLabel::new(main.span, "Main tier"))
        .with_label(ErrorLabel::new(mor.span, "%mor tier"))
        .with_suggestion("Each alignable word in main tier must have corresponding %mor item");

        alignment = alignment.with_error(error);

        // Add placeholders for extra main tier items
        for i in mor_count..expected_mor_count {
            alignment = alignment.with_pair(AlignmentPair::new(Some(MainWordIndex::new(i)), None));
        }
    } else if mor_count > expected_mor_count {
        let main_items = collect_tier_items(&main.content.content, TierDomain::Mor);
        let mor_items: Vec<TierPosition> = mor
            .items
            .iter()
            .map(|item| TierPosition {
                text: to_string(item),
                description: None,
            })
            .collect();

        let detailed_message =
            format_positional_mismatch("Main tier", "%mor tier", &main_items, &mor_items);

        let error = ParseError::at_span(
            ErrorCode::new("E706"),
            Severity::Error,
            main.span,
            detailed_message,
        )
        .with_label(ErrorLabel::new(main.span, "Main tier"))
        .with_label(ErrorLabel::new(mor.span, "%mor tier"))
        .with_suggestion("Remove extra %mor items or add corresponding words to main tier");

        alignment = alignment.with_error(error);

        // Add placeholders for extra %mor items
        for i in expected_mor_count..mor_count {
            alignment = alignment.with_pair(AlignmentPair::new(None, Some(MorItemIndex::new(i))));
        }
    }

    alignment
}

/// Build a `%mor` alignment error with shared labeling and preview context.
///
/// Centralizing this keeps all mismatch variants consistent in span labels and
/// suggestion formatting.
fn build_alignment_error(
    main: &MainTier,
    mor: &MorTier,
    code: &str,
    message: String,
    suggestion: &str,
    location_span: crate::Span,
) -> ParseError {
    let error_code = ErrorCode::new(code);
    let alignment_context = build_alignment_preview(main, mor);

    let suggestion_text = if alignment_context.is_empty() {
        suggestion.to_string()
    } else {
        format!(
            "{}\n\nAlignment preview:\n{}",
            suggestion, alignment_context
        )
    };

    let mut error = ParseError::at_span(error_code, Severity::Error, location_span, message)
        .with_suggestion(suggestion_text);

    if !main.span.is_dummy() {
        error
            .labels
            .push(crate::ErrorLabel::new(main.span, "Main tier"));
    }
    if !mor.span.is_dummy() {
        error
            .labels
            .push(crate::ErrorLabel::new(mor.span, "%mor tier"));
    }

    error
}

/// Build a compact side-by-side preview of main vs `%mor` alignable units.
///
/// The preview is appended to diagnostic suggestions to make count/value
/// mismatches easier to debug from one error message.
fn build_alignment_preview(main: &MainTier, mor: &MorTier) -> String {
    let main_items = collect_alignable_main_items(main);
    let mor_items = render_mor_items(mor);

    let mut out = String::new();
    out.push_str("Alignment (index: main | mor)\n");

    let max_len = main_items.len().max(mor_items.len());
    for idx in 0..max_len {
        let main_item = match main_items.get(idx) {
            Some(item) => item.as_str(),
            None => "<none>",
        };
        let mor_item = match mor_items.get(idx) {
            Some(item) => item.as_str(),
            None => "<none>",
        };
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!("{:>3}: {} | {}\n", idx, main_item, mor_item),
        );
    }

    out
}

/// Render `%mor` items into display strings for alignment previews.
fn render_mor_items(mor: &MorTier) -> Vec<String> {
    mor.items.iter().map(to_string).collect()
}

/// Collect alignable main-tier units in `%mor` alignment order.
///
/// Delegates to [`collect_tier_items`] to avoid duplicating the
/// exhaustive content traversal. Terminators are appended after lexical
/// units to mirror alignment reporting.
fn collect_alignable_main_items(main: &MainTier) -> Vec<String> {
    let mut items: Vec<String> = collect_tier_items(&main.content.content, TierDomain::Mor)
        .into_iter()
        .map(|item| item.text)
        .collect();
    if let Some(term) = &main.content.terminator {
        items.push(to_string(term));
    }
    items
}
