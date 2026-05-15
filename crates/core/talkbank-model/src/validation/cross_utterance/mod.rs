//! Cross-utterance validation for quotation patterns and completion linkers
//!
//! This module validates relationships between utterances that require looking
//! at sequences of utterances, particularly:
//! - Quotation patterns (Pattern A: +"/. and Pattern B: +". )
//! - Completion linkers (+, and ++)
//!
//! ## Disabled Validations
//!
//! **NOTE**: All quotation marker cross-utterance validations are currently **DISABLED** (2025-12-28).
//!
//! ### Rationale
//! The legacy CHAT system (Java) never performed validation of these cross-utterance patterns.
//! These validations were implemented fresh in the Rust version to enforce strict CHAT
//! conventions for quoted passages and completion sequences. However, analysis of real-world
//! corpora (particularly CORAAL, OralArguments) shows that these strict sequential patterns
//! don't match natural conversational flow:
//!
//! - **E341**: Quotation follows (`+"/. `) - Requires next same-speaker utterance to have `+"` linker.
//!   Fails when speakers don't continue with quoted content or other speakers interject.
//!
//! - **E344**: Quotation precedes (`+".`) - Requires preceding same-speaker utterances with `+"` linkers.
//!   Fails when quoted content appears without preceding marked utterances.
//!
//! - **E346**: Quoted utterance linker (`+"`) - Requires upstream/downstream same-speaker
//!   utterances to end with `+"/` or `+".` respectively. Fails when attorneys interrupt
//!   or continue quoted passages in non-canonical ways.
//!
//! - **E352**: Self-completion linker (`+,`) - Requires preceding same-speaker utterance
//!   to end with `+/.` (interruption). Fails when speakers resume utterances in
//!   natural conversation without interruption markers.
//!
//! ### Decision
//! Rather than relax or modify these validations (which could introduce new bugs), we're
//! temporarily disabling them to allow corpus-wide roundtrip testing. This lets us:
//!
//! 1. Make progress on roundtrip testing across all 807 corpora
//! 2. Collect real data on how these patterns actually appear in various CHAT corpora
//! 3. Later make an informed decision on validation approach:
//!    - Should these be warnings, not errors?
//!    - Should they be context-sensitive (different rules for different corpus types)?
//!    - Should we relax the sequential pattern matching?
//!    - Should we restore the old behavior (no validation)?
//!
//! ### Files with Disabled Logic
//! - `quotation_follows.rs`: check_quotation_follows() - DISABLED (E341)
//! - `quotation_precedes.rs`: check_quotation_precedes() - DISABLED (E344)
//! - `quoted_linker.rs`: check_quoted_linker() - DISABLED (E346)
//! - `completion.rs`: check_self_completion() - DISABLED (E352)
//! - `completion.rs`: check_other_completion() - DISABLED (E349/E350)
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

mod completion;
mod helpers;
mod quotation_follows;
mod quotation_precedes;
mod quoted_linker;
mod scoped_markers;
#[cfg(test)]
mod tests;

use crate::model::{OverlapPointKind, Terminator, Utterance, UtteranceContent};
use crate::{ErrorCollector, ErrorSink, ParseError};
use helpers::has_quoted_linker;

/// Validates cross-utterance constraints and returns collected diagnostics.
///
/// This is the allocation-friendly convenience entrypoint used by callers that
/// do not need to reuse a custom error sink. It always runs scoped-marker and
/// overlap checks, while quotation-specific checks depend on runtime context flags.
pub fn check_cross_utterance_patterns(
    utterances: &[Utterance],
    context: &crate::validation::ValidationContext,
) -> Vec<ParseError> {
    let errors = ErrorCollector::new();
    check_cross_utterance_patterns_with_sink(utterances, context, &errors);
    errors.into_vec()
}

/// Validates cross-utterance constraints using a caller-provided error sink.
///
/// This function centralizes the full rule dispatch order, including feature-
/// gated quotation checks and always-on scoped-marker balancing. The ordering
/// is intentionally deterministic so diagnostics remain stable across runs.
pub(crate) fn check_cross_utterance_patterns_with_sink(
    utterances: &[Utterance],
    context: &crate::validation::ValidationContext,
    errors: &impl ErrorSink,
) {
    for (idx, utterance) in utterances.iter().enumerate() {
        // Quotation follows pattern (Pattern A - E341)
        // Gated behind runtime flag - disabled by default
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation
            && let Some(ref term) = utterance.main.content.terminator
            && matches!(term, Terminator::QuotedNewLine { .. })
        {
            errors.report_all(quotation_follows::check_quotation_follows(utterances, idx));
        }

        // Quotation precedes pattern (Pattern B - E344)
        // Gated behind runtime flag - disabled by default
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation
            && let Some(ref term) = utterance.main.content.terminator
            && matches!(term, Terminator::QuotedPeriodSimple { .. })
        {
            errors.report_all(quotation_precedes::check_quotation_precedes(
                utterances, idx,
            ));
        }

        // Quoted utterance linker (E346)
        // Gated behind runtime flag - disabled by default
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation && has_quoted_linker(utterance) {
            errors.report_all(quoted_linker::check_quoted_linker(utterances, idx));
        }

        // Other-completion linker (++) — gated behind runtime flag.
        // See module-level documentation for rationale on default-disabled state.
        if context.shared.enable_quotation_validation
            && helpers::has_other_completion_linker(utterance)
        {
            errors.report_all(completion::check_other_completion(utterances, idx));
        }
    }

    // Self-completion linker (E352) - O(n) batch validation
    // Gated behind runtime flag - disabled by default
    // See module-level documentation for rationale
    if context.shared.enable_quotation_validation {
        completion::check_self_completion_all(utterances, errors);
    }

    // Validate scoped markers that can span across utterances
    scoped_markers::check_long_feature_balance(utterances, errors);
    scoped_markers::check_nonvocal_balance(utterances, errors);

    // E704: Same speaker must not encode top+bottom overlap pair with self.
    check_self_overlap_markers(utterances, errors);

    // E347: Cross-utterance overlap balance — top regions should have
    // matching bottom regions on a different speaker.
    check_cross_utterance_overlap_balance(utterances, errors);
}

/// Rejects overlap pairs that imply a speaker overlapping with themself.
///
/// Adjacent utterances by the same speaker must not encode a top-overlap pair
/// followed by a bottom-overlap pair, which semantically implies self-overlap.
/// This catches annotation slips that are easy to miss when overlap brackets
/// are edited manually across turn boundaries.
fn check_self_overlap_markers(utterances: &[Utterance], errors: &impl ErrorSink) {
    use crate::{ErrorCode, ErrorContext, Severity, SourceLocation};

    for pair in utterances.windows(2) {
        let first = &pair[0];
        let second = &pair[1];

        if first.main.speaker != second.main.speaker {
            continue;
        }

        let first_has_top = has_overlap_kind(
            &first.main.content.content,
            OverlapPointKind::TopOverlapBegin,
            OverlapPointKind::TopOverlapEnd,
        );
        let second_has_bottom = has_overlap_kind(
            &second.main.content.content,
            OverlapPointKind::BottomOverlapBegin,
            OverlapPointKind::BottomOverlapEnd,
        );

        if first_has_top && second_has_bottom {
            let span = second.main.span;
            let speaker = second.main.speaker.as_str();
            errors.report(
                crate::ParseError::new(
                    ErrorCode::SpeakerSelfOverlap,
                    Severity::Error,
                    SourceLocation::new(span),
                    ErrorContext::new(speaker, span, speaker),
                    format!(
                        "Speaker '{}' has overlapping top/bottom overlap markers across adjacent utterances",
                        speaker
                    ),
                )
                .with_suggestion(
                    "Overlap markers should represent overlap between different speakers",
                ),
            );
        }
    }
}

/// Returns whether the utterance has a well-paired overlap region of the given kind.
///
/// Uses `extract_overlap_info` to check all content levels (including intra-word
/// markers). Requires both begin and end markers to be present.
fn has_overlap_kind(
    content: &[UtteranceContent],
    begin_kind: OverlapPointKind,
    _end_kind: OverlapPointKind,
) -> bool {
    use crate::alignment::helpers::overlap::{OverlapRegionKind, extract_overlap_info};

    let target_kind = match begin_kind {
        OverlapPointKind::TopOverlapBegin => OverlapRegionKind::Top,
        OverlapPointKind::BottomOverlapBegin => OverlapRegionKind::Bottom,
        _ => return false,
    };

    let info = extract_overlap_info(content);
    info.regions
        .iter()
        .any(|r| r.kind == target_kind && r.is_well_paired())
}

/// Validate cross-utterance overlap balance (E347).
///
/// Uses [`analyze_file_overlaps`] for proper 1:N matching — one top region
/// from speaker A can be matched by bottom regions from speakers B, C, etc.
/// Only orphaned tops (no matching bottom from any speaker) and orphaned
/// bottoms (no matching top from any speaker) are reported.
fn check_cross_utterance_overlap_balance(utterances: &[Utterance], errors: &impl ErrorSink) {
    use crate::{ErrorCode, ErrorContext, Severity, SourceLocation};

    // Convert &[Utterance] to &[Line] for analyze_file_overlaps.
    // We need Line references, but utterances is &[Utterance]. Build Line
    // wrappers on the fly — the analysis only reads, doesn't mutate.
    //
    // Since analyze_file_overlaps takes &[Line], we need to construct Lines.
    // But we only have &[Utterance]. Re-extract directly instead.
    let analysis = {
        use crate::alignment::helpers::overlap::{OverlapRegionKind, extract_overlap_info};
        use crate::alignment::helpers::overlap_groups::{
            FileOverlapAnalysis, OverlapAnchor, OverlapGroup, PerUtteranceOverlap,
        };

        // Replicate the core logic of analyze_file_overlaps on &[Utterance]
        let mut per_utterance: Vec<PerUtteranceOverlap> = Vec::new();
        for utt in utterances {
            let info = extract_overlap_info(&utt.main.content.content.0);
            let bullet = utt
                .main
                .content
                .bullet
                .as_ref()
                .map(|b| (b.timing.start_ms, b.timing.end_ms));
            per_utterance.push(PerUtteranceOverlap {
                utterance_index: per_utterance.len(),
                speaker: utt.main.speaker.to_string(),
                info,
                bullet,
            });
        }

        let mut tops: Vec<OverlapAnchor> = Vec::new();
        let mut bottoms: Vec<OverlapAnchor> = Vec::new();
        for pu in &per_utterance {
            for region in &pu.info.regions {
                let anchor = OverlapAnchor {
                    utterance_index: pu.utterance_index,
                    speaker: pu.speaker.clone(),
                    region: region.clone(),
                    bullet: pu.bullet,
                };
                match region.kind {
                    OverlapRegionKind::Top if region.has_begin() => tops.push(anchor),
                    OverlapRegionKind::Bottom if region.has_begin() => bottoms.push(anchor),
                    _ => {}
                }
            }
        }

        let mut top_to_bottoms: Vec<Vec<OverlapAnchor>> = vec![Vec::new(); tops.len()];
        let mut bottom_matched: Vec<bool> = vec![false; bottoms.len()];

        for (bi, bottom) in bottoms.iter().enumerate() {
            // Reverse scan (nearest top first), matching 1:N across
            // speakers. The only refinement over a naive reverse scan:
            // when this bottom's speaker already has a sibling bottom
            // attached to the candidate top, AND there is a "vacant
            // sibling top" (same speaker, same utterance, same index,
            // no bottom from this speaker yet), skip to distribute
            // evenly. This prevents two FM bottoms from collapsing
            // onto one AM top when two AM tops exist.
            for (ti, top) in tops.iter().enumerate().rev() {
                if top.speaker == bottom.speaker {
                    continue;
                }
                if top.region.index != bottom.region.index {
                    continue;
                }
                if top.utterance_index > bottom.utterance_index {
                    continue;
                }
                // Distribution guard for same-speaker-pair siblings.
                let already_has_same_speaker = top_to_bottoms[ti]
                    .iter()
                    .any(|b| b.speaker == bottom.speaker);
                if already_has_same_speaker {
                    let has_vacant_sibling = tops.iter().enumerate().any(|(oti, other_top)| {
                        oti != ti
                            && other_top.speaker == top.speaker
                            && other_top.utterance_index == top.utterance_index
                            && other_top.region.index == top.region.index
                            && other_top.utterance_index <= bottom.utterance_index
                            && !top_to_bottoms[oti]
                                .iter()
                                .any(|b| b.speaker == bottom.speaker)
                    });
                    if has_vacant_sibling {
                        continue;
                    }
                }
                top_to_bottoms[ti].push(bottom.clone());
                bottom_matched[bi] = true;
                break;
            }
        }

        let mut groups = Vec::new();
        let mut orphaned_tops = Vec::new();
        for (ti, top) in tops.into_iter().enumerate() {
            let matched = std::mem::take(&mut top_to_bottoms[ti]);
            if matched.is_empty() {
                orphaned_tops.push(top);
            } else {
                groups.push(OverlapGroup {
                    top,
                    bottoms: matched,
                });
            }
        }

        let orphaned_bottoms: Vec<OverlapAnchor> = bottoms
            .into_iter()
            .zip(bottom_matched)
            .filter(|(_, m)| !*m)
            .map(|(b, _)| b)
            .collect();

        FileOverlapAnalysis {
            groups,
            orphaned_tops,
            orphaned_bottoms,
            per_utterance,
        }
    };

    // Report orphaned tops — only for indexed markers.
    // Unindexed multi-party overlaps are inherently ambiguous; see
    // docs/overlap-validation-audit.md (2026-03-19).
    for orphan in &analysis.orphaned_tops {
        if orphan.region.index.is_none() {
            continue;
        }
        let utt = &utterances[orphan.utterance_index];
        // Control-flow invariant: `is_none()` short-circuits the
        // continue above; reaching this line guarantees `Some(...)`.
        #[allow(clippy::unwrap_used)]
        let index_label = format!(" (index {})", orphan.region.index.unwrap().0);
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                SourceLocation::new(utt.main.span),
                ErrorContext::new(&orphan.speaker, utt.main.span, &orphan.speaker),
                format!(
                    "Top overlap ⌈{index_label} on speaker '{}' has no matching \
                     bottom overlap ⌊ from a different speaker",
                    orphan.speaker
                ),
            )
            .with_suggestion(
                "Check that the overlapping speaker's utterance has a matching ⌊ marker \
                 with the same index",
            ),
        );
    }

    // Report orphaned bottoms — only for indexed markers.
    for orphan in &analysis.orphaned_bottoms {
        if orphan.region.index.is_none() {
            continue;
        }
        let utt = &utterances[orphan.utterance_index];
        // Same control-flow invariant as the orphaned_tops loop above.
        #[allow(clippy::unwrap_used)]
        let index_label = format!(" (index {})", orphan.region.index.unwrap().0);
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                SourceLocation::new(utt.main.span),
                ErrorContext::new(&orphan.speaker, utt.main.span, &orphan.speaker),
                format!(
                    "Bottom overlap ⌊{index_label} on speaker '{}' has no matching \
                     top overlap ⌈ from a different speaker",
                    orphan.speaker
                ),
            )
            .with_suggestion(
                "Check that the other speaker's utterance has a matching ⌈ marker \
                 with the same index",
            ),
        );
    }
}
