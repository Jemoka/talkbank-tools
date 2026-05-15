//! Alignment rendering orchestration.
//!
//! Iterates utterances, dispatches to per-tier renderers in [`tiers`], and
//! accumulates [`RenderTotals`] for the final [`summary`]. Tier filtering and
//! compact-mode suppression are resolved here so the individual tier renderers
//! don't need to re-check them.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::Path;

use talkbank_model::{AlignmentSet, ChatFile};

use crate::cli::AlignmentTier;

mod header;
mod summary;
mod tiers;

/// Render utterance-by-utterance alignment output plus summary totals.
///
/// This routine iterates through each validated utterance, pulls the alignment crates for `%mor`, `%gra`, `%pho`, and `%sin`,
/// and prints them in the style described in the CLI’s alignment debugging section of the CHAT manual. It respects
/// any `AlignmentTier` filter and optionally omits detail in compact mode. Totals recorded in `RenderTotals` keep the
/// summary consistent with the manual’s reporting format, which also highlights whether validation failed before rendering.
pub(super) fn render_alignments(
    input: &Path,
    chat_file: &ChatFile,
    tier_filter: Option<AlignmentTier>,
    compact: bool,
    had_validation_errors: bool,
) {
    header::render_intro(input);

    let mut totals = RenderTotals::default();

    for (utt_idx, utterance) in chat_file.utterances().enumerate() {
        let alignments = match &utterance.alignments {
            Some(alignments) => alignments,
            None => continue,
        };

        if !should_display_for_filter(tier_filter, alignments) {
            continue;
        }

        let main_content = utterance.main.content.to_content_string();
        if !compact {
            header::render_utterance_header(
                utt_idx,
                utterance.main.speaker.as_str(),
                &main_content,
            );
        }

        let mut shown_alignments = 0;

        if should_render_tier(tier_filter, AlignmentTier::Mor) {
            let (shown, errors) =
                tiers::render_main_to_mor(utterance, utt_idx, alignments, compact);
            shown_alignments += shown;
            totals.total_alignments += shown;
            totals.total_errors += errors;
        }

        if should_render_tier(tier_filter, AlignmentTier::Gra) {
            let (shown, errors) = tiers::render_mor_to_gra(utterance, utt_idx, alignments, compact);
            shown_alignments += shown;
            totals.total_alignments += shown;
            totals.total_errors += errors;
        }

        if should_render_tier(tier_filter, AlignmentTier::Pho) {
            let (shown, errors) =
                tiers::render_main_to_pho(utterance, utt_idx, alignments, compact);
            shown_alignments += shown;
            totals.total_alignments += shown;
            totals.total_errors += errors;
        }

        if should_render_tier(tier_filter, AlignmentTier::Sin) {
            let (shown, errors) =
                tiers::render_main_to_sin(utterance, utt_idx, alignments, compact);
            shown_alignments += shown;
            totals.total_alignments += shown;
            totals.total_errors += errors;
        }

        if shown_alignments == 0 && !compact {
            println!("  (No alignments for selected tier type)");
        }
    }

    summary::render_summary(input, tier_filter, &totals, had_validation_errors);
}

/// Aggregate counters accumulated during rendering.
///
/// Tracks how many alignments were shown and how many errors the renderer encountered so the summary view
/// surfaces the same metrics highlighted in the manual's alignment troubleshooting guide.
#[derive(Default)]
struct RenderTotals {
    total_alignments: usize,
    total_errors: usize,
}

/// Returns `true` when this utterance has alignment data for the selected filter.
/// Determine whether we should print an utterance given the selected tier filter.
///
/// The `AlignmentTier` filter is a developer convenience (Main Tier, %mor, %gra, %pho, %sin), echoing the CLI
/// documentation that instructs users to isolate a single dependent tier when hunting for misalignments.
fn should_display_for_filter(
    tier_filter: Option<AlignmentTier>,
    alignments: &AlignmentSet,
) -> bool {
    match tier_filter {
        None => true,
        Some(AlignmentTier::Mor) => alignments.mor.is_some(),
        Some(AlignmentTier::Gra) => alignments.gra.is_some(),
        Some(AlignmentTier::Pho) => alignments.pho.is_some(),
        Some(AlignmentTier::Sin) => alignments.sin.is_some(),
    }
}

/// Returns `true` when the requested tier should be rendered for this view.
/// Check whether the requested tier should be rendered for this view.
fn should_render_tier(tier_filter: Option<AlignmentTier>, tier: AlignmentTier) -> bool {
    match tier_filter {
        None => true,
        Some(filter) => filter == tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_filter_none_accepts_any_alignment_set() {
        let alignments = AlignmentSet::default();
        assert!(should_display_for_filter(None, &alignments));
    }

    #[test]
    fn display_filter_checks_selected_tier_presence() {
        let mut alignments = AlignmentSet::default();

        assert!(!should_display_for_filter(
            Some(AlignmentTier::Mor),
            &alignments
        ));
        assert!(!should_display_for_filter(
            Some(AlignmentTier::Gra),
            &alignments
        ));
        assert!(!should_display_for_filter(
            Some(AlignmentTier::Pho),
            &alignments
        ));
        assert!(!should_display_for_filter(
            Some(AlignmentTier::Sin),
            &alignments
        ));

        alignments.mor = Some(Default::default());
        alignments.gra = Some(Default::default());
        alignments.pho = Some(Default::default());
        alignments.sin = Some(Default::default());

        assert!(should_display_for_filter(
            Some(AlignmentTier::Mor),
            &alignments
        ));
        assert!(should_display_for_filter(
            Some(AlignmentTier::Gra),
            &alignments
        ));
        assert!(should_display_for_filter(
            Some(AlignmentTier::Pho),
            &alignments
        ));
        assert!(should_display_for_filter(
            Some(AlignmentTier::Sin),
            &alignments
        ));
    }

    #[test]
    fn render_tier_respects_filter() {
        assert!(should_render_tier(None, AlignmentTier::Mor));
        assert!(should_render_tier(None, AlignmentTier::Gra));
        assert!(should_render_tier(None, AlignmentTier::Pho));
        assert!(should_render_tier(None, AlignmentTier::Sin));

        assert!(should_render_tier(
            Some(AlignmentTier::Mor),
            AlignmentTier::Mor
        ));
        assert!(!should_render_tier(
            Some(AlignmentTier::Mor),
            AlignmentTier::Gra
        ));
        assert!(!should_render_tier(
            Some(AlignmentTier::Mor),
            AlignmentTier::Pho
        ));
        assert!(!should_render_tier(
            Some(AlignmentTier::Mor),
            AlignmentTier::Sin
        ));
    }
}
