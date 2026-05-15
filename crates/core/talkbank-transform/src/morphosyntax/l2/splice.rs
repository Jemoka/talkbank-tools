//! Splice merged L2 morphology back into a ChatFile.
//!
//! Overwrites `L2|xxx` MOR items with pre-mapped `Mor` items from the
//! structural merge algorithm, and optionally corrects GRA deprels.

use super::extract::L2DeferredPosition;
use super::merge::MergedL2Morphology;
use super::plan::{L2Attachment, L2RootAnchor};
use talkbank_model::alignment::MorItemIndex;

/// Outcome of splicing L2 results into a `ChatFile`.
#[derive(Debug, Default)]
pub struct SpliceOutcome {
    /// Number of @s positions successfully spliced with real morphology.
    pub spliced: usize,
    /// Number of @s positions that fell back to L2|xxx (no secondary result).
    pub fallback: usize,
    /// Number of GRA deprels corrected.
    pub gra_upgraded: usize,
}

/// Reason an L2 splice rolled back to `L2|xxx` for one or more
/// positions. Each variant doubles as a TODO bucket: a category is
/// a candidate for smarter merge logic that would *recover*
/// secondary morphology instead of falling back to `L2|xxx`.
/// `Display` emits the lower-case snake_case tag used in
/// `tracing::warn!` `category` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpliceFallbackCategory {
    SecondaryMultiRoot,
    SecondaryNoRoot,
    SecondaryCycle,
    SecondaryHeadOob,
    SecondaryChunkCountMismatch,
    SpliceInvariantOther,
}

impl SpliceFallbackCategory {
    /// Classify the splice's post-validation error into a fallback
    /// bucket. Adding a new variant to `MappingError` should add a
    /// new arm here.
    fn from_mapping_error(err: &crate::morphosyntax::MappingError) -> Self {
        use crate::morphosyntax::MappingError;
        match err {
            MappingError::InvalidRoot { details } if details.contains("multiple") => {
                Self::SecondaryMultiRoot
            }
            MappingError::InvalidRoot { .. } => Self::SecondaryNoRoot,
            MappingError::CircularDependency { .. } => Self::SecondaryCycle,
            MappingError::InvalidHeadReference { .. } => Self::SecondaryHeadOob,
            MappingError::ChunkCountMismatch { .. } => Self::SecondaryChunkCountMismatch,
            _ => Self::SpliceInvariantOther,
        }
    }
}

impl std::fmt::Display for SpliceFallbackCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::SecondaryMultiRoot => "secondary_multi_root",
            Self::SecondaryNoRoot => "secondary_no_root",
            Self::SecondaryCycle => "secondary_cycle",
            Self::SecondaryHeadOob => "secondary_head_oob",
            Self::SecondaryChunkCountMismatch => "secondary_chunk_count_mismatch",
            Self::SpliceInvariantOther => "splice_invariant_other",
        };
        f.write_str(s)
    }
}

/// Render a slice of `GrammaticalRelation` as the `%gra` body
/// (`index|head|relation` per relation, space-separated). Used in
/// fallback `tracing::warn!` messages on the rollback path.
fn join_relations(relations: &[talkbank_model::model::GrammaticalRelation]) -> String {
    relations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Outcome of the post-splice invariant gate. The single-position
/// and multi-position splice branches both update their `outcome`
/// counters off this result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpliceValidationResult {
    /// Splice output passed `validate_generated_gra`; commit.
    Valid,
    /// Splice output failed validation; tiers were restored from the
    /// snapshots and the affected words were reset to `L2|xxx`.
    RolledBack,
}

/// Position descriptor for a splice fallback's `tracing::warn!`
/// payload. Single-position spans report `word_idx`; multi-position
/// contiguous spans report the inclusive word-index range start
/// + size, matching the shape of `splice_range_coordinated`'s
/// `item_range` argument.
#[derive(Debug, Clone, Copy)]
enum SplicePositionDescriptor {
    SingleWord { word_idx: usize },
    Span { range_start: usize, size: usize },
}

/// Bundle of warn-fields that don't depend on the splice outcome.
/// Keeps the `validate_or_rollback_splice` signature compact.
struct SpliceFallbackContext<'a> {
    line_idx: usize,
    target_lang: &'a talkbank_model::model::LanguageCode,
    /// Word indices in the host utterance that get reset to `L2|xxx`
    /// when the splice rolls back. For single-position splices this
    /// is a one-element slice; for multi-position contiguous spans,
    /// every word in the span.
    word_indices_to_reset: &'a [usize],
    position: SplicePositionDescriptor,
}

/// Validate the post-splice gra against the structural invariants
/// (`validate_generated_gra`); on failure, restore the pre-splice
/// snapshots, reset the affected `word_indices_to_reset` to
/// `L2|xxx`, and emit a categorized `tracing::warn!`.
///
/// The `secondary_gras_summary` closure is only invoked on the
/// rollback path so the success path pays no diagnostic-building
/// cost. Callers update their `outcome.spliced` / `outcome.fallback`
/// counters off the returned [`SpliceValidationResult`].
///
/// This is the single chokepoint for the "no invalid CHAT shipped
/// downstream" guarantee: any future change to the rollback shape
/// (categories, warn fields, fallback policy) lands here once
/// instead of being mirrored in both branches.
fn validate_or_rollback_splice(
    mor: &mut talkbank_model::model::dependent_tier::MorTier,
    gra: &mut talkbank_model::model::dependent_tier::GraTier,
    mor_snapshot: talkbank_model::model::dependent_tier::MorTier,
    gra_snapshot: talkbank_model::model::dependent_tier::GraTier,
    secondary_gras_summary: impl FnOnce() -> Vec<String>,
    ctx: SpliceFallbackContext<'_>,
) -> SpliceValidationResult {
    let Err(invariant_err) =
        crate::morphosyntax::gra_validate::validate_generated_gra(gra.relations())
    else {
        return SpliceValidationResult::Valid;
    };

    let category = SpliceFallbackCategory::from_mapping_error(&invariant_err);
    let pre_splice_gra = join_relations(gra_snapshot.relations());
    let post_splice_gra = join_relations(gra.relations());
    let secondary_gras = secondary_gras_summary();

    *mor = mor_snapshot;
    *gra = gra_snapshot;
    for &word_idx in ctx.word_indices_to_reset {
        if let Some(mor_item) = mor.items_mut().get_mut(word_idx) {
            mor_item.main.reset_to_l2_placeholder();
        }
    }

    match ctx.position {
        SplicePositionDescriptor::SingleWord { word_idx } => {
            tracing::warn!(
                line_idx = ctx.line_idx,
                word_idx,
                target_lang = %ctx.target_lang,
                category = %category,
                invariant_error = %invariant_err,
                secondary_gras = ?secondary_gras,
                host_pre_splice_gra = %pre_splice_gra,
                host_post_splice_gra = %post_splice_gra,
                "L2 splice fell back to L2|xxx because secondary input \
                 would have produced invalid CHAT (post-splice gra fails \
                 structural invariants); see this warning's category for \
                 the smarter-merge TODO bucket"
            );
        }
        SplicePositionDescriptor::Span { range_start, size } => {
            tracing::warn!(
                line_idx = ctx.line_idx,
                span_word_start = range_start,
                span_size = size,
                target_lang = %ctx.target_lang,
                category = %category,
                invariant_error = %invariant_err,
                secondary_gras_per_position = ?secondary_gras,
                host_pre_splice_gra = %pre_splice_gra,
                host_post_splice_gra = %post_splice_gra,
                "L2 multi-position splice fell back to L2|xxx for the \
                 whole span because secondary input would have produced \
                 invalid CHAT (post-splice gra fails structural \
                 invariants); see this warning's category for the \
                 smarter-merge TODO bucket"
            );
        }
    }

    SpliceValidationResult::RolledBack
}

/// Slice-based version: collect 0-indexed positions in `gras` whose
/// relation is the strict splice-contract root, offset by
/// `local_chunk_offset`. Used by both the splice loop (post-repair,
/// where we have the gras slice but not a `MergedL2Morphology`) and
/// by [`root_offsets_for_merged`].
fn root_offsets_in_gras(
    gras: &[talkbank_model::model::dependent_tier::GrammaticalRelation],
    local_chunk_offset: usize,
) -> Vec<usize> {
    gras.iter()
        .enumerate()
        .filter(|(_, rel)| rel.head == 0 && rel.relation.eq_ignore_ascii_case("ROOT"))
        .map(|(idx, _)| local_chunk_offset + idx)
        .collect()
}

/// Slice-based version: pair each strict-root position in `gras`
/// with the deprel `apply_safe_root_rewrites` should write at that
/// position. Empty when `attachment` is not [`L2Attachment::ExternalRoot`].
fn root_rewrites_for_attachment(
    gras: &[talkbank_model::model::dependent_tier::GrammaticalRelation],
    attachment: &L2Attachment,
    local_chunk_offset: usize,
) -> Vec<(usize, crate::morphosyntax::l2::deprel::UdDeprel)> {
    let Some(rewrite) = attachment.external_root_deprel().cloned() else {
        return Vec::new();
    };
    root_offsets_in_gras(gras, local_chunk_offset)
        .into_iter()
        .map(|idx| (idx, rewrite.clone()))
        .collect()
}

fn current_root_anchor_for_attachment(
    mor: &talkbank_model::model::MorTier,
    gra: &talkbank_model::model::GraTier,
    deferred: &[L2DeferredPosition],
    attachment: &L2Attachment,
) -> Option<usize> {
    match attachment {
        L2Attachment::InternalRoot => None,
        L2Attachment::ExternalRoot {
            root_anchor: L2RootAnchor::UtteranceRoot { .. },
            ..
        } => Some(0),
        L2Attachment::ExternalRoot {
            root_anchor:
                L2RootAnchor::HostGovernor {
                    source_deferred_index,
                },
            ..
        } => {
            let current = deferred.get(*source_deferred_index)?;
            mor.governing_head_for_item(gra, MorItemIndex::new(current.word_idx))
                .ok()
                .and_then(|head_ref| head_ref.word())
                .map(|index| index.as_usize())
        }
    }
}

fn anchor_depends_on_replaced_range(
    gra: &talkbank_model::model::GraTier,
    anchor: usize,
    replaced_start: usize,
    replaced_end: usize,
) -> bool {
    let mut current = anchor;
    let mut seen = std::collections::HashSet::new();

    while current > 0 && seen.insert(current) {
        if current >= replaced_start && current <= replaced_end {
            return true;
        }

        let Some(rel) = gra.relations().iter().find(|rel| rel.index == current) else {
            return false;
        };

        if rel.head == 0 || rel.head == current {
            return false;
        }

        current = rel.head;
    }

    false
}

fn safe_root_anchor_override(
    gra: &talkbank_model::model::GraTier,
    chunk_offset: usize,
    old_chunks: usize,
    new_chunks: usize,
    root_offsets: &[usize],
    candidate_anchor: Option<usize>,
) -> Option<usize> {
    let anchor = candidate_anchor?;
    let final_len = gra.len().saturating_sub(old_chunks) + new_chunks;
    if anchor > final_len {
        return None;
    }
    if root_offsets
        .iter()
        .any(|local_idx| anchor == chunk_offset + local_idx + 1)
    {
        return None;
    }

    let replaced_start = chunk_offset + 1;
    let replaced_end = chunk_offset + old_chunks;
    if anchor_depends_on_replaced_range(gra, anchor, replaced_start, replaced_end) {
        return None;
    }

    Some(anchor)
}

fn apply_safe_root_rewrites(
    gra: &mut talkbank_model::model::GraTier,
    chunk_offset: usize,
    root_rewrites: &[(usize, crate::morphosyntax::l2::deprel::UdDeprel)],
) {
    for (local_idx, deprel) in root_rewrites {
        if let Some(rel) = gra.relations_mut().get_mut(chunk_offset + local_idx) {
            if rel.head != 0 && rel.head != rel.index {
                rel.relation = deprel.to_chat_gra().into();
            }
        }
    }
}

/// Detect and repair the post-splice `secondary_multi_root` shape:
/// the L2 plan picked `UtteranceRoot` (so `splice_coordinated` kept
/// `head=0/ROOT` for the L2 position) but the host's pre-splice gra
/// already had a different `head=0/ROOT`, leaving two roots in the
/// post-splice gra. The host's structure is canonical; demote the
/// L2 contribution to attach to the host's root with a generic
/// `dep` relation.
///
/// Operates on the WHOLE gra. `l2_chunk_offset` and `l2_chunk_count`
/// identify the L2 span's chunk range; any `head=0/ROOT` inside that
/// range is the L2 contribution. Any `head=0/ROOT` outside is the
/// host's pre-existing root.
///
/// This catches the 43-rollbacks-per-750-file `secondary_multi_root`
/// variant that the merge-stage `repair_secondary_gras` couldn't
/// address (it only sees the span, not the surrounding host gra).
fn demote_duplicate_l2_root(
    gra: &mut talkbank_model::model::GraTier,
    l2_chunk_offset: usize,
    l2_chunk_count: usize,
) {
    let l2_range = l2_chunk_offset..(l2_chunk_offset + l2_chunk_count);
    let root_indices: Vec<usize> = gra
        .relations()
        .iter()
        .enumerate()
        .filter(|(_, r)| r.head == 0 && r.relation.eq_ignore_ascii_case("ROOT"))
        .map(|(i, _)| i)
        .collect();
    if root_indices.len() <= 1 {
        return;
    }
    let host_root_position = root_indices
        .iter()
        .find(|&&i| !l2_range.contains(&i))
        .map(|&i| gra.relations()[i].index);
    let Some(host_root_position) = host_root_position else {
        // All roots are inside the L2 span; constructive-merge's
        // `repair_secondary_gras` is responsible for that case.
        return;
    };
    for &i in root_indices.iter().filter(|&&i| l2_range.contains(&i)) {
        let rel = &mut gra.relations_mut()[i];
        rel.head = host_root_position;
        rel.relation = "DEP".into();
    }
}

/// Overwrite `L2|xxx` MOR items with merged morphology.
///
/// Each `MergedL2Morphology` contains a fully-mapped `Mor` item (produced
/// by `map_ud_sentence` which handles MWT contractions, then POS-overridden
/// by the merge algorithm).
///
/// **Multi-position spans (2026-05-03 fix).** When a contiguous run of
/// `@s` positions on the same line, in the same target language, was
/// dispatched as one secondary Stanza sentence (mirroring the grouping
/// in `super::spans::group_deferred_into_dispatch_spans`), the gras
/// inside each per-position [`MergedL2Morphology`] use heads in
/// SPAN-RELATIVE chunk space — including cross-position references
/// (e.g. `la@s fecha@s bien@s` produces `la → fecha (head=2)` and
/// `bien → fecha (head=2)`). Splicing each position with the
/// per-item [`splice_coordinated`] misclassifies these as within-MWT
/// and remaps them with the wrong `chunk_offset`, yielding the E722 +
/// E724 cascade documented in
/// `docs/postmortems/2026-05-03-l2-splice-cardinality-investigation.md`
/// §6c. To fix this, we group consecutive same-line same-language
/// positions here and apply [`splice_range_coordinated`] to each
/// multi-position span atomically. Single-position spans still use
/// the existing [`splice_coordinated`] path so the (passing) MWT
/// behavior is untouched.
///
/// This function must be called AFTER `inject_results` has set L2|xxx on
/// all @s positions.
pub fn splice_l2_into_chat(
    chat_file: &mut talkbank_model::model::ChatFile,
    deferred: &[L2DeferredPosition],
    merged_results: &[Option<MergedL2Morphology>],
) -> SpliceOutcome {
    use talkbank_model::model::DependentTier;
    use talkbank_model::model::Line;

    let mut outcome = SpliceOutcome::default();

    // Walk deferred positions, grouping into contiguous spans that match
    // the dispatch grouping (same `line_idx`, same `target_lang`,
    // consecutive `word_idx`). The splice algorithm then differs by span
    // size: 1 → existing per-item splice; >1 → atomic range splice that
    // preserves cross-position head references.
    let mut i = 0;
    while i < deferred.len() {
        // Find the end of the contiguous span starting at i.
        let mut j = i + 1;
        while j < deferred.len()
            && deferred[j].line_idx == deferred[i].line_idx
            && deferred[j].target_lang == deferred[i].target_lang
            && deferred[j].word_idx == deferred[j - 1].word_idx + 1
        {
            j += 1;
        }
        let span_indices = i..j;
        i = j;

        // Any position in the span without a merged result falls back
        // (no secondary inference for that target language). We process
        // the span only if EVERY position has a merged result; otherwise
        // each missing position is fallback and present positions go
        // through the per-item path so partial spans still get value.
        let all_present = span_indices.clone().all(|k| merged_results[k].is_some());

        if !all_present {
            for k in span_indices {
                if merged_results[k].is_none() {
                    outcome.fallback += 1;
                } else {
                    splice_one_position(
                        chat_file,
                        deferred,
                        &deferred[k],
                        merged_results[k].as_ref().expect("checked Some"),
                        &mut outcome,
                    );
                }
            }
            continue;
        }

        // All positions have merged results.
        let span_size = span_indices.end - span_indices.start;
        if span_size == 1 {
            let k = span_indices.start;
            splice_one_position(
                chat_file,
                deferred,
                &deferred[k],
                merged_results[k].as_ref().expect("checked Some above"),
                &mut outcome,
            );
            continue;
        }

        // Multi-position span: aggregate per-position mors+gras and
        // splice atomically.
        let line_idx = deferred[span_indices.start].line_idx;
        let utt = match &mut chat_file.lines[line_idx] {
            Line::Utterance(u) => u,
            _ => {
                outcome.fallback += span_size;
                continue;
            }
        };
        let mut mor_tier_ref = None;
        let mut gra_tier_ref = None;
        for tier in &mut utt.dependent_tiers {
            match tier {
                DependentTier::Mor(m) => mor_tier_ref = Some(m),
                DependentTier::Gra(g) => gra_tier_ref = Some(g),
                _ => {}
            }
        }

        let mor = match mor_tier_ref {
            Some(m) => m,
            None => {
                outcome.fallback += span_size;
                continue;
            }
        };

        // Aggregate mors and gras across the span. Each
        // MergedL2Morphology's gras are span-relative-chunk-1-indexed
        // by construction (sliced from map_ud_sentence's per-chunk
        // output for this same span), so concatenating them produces
        // a within-block-relative gra sequence — exactly the contract
        // splice_range_coordinated expects.
        let mut new_mors: Vec<talkbank_model::model::dependent_tier::mor::Mor> =
            Vec::with_capacity(span_size);
        let mut new_gras: Vec<talkbank_model::model::dependent_tier::GrammaticalRelation> =
            Vec::new();
        let mut any_corrected_deprel = false;
        let mut local_chunk_offset = 0usize;
        for k in span_indices.clone() {
            let merged = merged_results[k].as_ref().expect("all_present checked");
            new_mors.push(merged.mor.clone());
            new_gras.extend(merged.gras.iter().cloned());
            if merged.corrected_deprel.is_some() {
                any_corrected_deprel = true;
            }
            local_chunk_offset += merged.mor.count_chunks();
        }

        let item_range = deferred[span_indices.start].word_idx
            ..deferred[span_indices.start].word_idx + span_size;

        if let Some(gra) = gra_tier_ref {
            let chunk_offset: usize = mor.items()[..item_range.start]
                .iter()
                .map(|m| m.count_chunks())
                .sum();
            let old_chunks: usize = mor.items()[item_range.clone()]
                .iter()
                .map(|m| m.count_chunks())
                .sum();
            // Constructive repair on aggregated span-level gras. The
            // bounds for OOB clamp are span-total chunks
            // (`local_chunk_offset` after the per-position loop above);
            // cross-position head references like `head=2` for chunk 1
            // pointing at chunk 2 of the span are valid here, not OOB.
            // See `repair_secondary_gras` in merge.rs and the §6
            // walkthrough in `docs/architecture/l2-morphotag-redesign-2026-05-07.md`.
            let span_attachment = merged_results[span_indices.start]
                .as_ref()
                .map(|m| m.attachment.clone())
                .unwrap_or(L2Attachment::InternalRoot);
            let root_offsets = crate::morphosyntax::l2::merge::repair_secondary_gras(
                &mut new_gras,
                &span_attachment,
            );
            // Post-repair root-rewrites for `apply_safe_root_rewrites`
            // below: at-most-one strict root times the span
            // attachment's deprel (empty when InternalRoot).
            let root_rewrites: Vec<(usize, crate::morphosyntax::l2::deprel::UdDeprel)> =
                match span_attachment.external_root_deprel() {
                    Some(deprel) => root_offsets
                        .iter()
                        .map(|&idx| (idx, deprel.clone()))
                        .collect(),
                    None => Vec::new(),
                };
            let safe_anchor = safe_root_anchor_override(
                gra,
                chunk_offset,
                old_chunks,
                local_chunk_offset,
                &root_offsets,
                merged_results[span_indices.start]
                    .as_ref()
                    .and_then(|merged| {
                        current_root_anchor_for_attachment(mor, gra, deferred, &merged.attachment)
                    }),
            );

            // Whole-tier snapshot for rollback. `splice_range_coordinated`
            // mutates `.head` and `.index` fields across the ENTIRE host
            // gra (not just the spliced range), so slice-scoped restore
            // is not structurally sufficient. Cost analysis +
            // alternatives in `docs/l2-splice-snapshot-cost-analysis.md`.
            let mor_snapshot = mor.clone();
            let gra_snapshot = gra.clone();

            match mor.splice_range_coordinated(
                gra,
                item_range.clone(),
                new_mors,
                new_gras,
                safe_anchor,
            ) {
                Ok(()) => {
                    apply_safe_root_rewrites(gra, chunk_offset, &root_rewrites);
                    demote_duplicate_l2_root(gra, chunk_offset, local_chunk_offset);

                    let word_indices: Vec<usize> =
                        span_indices.clone().map(|k| deferred[k].word_idx).collect();
                    let merged_for_summary = merged_results;
                    let span_indices_for_summary = span_indices.clone();
                    match validate_or_rollback_splice(
                        mor,
                        gra,
                        mor_snapshot,
                        gra_snapshot,
                        || {
                            span_indices_for_summary
                                .map(|k| {
                                    merged_for_summary[k]
                                        .as_ref()
                                        .map(|m| join_relations(&m.gras))
                                        .unwrap_or_default()
                                })
                                .collect()
                        },
                        SpliceFallbackContext {
                            line_idx,
                            target_lang: &deferred[span_indices.start].target_lang,
                            word_indices_to_reset: &word_indices,
                            position: SplicePositionDescriptor::Span {
                                range_start: item_range.start,
                                size: span_size,
                            },
                        },
                    ) {
                        SpliceValidationResult::Valid => {
                            outcome.spliced += span_size;
                            if any_corrected_deprel {
                                outcome.gra_upgraded += 1;
                            }
                        }
                        SpliceValidationResult::RolledBack => {
                            outcome.fallback += span_size;
                        }
                    }
                }
                Err(_) => {
                    outcome.fallback += span_size;
                }
            }
        } else {
            // No %gra tier: replace mor items in place, span-level. The
            // single-position fallback already supports this; do the
            // equivalent for the range here.
            let start = deferred[span_indices.start].word_idx;
            for (offset, mor_item) in new_mors.into_iter().enumerate() {
                if let Some(slot) = mor.items_mut().get_mut(start + offset) {
                    *slot = mor_item;
                    outcome.spliced += 1;
                } else {
                    outcome.fallback += 1;
                }
            }
        }
    }

    outcome
}

/// Splice ONE merged L2 result into a host utterance. Extracted so the
/// single-position path and the partial-span-fallback path share code.
fn splice_one_position(
    chat_file: &mut talkbank_model::model::ChatFile,
    deferred: &[L2DeferredPosition],
    def: &L2DeferredPosition,
    merged: &MergedL2Morphology,
    outcome: &mut SpliceOutcome,
) {
    use talkbank_model::model::DependentTier;
    use talkbank_model::model::Line;

    let utt = match &mut chat_file.lines[def.line_idx] {
        Line::Utterance(u) => u,
        _ => {
            outcome.fallback += 1;
            return;
        }
    };

    let mut mor_tier = None;
    let mut gra_tier = None;
    for tier in &mut utt.dependent_tiers {
        match tier {
            DependentTier::Mor(m) => mor_tier = Some(m),
            DependentTier::Gra(g) => gra_tier = Some(g),
            _ => {}
        }
    }

    if let Some(mor) = mor_tier {
        if let Some(gra) = gra_tier {
            let chunk_offset: usize = mor.items()[..def.word_idx]
                .iter()
                .map(|m| m.count_chunks())
                .sum();
            let old_chunks = mor.items()[def.word_idx].count_chunks();
            // Constructive repair: a single-position span IS its own
            // span; gras.len() bounds head indices and the
            // attachment dictates whether one head=0/ROOT must exist.
            // See `repair_secondary_gras` in merge.rs.
            let mut repaired_gras = merged.gras.clone();
            let root_offsets = crate::morphosyntax::l2::merge::repair_secondary_gras(
                &mut repaired_gras,
                &merged.attachment,
            );
            let safe_anchor = safe_root_anchor_override(
                gra,
                chunk_offset,
                old_chunks,
                merged.mor.count_chunks(),
                &root_offsets,
                merged
                    .attachment
                    .is_external_root()
                    .then(|| {
                        current_root_anchor_for_attachment(mor, gra, deferred, &merged.attachment)
                    })
                    .flatten(),
            );
            // Whole-tier snapshot for rollback. `splice_coordinated`
            // mutates `.head` and `.index` fields across the ENTIRE host
            // gra (not just the spliced item), so slice-scoped restore
            // is not structurally sufficient. Cost analysis +
            // alternatives in `docs/l2-splice-snapshot-cost-analysis.md`.
            let mor_snapshot = mor.clone();
            let gra_snapshot = gra.clone();

            if mor
                .splice_coordinated(
                    gra,
                    def.word_idx,
                    merged.mor.clone(),
                    repaired_gras.clone(),
                    safe_anchor,
                )
                .is_ok()
            {
                let root_rewrites =
                    root_rewrites_for_attachment(&repaired_gras, &merged.attachment, 0);
                apply_safe_root_rewrites(gra, chunk_offset, &root_rewrites);
                demote_duplicate_l2_root(gra, chunk_offset, merged.mor.count_chunks());

                let word_indices = [def.word_idx];
                match validate_or_rollback_splice(
                    mor,
                    gra,
                    mor_snapshot,
                    gra_snapshot,
                    || vec![join_relations(&repaired_gras)],
                    SpliceFallbackContext {
                        line_idx: def.line_idx,
                        target_lang: &def.target_lang,
                        word_indices_to_reset: &word_indices,
                        position: SplicePositionDescriptor::SingleWord {
                            word_idx: def.word_idx,
                        },
                    },
                ) {
                    SpliceValidationResult::Valid => {
                        outcome.spliced += 1;
                        if merged.corrected_deprel.is_some() {
                            outcome.gra_upgraded += 1;
                        }
                    }
                    SpliceValidationResult::RolledBack => {
                        outcome.fallback += 1;
                    }
                }
            } else {
                outcome.fallback += 1;
            }
        } else if let Some(mor_item) = mor.items_mut().get_mut(def.word_idx) {
            *mor_item = merged.mor.clone();
            outcome.spliced += 1;
        } else {
            outcome.fallback += 1;
        }
    } else {
        outcome.fallback += 1;
    }
}

#[cfg(test)]
mod cardinality_tests {
    use super::*;
    use crate::morphosyntax::l2::extract::{L2DeferredPosition, PrimaryStructuralInfo};
    use crate::morphosyntax::l2::merge::MergedL2Morphology;
    use crate::parse::parse_lenient;
    use talkbank_model::ParseValidateOptions;
    use talkbank_model::model::LanguageCode;
    use talkbank_model::model::dependent_tier::GrammaticalRelation;
    use talkbank_model::model::dependent_tier::mor::{Mor, MorStem, MorWord, PosCategory};
    use talkbank_parser::TreeSitterParser;

    /// Splice replaces a 1-chunk `L2|xxx` slot with an N-chunk merged Mor.
    /// The output `%mor` chunk count grows but `%gra` count stays the same;
    /// the resulting ChatFile must still validate.
    ///
    /// Fixture note (2026-05-06): the original fixture was
    /// `*PAR: yellow@s .` — a whole-utterance `@s` pattern that
    /// validator E255 (BUG-023, GREEN 2026-05-05) correctly rejects
    /// (whole-utterance language switches must use `[- LANG]` precode,
    /// not per-word `@s`). The test now embeds the L2 word among native
    /// French words so the fixture validates pre-splice while still
    /// exercising the multi-chunk MWT splice path.
    #[test]
    fn multi_chunk_merged_mor_keeps_chat_valid() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, ara\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tvoici yellow@s .\n\
                         %mor:\tintj|voici L2|xxx .\n\
                         %gra:\t1|2|DISCOURSE 2|0|ROOT 3|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);

        let mut precondition = chat_file.clone();
        let opts = ParseValidateOptions::default().with_alignment();
        assert!(
            talkbank_model::validate_chat_file_with_options(&mut precondition, &opts).is_ok(),
            "fixture precondition: input must validate before splice",
        );

        let merged_mor = Mor::new(MorWord::new(PosCategory::new("verb"), MorStem::new("yel")))
            .with_post_clitic(MorWord::new(PosCategory::new("part"), MorStem::new("lo")));

        let mut utt_idx = None;
        for (i, line) in chat_file.lines.iter().enumerate() {
            if let talkbank_model::model::Line::Utterance(_) = line {
                utt_idx = Some(i);
                break;
            }
        }
        let line_idx = utt_idx.expect("utterance present");

        // The L2 placeholder is at word_idx 1 (after "voici" at idx 0).
        // Host primary said the L2 word is utterance root (head=0,
        // deprel=root) — chunk 2 in the host gra.
        let deferred = vec![L2DeferredPosition {
            line_idx,
            word_idx: 1,
            target_lang: LanguageCode::new("ara"),
            primary: PrimaryStructuralInfo {
                deprel: crate::morphosyntax::l2::deprel::UdDeprel::new("root"),
                upos: None,
                head: 0,
                dependent_deprels: Vec::new(),
                head_upos: None,
            },
        }];
        let merged = vec![Some(MergedL2Morphology {
            mor: merged_mor,
            gras: vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 1, "DEP"),
            ],
            corrected_deprel: None,
            attachment: L2Attachment::InternalRoot,
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 1,
            "splice must report success for the slot"
        );
        assert_eq!(outcome.fallback, 0);

        validate_morphosyntax(&mut chat_file);
    }

    /// Helper: build a one-utterance ChatFile with N L2 placeholders.
    /// Returns (ChatFile, line_idx). The %mor and %gra are pre-validated
    /// to ensure the precondition is met before splicing.
    fn build_l2_fixture(
        languages_header: &str,
        word_text: &str,
        n_l2_words: usize,
    ) -> (talkbank_model::model::ChatFile, usize) {
        // %mor: one `L2|xxx` per word, then terminator.
        let mor_line = (0..n_l2_words)
            .map(|_| "L2|xxx")
            .collect::<Vec<_>>()
            .join(" ")
            + " .";
        // %gra: one ROOT then n_l2_words-1 DEPs back to root, then PUNCT.
        // For n=1: `1|0|ROOT 2|1|PUNCT`
        // For n=3: `1|0|ROOT 2|1|DEP 3|1|DEP 4|1|PUNCT`
        let mut gra_parts: Vec<String> = Vec::with_capacity(n_l2_words + 1);
        gra_parts.push("1|0|ROOT".to_string());
        for i in 2..=n_l2_words {
            gra_parts.push(format!("{}|1|DEP", i));
        }
        gra_parts.push(format!("{}|1|PUNCT", n_l2_words + 1));
        let gra_line = gra_parts.join(" ");

        let chat_text = format!(
            "@UTF8\n\
             @Begin\n\
             @Languages:\t{lang}\n\
             @Participants:\tPAR Participant\n\
             @ID:\t{lang_first}|test|PAR|||||Participant|||\n\
             *PAR:\t{words} .\n\
             %mor:\t{mor}\n\
             %gra:\t{gra}\n\
             @End\n",
            lang = languages_header,
            lang_first = languages_header.split(',').next().unwrap().trim(),
            words = word_text,
            mor = mor_line,
            gra = gra_line,
        );
        let parser = TreeSitterParser::new().unwrap();
        let (chat_file, _errors) = parse_lenient(&parser, &chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .expect("fixture has an utterance");
        (chat_file, line_idx)
    }

    /// Helper: construct a deferred position with no head/no head_upos
    /// other than what the caller provides. Models the primary analysis
    /// of a single `@s` word.
    fn deferred_position(
        line_idx: usize,
        word_idx: usize,
        target_lang: &str,
        primary_deprel: &str,
        primary_head: usize,
    ) -> L2DeferredPosition {
        L2DeferredPosition {
            line_idx,
            word_idx,
            target_lang: LanguageCode::new(target_lang),
            primary: PrimaryStructuralInfo {
                deprel: crate::morphosyntax::l2::deprel::UdDeprel::new(primary_deprel),
                upos: None,
                head: primary_head,
                dependent_deprels: Vec::new(),
                head_upos: None,
            },
        }
    }

    /// **RED test 1** (l2.md §6 / postmortem §6 Step 1): three contiguous
    /// `@s` words in one utterance — the minimal multi-position L2 span.
    /// Mirrors the wild bad-case shape from
    /// `~/0tb/data/biling-data/Bangor/Patagonia/07.cha:394`
    /// (`la@s fecha@s bien@s`). Stanza secondary returns one sentence
    /// covering all three with cross-position heads. Per-position splicing
    /// of the resulting per-position gras is expected to fail because
    /// `splice_coordinated`'s "internal reference within new block" branch
    /// remaps heads with the WRONG position's `chunk_offset`.
    #[test]
    fn multi_position_mwt_in_one_utterance() {
        let (mut chat_file, line_idx) = build_l2_fixture("spa, eng", "la fecha bien", 3);

        // Three contiguous L2 positions, primary analysis from (host) Welsh
        // says all three head to position 1 (the conventional ROOT in the
        // fixture's %gra). Targeting Spanish secondary.
        let deferred = vec![
            deferred_position(line_idx, 0, "spa", "det", 0),
            deferred_position(line_idx, 1, "spa", "root", 0),
            deferred_position(line_idx, 2, "spa", "advmod", 0),
        ];

        // Per-position merged_results, the way `dispatch_secondary_l2`
        // currently slices `gra_relations` from the secondary Stanza call.
        // Heads use SECONDARY-SENTENCE 1-indexed values: la→head=2 means
        // "fecha" (the second word in the secondary sentence), bien→head=2
        // means "fecha" too.
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("det"), MorStem::new("la"))),
                gras: vec![GrammaticalRelation::new(1, 2, "DET")],
                corrected_deprel: None,
                attachment: L2Attachment::InternalRoot,
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(
                    PosCategory::new("noun"),
                    MorStem::new("fecha"),
                )),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: L2Attachment::InternalRoot,
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("adv"), MorStem::new("bien"))),
                gras: vec![GrammaticalRelation::new(1, 2, "ADVMOD")],
                corrected_deprel: None,
                attachment: L2Attachment::InternalRoot,
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 3, "all three positions must splice");
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);
    }

    /// **RED test 2** — one `@s` word that Stanza expands to 3 chunks
    /// (verb + two clitics, e.g. `verb|x~part|y~part|z`). Same cardinality
    /// assertion as the existing 1→2 chunk test. Catches off-by-one
    /// breakage in `splice_coordinated`'s chunk-count delta arithmetic
    /// when delta = +2 instead of the +1 the existing test exercises.
    #[test]
    fn multi_clitic_mwt() {
        let (mut chat_file, line_idx) = build_l2_fixture("fra, ara", "verb", 1);

        let merged_mor = Mor::new(MorWord::new(PosCategory::new("verb"), MorStem::new("v")))
            .with_post_clitic(MorWord::new(PosCategory::new("part"), MorStem::new("c1")))
            .with_post_clitic(MorWord::new(PosCategory::new("part"), MorStem::new("c2")));

        let deferred = vec![deferred_position(line_idx, 0, "fra", "root", 0)];
        let merged = vec![Some(MergedL2Morphology {
            mor: merged_mor,
            gras: vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 1, "DEP"),
                GrammaticalRelation::new(3, 1, "DEP"),
            ],
            corrected_deprel: None,
            attachment: L2Attachment::InternalRoot,
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);
    }

    /// **RED test 3** — phrasal-verb particle: an `@s` word whose merge
    /// sets `corrected_deprel = Some("compound:prt")`. Verifies that
    /// (a) the splice still reports success and (b) the host's terminator
    /// gra is preserved (not silently overwritten by the corrected deprel
    /// path). The existing single-position test does not exercise the
    /// `corrected_deprel.is_some()` branch.
    #[test]
    fn phrasal_verb_particle_preserves_terminator_gra() {
        // Host: `wake up .` where `up@s` is the L2 particle. Primary says
        // "up" is advmod of "wake".
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, fra\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\twake up .\n\
                         %mor:\tverb|wake L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "fra", "advmod", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(PosCategory::new("part"), MorStem::new("up"))),
            gras: vec![GrammaticalRelation::new(1, 0, "COMPOUND-PRT")],
            corrected_deprel: Some(crate::morphosyntax::l2::deprel::UdDeprel::new(
                "compound:prt",
            )),
            attachment: host_attachment(0, "compound:prt"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.gra_upgraded, 1);
        validate_morphosyntax(&mut chat_file);

        // Explicit: terminator gra (originally `3|1|PUNCT`) must still be
        // a PUNCT relation pointing at the verb.
        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier must exist after splice");
        let last = gra_tier
            .relations()
            .last()
            .expect("terminator gra must remain");
        assert_eq!(
            last.relation.as_str().to_ascii_uppercase(),
            "PUNCT",
            "terminator gra deprel must still be PUNCT after splice"
        );
    }

    /// **RED test 4** — Italian range-override path: `del` is the MWT of
    /// `di` + `il` (prep + det). The fixture exercises the
    /// `try_handle_italian_range_override` codepath in `sentence_mapping`.
    /// Asserts cardinality is preserved post-splice.
    #[test]
    fn italian_range_override_preserves_cardinality() {
        let (mut chat_file, line_idx) = build_l2_fixture("ita, eng", "del", 1);

        let merged_mor = Mor::new(MorWord::new(PosCategory::new("prep"), MorStem::new("di")))
            .with_post_clitic(MorWord::new(PosCategory::new("det"), MorStem::new("il")));

        let deferred = vec![deferred_position(line_idx, 0, "ita", "root", 0)];
        let merged = vec![Some(MergedL2Morphology {
            mor: merged_mor,
            gras: vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 1, "DET"),
            ],
            corrected_deprel: None,
            attachment: L2Attachment::InternalRoot,
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);
    }

    /// **RED test 5** — end-to-end serialize / re-parse / re-validate.
    /// The internal `validate_chat_file_with_options` used by the other
    /// tests catches some but not all cardinality issues (per postmortem
    /// §4b: "the 235 wild files passed `validate_chat_file_with_options`
    /// at write time but fail `chatter validate` afterwards"). This test
    /// closes that gap by serializing the spliced ChatFile and re-validating
    /// the round-tripped form, which is what `chatter validate` itself does.
    #[test]
    fn chatter_validate_passes_after_splice() {
        use crate::serialize::to_chat_string;

        let (mut chat_file, line_idx) = build_l2_fixture("fra, ara", "yellow", 1);

        let merged_mor = Mor::new(MorWord::new(PosCategory::new("verb"), MorStem::new("yel")))
            .with_post_clitic(MorWord::new(PosCategory::new("part"), MorStem::new("lo")));
        let deferred = vec![deferred_position(line_idx, 0, "ara", "root", 0)];
        let merged = vec![Some(MergedL2Morphology {
            mor: merged_mor,
            gras: vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 1, "DEP"),
            ],
            corrected_deprel: None,
            attachment: L2Attachment::InternalRoot,
        })];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);

        // Round-trip: serialize → re-parse → validate the re-parsed file.
        let parser = TreeSitterParser::new().unwrap();
        let serialized = to_chat_string(&chat_file);
        let (mut reparsed, _errors) = parse_lenient(&parser, &serialized);
        validate_morphosyntax(&mut reparsed);
    }

    #[test]
    fn single_position_root_remap_does_not_leave_root_deprel_on_non_root_head() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign tail .\n\
                         %mor:\tverb|host L2|xxx noun|tail .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|OBJ 4|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "obj", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let foreign_rel = &gra_tier.relations()[1];
        assert_eq!(foreign_rel.head, 1);
        assert_ne!(
            foreign_rel.relation.as_str().to_ascii_uppercase(),
            "ROOT",
            "a remapped external dependency must not keep deprel ROOT"
        );
    }

    #[test]
    fn single_position_root_remap_does_not_create_two_cycle() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign .\n\
                         %mor:\tnoun|host L2|xxx .\n\
                         %gra:\t1|2|DEP 2|0|ROOT 3|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "case", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "case"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let host_rel = &gra_tier.relations()[0];
        let foreign_rel = &gra_tier.relations()[1];
        assert!(
            !(host_rel.head == foreign_rel.index && foreign_rel.head == host_rel.index),
            "root remap must not create a direct host<->foreign cycle"
        );
    }

    #[test]
    fn single_position_root_remap_does_not_self_anchor_non_root_relation() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign .\n\
                         %mor:\tverb|host L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "case", 2)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "case"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let foreign_rel = &gra_tier.relations()[1];
        assert_ne!(
            foreign_rel.head, foreign_rel.index,
            "root remap must not create a self-headed non-ROOT relation"
        );
    }

    #[test]
    fn single_position_root_remap_does_not_use_out_of_bounds_anchor() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign .\n\
                         %mor:\tverb|host L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "compound", 5)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "compound"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let foreign_rel = &gra_tier.relations()[1];
        assert!(
            foreign_rel.head <= gra_tier.relations().len(),
            "root remap must not emit a head outside the final %gra length"
        );
    }

    #[test]
    fn single_position_root_remap_uses_host_chunk_anchor_not_word_index() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\tit@s:eng foreign .\n\
                         %mor:\tpron|it~aux|be L2|xxx .\n\
                         %gra:\t1|2|EXPL 2|0|ROOT 3|2|DEP 4|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "obj", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let foreign_rel = &gra_tier.relations()[2];
        assert_eq!(
            foreign_rel.head, 2,
            "L2 root remap must target the host's governing chunk, not the raw word index"
        );
    }

    #[test]
    fn multiple_noncontiguous_single_word_l2_spans_preserve_single_root() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost uno bridge dos tail .\n\
                         %mor:\tverb|host L2|xxx noun|bridge L2|xxx noun|tail .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|OBJ 4|1|DEP 5|1|OBL 6|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![
            deferred_position(line_idx, 1, "spa", "obj", 1),
            deferred_position(line_idx, 3, "spa", "obl", 1),
        ];
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("uno"))),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: host_attachment(0, "obj"),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("dos"))),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: host_attachment(1, "obl"),
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 2);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        let root_count = gra_tier
            .relations()
            .iter()
            .filter(|rel| rel.head == 0)
            .count();
        assert_eq!(root_count, 1, "exactly one ROOT head must remain");
        let non_root_root_labels = gra_tier
            .relations()
            .iter()
            .filter(|rel| rel.head != 0 && rel.relation.as_str().eq_ignore_ascii_case("ROOT"))
            .count();
        assert_eq!(
            non_root_root_labels, 0,
            "non-contiguous L2 spans must not leave stray ROOT labels"
        );
    }

    #[test]
    fn multiword_span_with_later_root_source_preserves_that_root_anchor() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost uno dos .\n\
                         %mor:\tverb|host L2|xxx L2|xxx .\n\
                         %gra:\t1|3|DEP 2|3|DEP 3|0|ROOT 4|3|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![
            deferred_position(line_idx, 1, "spa", "dep", 3),
            deferred_position(line_idx, 2, "spa", "root", 0),
        ];
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("uno"))),
                gras: vec![GrammaticalRelation::new(1, 2, "DEP")],
                corrected_deprel: None,
                attachment: utterance_root_attachment(1),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("dos"))),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: utterance_root_attachment(1),
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 2);
        assert_eq!(outcome.fallback, 0);
        validate_morphosyntax(&mut chat_file);

        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("gra tier");
        assert_eq!(
            gra_tier.relations()[2].head,
            0,
            "when the later item in a contiguous L2 span is the root-bearing \
             source, the spliced root must stay anchored to utterance ROOT \
             instead of inheriting the first replaced item's old head"
        );
    }

    fn validate_morphosyntax(chat: &mut talkbank_model::model::ChatFile) {
        use talkbank_model::ParseValidateOptions;
        let opts = ParseValidateOptions::default().with_alignment();
        if let Err(e) = talkbank_model::validate_chat_file_with_options(chat, &opts) {
            panic!("Morphosyntax validation failed: {:#?}", e);
        }
    }

    // ========================================================================
    // Family B — L2 splice integrity (joint-invariant RED tests).
    //
    // Pure-unit pinning for the Family B partition; see the L2
    // architectural-reassessment notes (§5).
    //
    // Wild evidence (from a 2026-05-06 wild-corpus error classification
    // log):
    //
    // - sastre03.cha:843 `+" yo@s soy@s el@s lieutenant .` produces
    //   `%gra: 1|3|NSUBJ 2|3|COP 3|0|DET 4|1|FLAT 5|1|PUNCT` — chunk 3
    //   has head=0 with deprel="DET" instead of "ROOT" (E722).
    // - herring09.cha:2570 `... el@s camino@s .` produces
    //   `%gra: 1|6|CC 2|6|NSUBJ 3|6|AUX 4|6|COP 5|6|CASE 6|7|DET 7|0|NMOD 8|6|PUNCT`
    //   — chunk 7 has head=0 with deprel="NMOD" instead of "ROOT" (E722).
    // - sastre03.cha:2823 `al@s lado@s de@s Smith .` produces
    //   `%gra: 1|3|CASE 2|3|DET 3|0|NMOD 4|1|FIXED 5|1|FLAT 6|1|PUNCT`
    //   — chunk 3 has head=0 with deprel="NMOD" instead of "ROOT" (E722).
    //
    // The unifying invariant the splice must enforce after writing its
    // output to the host `%gra`:
    //
    //     For every relation r:
    //         (r.head == 0) ⟺ (r.relation.eq_ignore_ascii_case("ROOT"))
    //
    // BUG-025's `single_position_root_remap_does_not_leave_root_deprel_on_non_root_head`
    // covered the right-to-left direction (deprel="ROOT" but head ≠ 0).
    // The wild patterns above are the left-to-right direction
    // (head = 0 but deprel ≠ "ROOT") — symmetric and equally broken.
    // ========================================================================

    /// Walk a GraTier's relations and assert the joint root invariant
    /// holds. Reports the offending relation(s) with the full %gra body
    /// for debuggability. Use from any Family B test.
    fn assert_joint_root_invariant(
        chat: &talkbank_model::model::ChatFile,
        line_idx: usize,
        scenario_label: &str,
    ) {
        let utt = match &chat.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => panic!("expected Line::Utterance at idx {line_idx}"),
        };
        let gra_tier = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("utterance must have %gra after splice");
        let body: String = join_relations(gra_tier.relations());

        let mut head_zero_count = 0usize;
        for rel in gra_tier.relations() {
            let head_zero = rel.head == 0;
            let labelled_root = rel.relation.as_str().eq_ignore_ascii_case("ROOT");
            assert!(
                head_zero == labelled_root,
                "Family B joint invariant violated [{scenario_label}]: \
                 chunk {} has head={} relation={:?} — head=0 must pair \
                 EXACTLY with deprel=ROOT (no head=0/non-ROOT, no \
                 ROOT-deprel/head!=0); got %gra: {body}",
                rel.index,
                rel.head,
                rel.relation.as_str()
            );
            if head_zero {
                head_zero_count += 1;
            }
        }
        assert_eq!(
            head_zero_count, 1,
            "Family B invariant [{scenario_label}]: utterance must have \
             exactly one head=0 relation; got %gra: {body}"
        );
    }

    /// **Family B, B-WILD-1** — three-`@s` cluster as the host's
    /// utterance root, mirroring the sastre03.cha:843 shape
    /// (`+" yo@s soy@s el@s lieutenant .`).
    ///
    /// Setup: host primary `%gra: 1|0|ROOT 2|0|ROOT 3|0|ROOT 4|3|FLAT
    /// 5|3|PUNCT` would be invalid (multiple ROOTs); the realistic shape
    /// is one of the L2 positions carrying head=0/ROOT in the primary
    /// (here: chunk 3 = "el") and the others as in-cluster dependents.
    /// Three deferred L2 positions; one secondary response of three
    /// chunks where the secondary's own root is the second chunk
    /// ("soy" — Spanish copula); the splice must promote the
    /// secondary's root to the host's root anchor and emit
    /// `head=0/deprel=ROOT` (NOT `head=0/deprel=DET` or any other
    /// label inherited from the host's primary deprel for the L2
    /// position).
    ///
    /// EXPECTED on current build: FAILS via the joint-invariant walker.
    #[test]
    fn family_b_three_at_s_cluster_at_host_root_keeps_root_deprel() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\tyo soy el lieutenant .\n\
                         %mor:\tL2|xxx L2|xxx L2|xxx x|lieutenant .\n\
                         %gra:\t1|3|DEP 2|3|DEP 3|0|ROOT 4|3|FLAT 5|3|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        // Three deferred positions for "yo", "soy", "el".
        // Deferred index 2 ("el") is the host primary's root — that's
        // the position that carries the utterance-root anchor.
        let deferred = vec![
            deferred_position(line_idx, 0, "spa", "dep", 3),
            deferred_position(line_idx, 1, "spa", "dep", 3),
            deferred_position(line_idx, 2, "spa", "root", 0),
        ];

        // Secondary Stanza-Spanish parse of "yo soy el":
        //   yo  → head=2 (subject of soy), deprel=nsubj
        //   soy → head=0 (root), deprel=root
        //   el  → head=2 (det, but secondary's "el" has nothing to
        //         determine since "lieutenant" is outside the secondary
        //         input — Stanza in practice may attach el→soy with
        //         deprel=det or similar)
        // Each per-position MergedL2Morphology carries one chunk.
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("pron"), MorStem::new("yo"))),
                gras: vec![GrammaticalRelation::new(1, 2, "NSUBJ")],
                corrected_deprel: None,
                attachment: utterance_root_attachment(2),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("aux"), MorStem::new("ser"))),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: utterance_root_attachment(2),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("det"), MorStem::new("el"))),
                gras: vec![GrammaticalRelation::new(1, 2, "DET")],
                corrected_deprel: None,
                attachment: utterance_root_attachment(2),
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 3);
        assert_eq!(outcome.fallback, 0);

        assert_joint_root_invariant(
            &chat_file,
            line_idx,
            "three-@s-cluster-at-host-root (sastre03 shape)",
        );
    }

    /// **Family B, B-WILD-2** — two-`@s` Spanish noun phrase whose
    /// internal root is the second chunk, mirroring herring09.cha:2570
    /// (`... el@s camino@s .`). Host primary attaches the cluster to
    /// chunk K (a host word) with deprel=NMOD. Secondary parses the
    /// 2-chunk cluster with the second chunk (`camino`) as its root.
    ///
    /// EXPECTED on current build: FAILS — the wild output had
    /// `7|0|NMOD` (head=0, deprel=NMOD) instead of one consistent
    /// pairing.
    #[test]
    fn family_b_two_at_s_np_anchored_externally_keeps_root_deprel_consistent() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign1 foreign2 .\n\
                         %mor:\tverb|host L2|xxx L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|OBL 3|2|FLAT 4|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        // Host primary said "foreign1 foreign2" attaches to host chunk
        // 1 with deprel=OBL. Both deferred indices share the same
        // host attachment.
        let deferred = vec![
            deferred_position(line_idx, 1, "spa", "obl", 1),
            deferred_position(line_idx, 2, "spa", "flat", 2),
        ];
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("det"), MorStem::new("el"))),
                gras: vec![GrammaticalRelation::new(1, 2, "DET")],
                corrected_deprel: None,
                attachment: host_attachment(0, "obl"),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(
                    PosCategory::new("noun"),
                    MorStem::new("camino"),
                )),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: host_attachment(0, "obl"),
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 2);
        assert_eq!(outcome.fallback, 0);

        assert_joint_root_invariant(
            &chat_file,
            line_idx,
            "two-@s-NP-anchored-externally (herring09 shape)",
        );
    }

    /// **Family B, B-WILD-3** — joint-invariant guard for the
    /// already-fixed BUG-025 direction. Re-uses the existing
    /// `single_position_root_remap_does_not_leave_root_deprel_on_non_root_head`
    /// scenario but applies the symmetric joint-invariant walker, so
    /// any future regression in EITHER direction trips this test.
    ///
    /// EXPECTED on current build: PASSES (BUG-025 was fixed). Locks the
    /// fix in via the unifying invariant rather than a one-off
    /// "deprel != ROOT" assertion.
    #[test]
    fn family_b_joint_invariant_holds_after_bug025_remap() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign tail .\n\
                         %mor:\tverb|host L2|xxx noun|tail .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|OBJ 4|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "obj", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("extranjero"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(outcome.spliced, 1);
        assert_eq!(outcome.fallback, 0);

        assert_joint_root_invariant(
            &chat_file,
            line_idx,
            "BUG-025 single-position remap (regression guard)",
        );
    }

    // ========================================================================
    // Family C — Post-splice gra invariant under adversarial secondary
    // input. RED tests pinning the load-bearing rule:
    //
    //     splice_l2_into_chat MUST never write a `%gra` that violates
    //     the structural invariants checked by `validate_generated_gra`
    //     (`crates/talkbank-transform/src/morphosyntax/gra_validate.rs`):
    //     - exactly one head=0 ROOT relation
    //     - acyclic head graph
    //     - all heads in 0..=N
    //
    // When secondary Stanza dispatch produces noisy output (cycles,
    // out-of-bounds heads, multiple roots, terminator-punct as root),
    // OR when our merge slicing math is wrong, the splice must either
    // correctly normalize or fall back to `L2|xxx` for the affected
    // position. Silent passthrough produced 756 wild errors on
    // 2026-05-06 (E724=539, E713=109, E723=108): see
    // `~/talkbank/still-have-error-3.log`.
    //
    // Each fallback in production must emit a structured warning so
    // every fallback becomes an actionable TODO toward smarter merge
    // logic. (This is enforced separately at the splice's fallback
    // sites; tests here only assert post-splice gra is valid.)
    //
    // EXPECTED on current build: every test in this section FAILS
    // because the splice does not validate its own output.
    // ========================================================================

    /// Helper: assert post-splice ChatFile passes ALL the L2 splice
    /// integrity invariants. Reports the offending utterance and the
    /// validation diagnostics for debuggability. Use from any Family C
    /// test.
    fn assert_post_splice_gra_valid(chat: &mut talkbank_model::model::ChatFile, scenario: &str) {
        use talkbank_model::ParseValidateOptions;
        let opts = ParseValidateOptions::default().with_alignment();
        let result = talkbank_model::validate_chat_file_with_options(chat, &opts);
        if let Err(e) = result {
            let mut gra_summary = String::new();
            for line in &chat.lines {
                if let talkbank_model::model::Line::Utterance(u) = line {
                    for tier in &u.dependent_tiers {
                        if let talkbank_model::model::DependentTier::Gra(g) = tier {
                            gra_summary.push_str(&join_relations(g.relations()));
                            gra_summary.push_str(" | ");
                        }
                    }
                }
            }
            panic!(
                "Family C invariant violation [{scenario}]: post-splice \
                 ChatFile fails validation. Splice must reject and fall \
                 back to L2|xxx (with a structured warning) when secondary \
                 input would produce invalid gra.\n  post-splice gra: \
                 {gra_summary}\n  validation: {e:#?}"
            );
        }
    }

    /// **Family C, C1** — secondary's gra has head pointing at a
    /// nonexistent chunk (out-of-bounds). Mirrors the wild E713
    /// pattern at `asd-data/Croatian/ROGPOP/ASD/46.cha:970` where the
    /// post-splice %gra was `1|2|ADVMOD 2|0|ROOT 3|5|COMPOUND
    /// 4|2|PUNCT` — head=5 in a 4-chunk gra.
    ///
    /// Adversarial input: secondary merged result with one chunk but
    /// a gra relation pointing at chunk index 5 (which doesn't exist
    /// within the secondary's chunk count). The splice must not
    /// propagate the broken head into the host gra.
    ///
    /// EXPECTED on current build: FAILS — splice silently propagates
    /// the bogus head index.
    #[test]
    fn family_c_secondary_head_out_of_bounds_falls_back_or_normalizes() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\tone foreign three .\n\
                         %mor:\tnum|one L2|xxx num|three .\n\
                         %gra:\t1|2|NUMMOD 2|0|ROOT 3|2|FLAT 4|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "root", 0)];
        // Adversarial: head=5 references a nonexistent chunk.
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("dos"))),
            gras: vec![GrammaticalRelation::new(1, 5, "COMPOUND")],
            corrected_deprel: None,
            attachment: utterance_root_attachment(0),
        })];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(&mut chat_file, "C1 — secondary head OOB (E713 wild shape)");
    }

    /// **Family C, C2** — secondary's gras form a 2-cycle.
    /// Mirrors the wild E724 dominant pattern
    /// `1|2|DET 2|3|NMOD 3|2|PUNCT` (18 occurrences across the corpus).
    ///
    /// Adversarial input: 2-chunk MWT secondary where chunk 1 → 2 and
    /// chunk 2 → 1 (mutual reference cycle within the secondary slice).
    ///
    /// EXPECTED on current build: FAILS — splice propagates the cycle.
    #[test]
    fn family_c_secondary_cycle_falls_back_or_normalizes() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, eng\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tle foreign .\n\
                         %mor:\tdet|le L2|xxx .\n\
                         %gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "eng", "root", 0)];
        // Adversarial: secondary returns a 2-chunk MWT result with an
        // internal 2-cycle.
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("foo")))
                .with_post_clitic(MorWord::new(PosCategory::new("part"), MorStem::new("bar"))),
            gras: vec![
                GrammaticalRelation::new(1, 2, "FLAT"),
                GrammaticalRelation::new(2, 1, "FLAT"),
            ],
            corrected_deprel: None,
            attachment: utterance_root_attachment(0),
        })];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(&mut chat_file, "C2 — secondary 2-cycle (E724 wild shape)");
    }

    /// **Family C, C3** — secondary's gras have multiple head=0
    /// relations. Mirrors wild E723 patterns like
    /// `1|0|ROOT 2|3|COP 3|0|ROOT 4|1|PUNCT`.
    ///
    /// Adversarial input: 2-chunk MWT secondary where BOTH chunks
    /// have head=0 / deprel=ROOT (Stanza emitting two roots, which
    /// is malformed UD but does happen).
    ///
    /// EXPECTED on current build: FAILS — splice's merge inserts both
    /// secondary roots, leaving the host with two head=0 relations.
    #[test]
    fn family_c_secondary_multi_root_falls_back_or_normalizes() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost foreign .\n\
                         %mor:\tverb|host L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|OBJ 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "spa", "obj", 1)];
        // Adversarial: secondary returns 2 chunks, BOTH labelled ROOT
        // with head=0.
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("a")))
                .with_post_clitic(MorWord::new(PosCategory::new("noun"), MorStem::new("b"))),
            gras: vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 0, "ROOT"),
            ],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "C3 — secondary multi-root (E723 wild shape)",
        );
    }

    /// **Family C, C4** — secondary's per-relation head points at a
    /// host-side terminator chunk after remap. Mirrors the
    /// `bougzers@s` cycle pattern at
    /// `biling-data/MLE-MPF/09.cha:2425`. Wild output:
    /// `1|2|DET 2|3|NMOD 3|2|PUNCT` — chunk 2 (the L2 word) has
    /// head=3 (the period). Cycle 2↔3.
    ///
    /// Adversarial shape: single L2 word as host's utterance root,
    /// secondary returns one chunk whose gra has head=2 within the
    /// secondary's local index space (e.g., Stanza pointed
    /// `bougzers` at the period that followed it in the secondary
    /// dispatch input).
    ///
    /// EXPECTED on current build: FAILS — splice propagates the bogus
    /// head into the host instead of honoring the planner's
    /// `UtteranceRoot` attachment.
    #[test]
    fn family_c_secondary_head_into_host_terminator_falls_back() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, eng\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tle foreign .\n\
                         %mor:\tdet|le L2|xxx .\n\
                         %gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "eng", "root", 0)];
        // Adversarial: Stanza English treats placeholder as a dependent
        // of a phantom token at secondary local index 2 (which doesn't
        // exist in this 1-chunk merged response). After the splice's
        // remap, head=2 maps to host chunk 3 (the period), creating a
        // 2-cycle (chunk 2 ↔ chunk 3).
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("foreign"),
            )),
            gras: vec![GrammaticalRelation::new(1, 2, "NMOD")],
            corrected_deprel: None,
            attachment: utterance_root_attachment(0),
        })];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "C4 — secondary head into host terminator (bougzers wild shape)",
        );
    }

    /// **Family C, C6** — multi-position contiguous span where
    /// `splice_range_coordinated` succeeds with invalid output.
    /// Mirrors the wild `por@s favor@s` shape at
    /// `biling-data/Bangor/Miami/eng/maria/maria20.cha:559-562`:
    ///
    /// ```text
    /// *MAR: they look normal Jackie por@s favor@s .
    /// %mor: pron|they verb|look adj|normal propn|Jackie adp|por noun|favor .
    /// %gra: 1|2|NSUBJ 2|0|ROOT 3|5|AMOD 4|5|COMPOUND 5|4|FLAT 6|5|FIXED 7|2|PUNCT
    /// ```
    ///
    /// Cycle: chunk 4 → chunk 5 (COMPOUND), chunk 5 → chunk 4 (FLAT).
    /// E724 fires.
    ///
    /// Adversarial scenario for the unit test: two contiguous `@s`
    /// positions whose secondaries supply gras that — when concatenated
    /// — encode a 2-cycle within the spliced span (chunk-1 → chunk-2,
    /// chunk-2 → chunk-1). After the host-side index remap this
    /// produces a cycle in the host gra that `splice_range_coordinated`
    /// doesn't reject (the mor chunk count balances; the gra count
    /// balances; only the structural acyclic invariant is violated).
    ///
    /// EXPECTED on current build: FAILS — multi-position branch has
    /// no post-splice validation.
    #[test]
    fn family_c_multi_position_contiguous_internal_cycle_falls_back() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost a b .\n\
                         %mor:\tverb|host L2|xxx L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|DEP 4|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        // Two CONTIGUOUS positions, same target_lang — exercises the
        // multi-position branch (span_size > 1) of splice_l2_into_chat.
        let deferred = vec![
            deferred_position(line_idx, 1, "spa", "dep", 1),
            deferred_position(line_idx, 2, "spa", "dep", 1),
        ];
        // Adversarial shape: secondary's gras for the 2-position span
        // form an in-span 2-cycle.
        //
        // Secondary returns a 2-chunk block where:
        //   - chunk 1's gra has head=2 (points at chunk 2 within span)
        //   - chunk 2's gra has head=1 (points back at chunk 1)
        //
        // Chunk counts and gra counts balance, so
        // `splice_range_coordinated` accepts the input. Only the cycle
        // invariant is violated.
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("adp"), MorStem::new("a"))),
                gras: vec![GrammaticalRelation::new(1, 2, "FIXED")],
                corrected_deprel: None,
                attachment: host_attachment(0, "dep"),
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("b"))),
                gras: vec![GrammaticalRelation::new(1, 1, "FLAT")],
                corrected_deprel: None,
                attachment: host_attachment(1, "dep"),
            }),
        ];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "C6 — multi-position contiguous internal cycle (maria20 wild shape)",
        );
    }

    /// **Family C, C5** — joint sweep with two L2 positions in one
    /// utterance: one position has adversarial OOB input, the other
    /// has clean input. Forces the splice to handle multi-position
    /// fallback consistently — the bad position must not poison the
    /// good one, and vice versa.
    ///
    /// EXPECTED on current build: FAILS at the OOB position.
    #[test]
    fn family_c_multi_position_adversarial_sweep_falls_back() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, spa\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\thost a b .\n\
                         %mor:\tverb|host L2|xxx L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|DEP 3|1|DEP 4|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![
            deferred_position(line_idx, 1, "spa", "dep", 1),
            deferred_position(line_idx, 2, "spa", "dep", 1),
        ];
        let merged = vec![
            // Position 1: adversarial OOB head=99
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("a"))),
                gras: vec![GrammaticalRelation::new(1, 99, "COMPOUND")],
                corrected_deprel: None,
                attachment: host_attachment(0, "dep"),
            }),
            // Position 2: clean — should succeed normally and not be
            // disturbed by the failure at position 1.
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("b"))),
                gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
                corrected_deprel: None,
                attachment: host_attachment(1, "dep"),
            }),
        ];

        splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_post_splice_gra_valid(&mut chat_file, "C5 — multi-position adversarial sweep");
    }

    // ─────────────────────────────────────────────────────────────────────
    // L2 redesign 2026-05-07 — constructive merge tests.
    //
    // These four tests pin the dominant rollback variants observed in the
    // wild (net captured-tracing run 8b461fee-df9, 750 sample files):
    //
    //   secondary_no_root        40.4%  → constructed away (Tests 1, 5)
    //   secondary_multi_root     38.5%  → constructed away (Test 2)
    //   secondary_head_oob       11.0%  → constructed away (Test 3)
    //   secondary_cycle          10.1%  → genuine fallback (Test 4)
    //
    // Each test fixture is grounded in a real warn-line shape from
    // a captured-tracing morphotag run's `server.log`. Root-cause
    // walk-through is in the private workspace.
    //
    // PRE-FIX: tests 1-3 fail (rollback, fallback==1). Test 4 passes
    // (cycle is irreducible; rollback is the right answer).
    // POST-FIX: tests 1-3 pass (splice succeeds; tree invariants
    // maintained by construction). Test 4 still passes (regression pin).
    // ─────────────────────────────────────────────────────────────────────

    /// Test 1 (no_root): single-position L2 span where the L2 word IS the
    /// host root, and secondary's per-position relation has NO `head=0,
    /// ROOT`. Real-world shape from the warn log: host `4|0|ROOT` at the
    /// L2 position; secondary returns `1|2|NMOD`; current splice replaces
    /// `4|0|ROOT` with `4|5|NMOD`, killing the only root. After fix:
    /// merge ensures the L2 position becomes `head=0, ROOT` because the
    /// `InternalRoot` attachment promises this position owns the host's
    /// root.
    #[test]
    fn merge_constructs_root_when_l2_span_owns_host_root() {
        // Host: `voici yellow@s .` — `yellow` is at word_idx=1 and is the
        // host's primary root (matches the 2026-05-07 warn-line family
        // where the L2 word at word_idx=N IS at host root position).
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, ara\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tvoici yellow@s .\n\
                         %mor:\tintj|voici L2|xxx .\n\
                         %gra:\t1|2|DISCOURSE 2|0|ROOT 3|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "ara", "root", 0)];
        // Secondary's relation lacks head=0/ROOT — head=2 mimics the
        // wild warn-line `secondary_gras=["1|2|NMOD"]`. Single-chunk
        // merged Mor (one gra entry).
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("yellow"),
            )),
            gras: vec![GrammaticalRelation::new(1, 2, "NMOD")],
            corrected_deprel: None,
            attachment: L2Attachment::InternalRoot,
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 1,
            "InternalRoot span must splice successfully; merge must \
             ensure the position carries head=0/ROOT regardless of \
             secondary's relation. Got fallback={}, spliced={}.",
            outcome.fallback, outcome.spliced
        );
        assert_eq!(outcome.fallback, 0);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "Test 1 — InternalRoot, no head=0/ROOT in secondary",
        );
        // Stronger: assert the L2 position is the post-splice root.
        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("post-splice gra present");
        let roots: Vec<_> = gra
            .relations()
            .iter()
            .filter(|r| r.head == 0 && r.relation.eq_ignore_ascii_case("ROOT"))
            .collect();
        assert_eq!(
            roots.len(),
            1,
            "post-splice gra must have exactly one ROOT; got {} ({:?})",
            roots.len(),
            gra.relations()
        );
    }

    /// Test 2 (multi_root): L2 word is NOT the host root, and secondary
    /// returned `1|0|ROOT` for the L2 word (parsed it as its local root).
    /// Splice must rewrite secondary's `head=0/ROOT` to attach to the
    /// host anchor with the host's deprel — not preserve a second root.
    /// Real-world shape from warn log: host_pre had `16|0|ROOT, 18|...`;
    /// L2 at host pos 18; secondary returned `1|0|ROOT`; current splice
    /// produces both `16|0|ROOT` and `18|0|ROOT`.
    #[test]
    fn merge_does_not_double_root_when_secondary_returns_local_root() {
        // Host: `voici yellow@s .` — `voici` is host root; `yellow` is OBJ.
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, ara\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tvoici yellow@s .\n\
                         %mor:\tintj|voici L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|OBJ 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "ara", "obj", 1)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("yellow"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 1,
            "ExternalRoot span must splice; secondary's `1|0|ROOT` must \
             be rewritten to attach to host anchor. \
             fallback={}, spliced={}",
            outcome.fallback, outcome.spliced
        );
        assert_eq!(outcome.fallback, 0);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "Test 2 — ExternalRoot, secondary returns head=0/ROOT",
        );
        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("post-splice gra present");
        let roots: Vec<_> = gra
            .relations()
            .iter()
            .filter(|r| r.head == 0 && r.relation.eq_ignore_ascii_case("ROOT"))
            .collect();
        assert_eq!(
            roots.len(),
            1,
            "post-splice gra must have exactly one ROOT (the host's \
             original root, not a duplicate from secondary); got {} ({:?})",
            roots.len(),
            gra.relations()
        );
    }

    /// Test 3 (head_oob): single-position L2 with secondary's head index
    /// way out of bounds. Real-world shape: secondary returned
    /// `1|11|NMOD` when the host has only 10 positions; current splice's
    /// `splice_coordinated` translation produces `9|11|NMOD` and the
    /// validator rejects it. After fix: merge clamps OOB heads to attach
    /// at the host anchor (or treats them as external attachments).
    #[test]
    fn merge_clamps_secondary_head_to_anchor_when_local_index_oob() {
        // Host: `voici yellow@s .` — `voici` is host root; `yellow` is OBJ.
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\tfra, ara\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\tfra|test|PAR|||||Participant|||\n\
                         *PAR:\tvoici yellow@s .\n\
                         %mor:\tintj|voici L2|xxx .\n\
                         %gra:\t1|0|ROOT 2|1|OBJ 3|1|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _errors) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        let deferred = vec![deferred_position(line_idx, 1, "ara", "obj", 1)];
        // Secondary returns `1|99|NMOD` — head=99 has no host preimage.
        // Today: splice_coordinated maps 99 to a host index out of bounds
        // → secondary_head_oob. After fix: head clamped to anchor (host
        // pos 1, the verb), relation rewritten to host's `obj`.
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("yellow"),
            )),
            gras: vec![GrammaticalRelation::new(1, 99, "NMOD")],
            corrected_deprel: None,
            attachment: host_attachment(0, "obj"),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 1,
            "OOB-head span must splice with head clamped to anchor. \
             fallback={}, spliced={}",
            outcome.fallback, outcome.spliced
        );
        assert_eq!(outcome.fallback, 0);
        assert_post_splice_gra_valid(&mut chat_file, "Test 3 — ExternalRoot, secondary head OOB");
    }

    /// Test 4 (cycle, REPAIRED): two-position L2 span where the
    /// per-position secondary relations form a cycle within the span.
    ///
    /// **Originally drafted as a regression pin for "cycles are
    /// irreducible".** After implementing the constructive merge, it
    /// turns out cycles ARE repairable: pass 4 of `repair_secondary_gras`
    /// detects a cycle by walking head chains and breaks it by
    /// re-rooting one of the cycle members. The result is a valid tree
    /// (with one position deterministically picked as the root) rather
    /// than a rollback to L2|xxx.
    ///
    /// This is strictly better than rollback: morphology (POS, lemma,
    /// features) is preserved at every position; only the structural
    /// arc that participated in the cycle gets adjusted. The "wrong
    /// attachment within span" cost is bounded to one edge per cycle.
    ///
    /// What stays the same: Test 4 still pins the cycle code path —
    /// it now pins that the cycle gets *repaired* rather than that it
    /// rolls back. The regression class this defends against is "the
    /// cycle-detection pass silently regresses to letting the cyclic
    /// gras through to splice, where it fails the post-splice
    /// validator".
    /// Test 5 (multi_root via UtteranceRoot mismatch): the L2 plan
    /// picked `UtteranceRoot` for an L2 word (because the primary
    /// host parse said primary.head=0 for that position at extract
    /// time), but the host's final gra has the root at a DIFFERENT
    /// position. Pre-fix: splice keeps `head=0/ROOT` at the L2
    /// position and the host's existing root, producing two
    /// head=0/ROOT entries → `secondary_multi_root` rollback to
    /// L2|xxx. Post-fix: post-splice repair detects the duplicate
    /// root and demotes the L2 contribution to attach to the host
    /// root with a generic DEP relation.
    ///
    /// Background: this is the 43-rollbacks-per-750-files variant
    /// the constructive merge didn't address. See the postmortem at
    /// `docs/postmortems/2026-05-07-noalign-morphotag-skip.md`.
    #[test]
    fn merge_demotes_duplicate_root_when_host_already_has_one() {
        let chat_text = "@UTF8\n\
                         @Begin\n\
                         @Languages:\teng, fra\n\
                         @Participants:\tPAR Participant\n\
                         @ID:\teng|test|PAR|||||Participant|||\n\
                         *PAR:\tshe said yellow@s .\n\
                         %mor:\tpron|she verb|say-Past L2|xxx .\n\
                         %gra:\t1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT\n\
                         @End\n";
        let parser = TreeSitterParser::new().unwrap();
        let (mut chat_file, _) = parse_lenient(&parser, chat_text);
        let line_idx = chat_file
            .lines
            .iter()
            .position(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .unwrap();

        // L2 word at word_idx=2 ("yellow"). Plan thinks utterance
        // root (UtteranceRoot attachment), but host's final gra has
        // the verb at position 2 as root.
        let deferred = vec![deferred_position(line_idx, 2, "fra", "root", 0)];
        let merged = vec![Some(MergedL2Morphology {
            mor: Mor::new(MorWord::new(
                PosCategory::new("noun"),
                MorStem::new("yellow"),
            )),
            gras: vec![GrammaticalRelation::new(1, 0, "ROOT")],
            corrected_deprel: None,
            attachment: utterance_root_attachment(0),
        })];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 1,
            "L2 with UtteranceRoot conflicting with host root must \
             splice (with demotion) rather than rolling back. \
             fallback={}, spliced={}",
            outcome.fallback, outcome.spliced
        );
        assert_eq!(outcome.fallback, 0);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "Test 5 — UtteranceRoot vs host root conflict",
        );

        // Verify post-splice has exactly ONE head=0/ROOT.
        let utt = match &chat_file.lines[line_idx] {
            talkbank_model::model::Line::Utterance(u) => u,
            _ => unreachable!(),
        };
        let gra = utt
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                talkbank_model::model::DependentTier::Gra(g) => Some(g),
                _ => None,
            })
            .expect("post-splice gra present");
        let roots: Vec<_> = gra
            .relations()
            .iter()
            .filter(|r| r.head == 0 && r.relation.eq_ignore_ascii_case("ROOT"))
            .collect();
        assert_eq!(
            roots.len(),
            1,
            "exactly one head=0/ROOT must remain (host's original); \
             got {} ({:?})",
            roots.len(),
            gra.relations()
        );
        // The host's root at position 2 should be the surviving one;
        // the L2 word at position 3 (its chunk after the verb) should
        // be demoted to attach to it.
        let host_root = roots[0];
        assert_eq!(host_root.index, 2, "host root preserved at position 2");
    }

    #[test]
    fn merge_falls_back_only_on_genuine_secondary_cycle() {
        let (mut chat_file, line_idx) = build_l2_fixture("eng, fra", "x y", 2);

        let deferred = vec![
            deferred_position(line_idx, 0, "fra", "dep", 0),
            deferred_position(line_idx, 1, "fra", "dep", 0),
        ];
        // Each position's secondary parse points at the OTHER position
        // (in span-local indexing): pos 0's relation says head=2 (= span
        // pos 1), pos 1's relation says head=1 (= span pos 0). Cycle.
        let merged = vec![
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("x"))),
                gras: vec![GrammaticalRelation::new(1, 2, "DEP")],
                corrected_deprel: None,
                attachment: L2Attachment::InternalRoot,
            }),
            Some(MergedL2Morphology {
                mor: Mor::new(MorWord::new(PosCategory::new("noun"), MorStem::new("y"))),
                gras: vec![GrammaticalRelation::new(1, 1, "DEP")],
                corrected_deprel: None,
                attachment: L2Attachment::InternalRoot,
            }),
        ];

        let outcome = splice_l2_into_chat(&mut chat_file, &deferred, &merged);
        assert_eq!(
            outcome.spliced, 2,
            "cycle must be repaired into a valid tree; \
             fallback={}, spliced={}",
            outcome.fallback, outcome.spliced
        );
        assert_eq!(outcome.fallback, 0);
        assert_post_splice_gra_valid(
            &mut chat_file,
            "Test 4 — cycle in secondary, repaired via cycle-detection pass",
        );
    }
}

#[cfg(test)]
fn host_attachment(source_deferred_index: usize, deprel: &str) -> L2Attachment {
    L2Attachment::ExternalRoot {
        host_deprel: crate::morphosyntax::l2::deprel::UdDeprel::new(deprel),
        root_anchor: L2RootAnchor::HostGovernor {
            source_deferred_index,
        },
    }
}

#[cfg(test)]
fn utterance_root_attachment(source_deferred_index: usize) -> L2Attachment {
    L2Attachment::ExternalRoot {
        host_deprel: crate::morphosyntax::l2::deprel::UdDeprel::new("root"),
        root_anchor: L2RootAnchor::UtteranceRoot {
            source_deferred_index,
        },
    }
}

/// Apply L2|xxx fallback to deferred positions that have no merged result.
pub fn apply_l2_fallback(
    chat_file: &mut talkbank_model::model::ChatFile,
    deferred: &[L2DeferredPosition],
) {
    use talkbank_model::model::DependentTier;
    use talkbank_model::model::Line;

    for def in deferred {
        let utt = match &mut chat_file.lines[def.line_idx] {
            Line::Utterance(u) => u,
            _ => continue,
        };
        let mor_tier = utt.dependent_tiers.iter_mut().find_map(|t| match t {
            DependentTier::Mor(m) => Some(m),
            _ => None,
        });
        if let Some(mor) = mor_tier
            && let Some(mor_item) = mor.items_mut().get_mut(def.word_idx)
        {
            mor_item.main.reset_to_l2_placeholder();
        }
    }
}
