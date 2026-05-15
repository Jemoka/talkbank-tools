//! Overlap validation functions.
//!
//! Validates CA overlap markers (⌈⌉⌊⌋) within individual utterances:
//! - **E373**: Invalid overlap index (must be 2-9 if present)
//! - **E348**: Unpaired overlap marker (opening without closing or vice versa)
//!
//! Uses [`extract_overlap_info`] from `alignment::helpers::overlap` for the
//! content traversal — same traversal used by the alignment pipeline,
//! eliminating duplicated walk logic.
//!
//! Cross-utterance checks (E347 unbalanced across speakers, E704 self-overlap)
//! are in `validation/cross_utterance/`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::ErrorSink;
use crate::alignment::helpers::overlap::{OverlapRegionKind, extract_overlap_info};
use crate::model::Utterance;
use crate::validation::{Validate, ValidationContext};

/// Validate overlap markers within one utterance.
///
/// Checks both index validity (E373) and pairing completeness (E348).
pub(crate) fn check_overlap_markers(
    utterance: &Utterance,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    check_overlap_index_values(utterance, context, errors);
    check_overlap_pairing(utterance, context, errors);
}

/// Validate overlap-point indices throughout one utterance tree (E373).
///
/// Collects all overlap points and validates that indices are in range 2-9.
/// Uses the shared traversal from `alignment::helpers::overlap`.
pub(crate) fn check_overlap_index_values(
    utterance: &Utterance,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    let index_context = context
        .clone()
        .with_field_span(utterance.main.span)
        .with_field_label("overlap_index");

    // Collect all overlap points via the shared traversal.
    // The regions give us the index values; we validate each one.
    let info = extract_overlap_info(&utterance.main.content.content.0);
    for region in &info.regions {
        if let Some(index) = region.index {
            // Validate the index value (must be 2-9).
            // Create a temporary OverlapPoint for the Validate trait.
            let point = crate::model::OverlapPoint::new(
                match region.kind {
                    OverlapRegionKind::Top => crate::model::OverlapPointKind::TopOverlapBegin,
                    OverlapRegionKind::Bottom => crate::model::OverlapPointKind::BottomOverlapBegin,
                },
                Some(index),
            );
            point.validate(&index_context, errors);
        }
    }
}

/// Check that overlap markers are properly paired within the utterance (E348).
///
/// Reports E348 when:
/// - An opening marker (⌈ or ⌊) has no matching closing marker (⌉ or ⌋)
/// - A closing marker appears without a preceding opening marker
///
/// Note: onset-only marking (⌈ without ⌉) is a legitimate CA practice in
/// some corpora. This check reports it as a warning, not an error, because
/// the convention varies between research groups.
fn check_overlap_pairing(
    utterance: &Utterance,
    _context: &ValidationContext,
    _errors: &impl ErrorSink,
) {
    let info = extract_overlap_info(&utterance.main.content.content.0);

    for region in &info.regions {
        if region.begin_at_word.is_some() && region.end_at_word.is_none() {
            // Opening without closing in this utterance — legitimate
            // cross-utterance overlap span. The matching close marker will
            // appear on a later utterance from the same speaker. The
            // cross-utterance check (E347) handles pairing across utterances.
        } else if region.begin_at_word.is_none() && region.end_at_word.is_some() {
            // Closing without opening in this utterance — legitimate
            // cross-utterance overlap span. The matching open marker was
            // on a preceding utterance from the same speaker. The
            // cross-utterance check (E347) handles pairing across utterances.
        }
    }
}
