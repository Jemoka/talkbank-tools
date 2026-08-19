//! Utterance segmentation helpers.
//!
//! Splits a single utterance into multiple utterances based on word-level
//! assignments from a segmentation callback.
//!
//! Also provides types and functions for the server-side utseg orchestrator:
//! payload collection, cache key computation, and result application.
//!
//! ## Outcome model
//!
//! Every utterance visited by `collect_utseg_payloads` + `apply_utseg_results`
//! produces exactly one [`UtsegOutcome`]. This is the sibling-task analog of
//! morphotag's [`MorOutcome`](crate::morphosyntax::outcome::MorOutcome) and
//! serves the same architectural purpose: making correct-by-design behavior
//! (e.g. single-word utterances that trivially need no segmentation) visible
//! as a typed `NotApplicable` outcome rather than invisible silent skip, and
//! making worker-response shape mismatches fail loudly as typed
//! `MisalignmentBug` diagnostics rather than being absorbed by defensive
//! index guards in `split_utterance`.
//!
//! The utseg invariant is simpler than morphotag's: there is no tokenizer
//! realignment stage — the Python utseg worker is a per-word classifier
//! whose `assignments` return MUST have the same length as the input
//! `words`. A mismatch is always a worker-contract bug, not an expected
//! divergence class.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use talkbank_model::Span;
use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{
    ChatFile, DependentTier, Line, MainTier, Terminator, Utterance, UtteranceContent, WorTier,
};
use talkbank_model::validation::ValidationState;

use crate::extract;
use talkbank_model::SpeakerCode;

// ---------------------------------------------------------------------------
// Wire types (match Python's UtsegBatchItem / UtsegResponse)
// ---------------------------------------------------------------------------

/// Input payload for a single utterance segmentation request.
///
/// Matches the Python `UtsegBatchItem` Pydantic model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UtsegBatchItem {
    /// Tokenized words from the utterance.
    pub words: Vec<String>,
    /// Full utterance text (for constituency parsing).
    pub text: String,
}

/// Response from utterance segmentation inference.
///
/// Each element in `assignments` is a 0-based utterance group ID, parallel
/// to the `words` in the corresponding `UtsegBatchItem`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtsegResponse {
    /// 0-based utterance group ID per word, parallel to `UtsegBatchItem::words`.
    pub assignments: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Typed outcome model (Wave 5 of the morphotag reconciliation architecture)
// ---------------------------------------------------------------------------

/// One utterance segmentation outcome.
///
/// Carries `utt_ordinal` and `speaker` so it can be converted to a
/// [`DecisionRecord`](crate::decisions::DecisionRecord) without further
/// context. `line_idx` is also available when needed — utseg is indexed
/// by `utt_ordinal` rather than `line_idx` to align with the existing
/// `HashMap<utt_ordinal, assignments>` dispatch map, but the two are
/// trivially interconvertible.
#[derive(Debug, Clone)]
pub struct UtsegOutcome {
    /// 0-based index of the utterance among all `Utterance` lines in the file.
    pub utt_ordinal: usize,
    /// Speaker code for the affected utterance.
    pub speaker: SpeakerCode,
    /// What happened on this utterance.
    pub kind: UtsegOutcomeKind,
}

/// The three possible utseg outcomes per utterance.
///
/// Structurally parallel to
/// [`MorOutcomeKind`](crate::morphosyntax::outcome::MorOutcomeKind); see
/// the morphotag invariants architecture doc for rationale.
#[derive(Debug, Clone)]
pub enum UtsegOutcomeKind {
    /// The utterance did not require segmentation.
    ///
    /// Most commonly this is a single-word utterance — a one-word
    /// utterance trivially occupies one segment, so utseg skips the
    /// worker call entirely. It is CORRECT behavior, not a silent skip.
    NotApplicable {
        /// Why this utterance was not dispatched.
        reason: UtsegNotApplicableReason,
    },
    /// Worker returned exactly N assignments for N input words;
    /// `split_utterance` applied cleanly. Happy path.
    Aligned {
        /// The agreed word count on both sides.
        n_words: usize,
        /// Number of segments the utterance was split into.
        /// `1` means "all words assigned the same group" (no split).
        n_segments: usize,
    },
    /// Worker returned a response whose `assignments` length does not
    /// match the dispatched `words` length. This is always a
    /// worker-contract bug — the Python classifier is supposed to emit
    /// one assignment per input word.
    MisalignmentBug(UtsegMisalignmentDiagnostic),
}

/// Why an utterance was not dispatched to the utseg worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtsegNotApplicableReason {
    /// The utterance had a single alignable word. Segmentation into one
    /// segment is trivial; the worker call is skipped for efficiency.
    SingleWord,
    /// The utterance had zero alignable words (filler-only, empty, etc.).
    /// Nothing to segment.
    Empty,
}

impl UtsegNotApplicableReason {
    /// Short label for `%xalign` tier output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SingleWord => "single_word",
            Self::Empty => "empty",
        }
    }
}

/// Diagnostic for an utseg misalignment bug — the worker did not return
/// the contract-required number of assignments.
#[derive(Debug, Clone)]
pub struct UtsegMisalignmentDiagnostic {
    /// Number of words sent to the worker.
    pub expected_assignments: usize,
    /// Number of assignments the worker actually returned.
    pub actual_assignments: usize,
    /// The words that were sent — helps a developer reproduce the case.
    pub words: Vec<String>,
}

impl UtsegOutcome {
    /// Convert into a [`DecisionRecord`](crate::decisions::DecisionRecord)
    /// for surfacing via the `%xalign` tier. Aligned outcomes return
    /// `None` for the same reason as `MorOutcome`: happy-path
    /// utterances shouldn't flood the reporting tier.
    pub fn to_decision_record(&self, line_idx: usize) -> Option<crate::decisions::DecisionRecord> {
        use crate::decisions::{DecisionRecord, DecisionStrategy, UtsegStrategy};
        match &self.kind {
            UtsegOutcomeKind::Aligned { .. } => None,
            UtsegOutcomeKind::NotApplicable { reason } => Some(DecisionRecord {
                line_idx,
                speaker: self.speaker.as_str().to_string(),
                strategy: DecisionStrategy::Utseg(UtsegStrategy::NotApplicable),
                reason: format!("reason={}", reason.as_str()),
                needs_review: false,
            }),
            UtsegOutcomeKind::MisalignmentBug(diag) => Some(DecisionRecord {
                line_idx,
                speaker: self.speaker.as_str().to_string(),
                strategy: DecisionStrategy::Utseg(UtsegStrategy::MisalignmentBug),
                reason: format!(
                    "expected_assignments={} actual_assignments={} words={:?}",
                    diag.expected_assignments, diag.actual_assignments, diag.words,
                ),
                needs_review: true,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Payload collection
// ---------------------------------------------------------------------------

/// Result of [`collect_utseg_payloads`]: batch items to dispatch plus
/// typed outcomes for every utterance that was not dispatched.
///
/// Mirrors the shape of
/// [`PayloadCollection`](crate::morphosyntax::payloads::PayloadCollection)
/// from Wave 1. Utterances fall into one of two mutually-exclusive sets:
/// `batch_items` (will be sent to the worker) and `not_applicable`
/// (will not be dispatched; correct).
pub struct UtsegPayloadCollection {
    /// Utterances that will be sent to the utseg worker.
    pub batch_items: Vec<(usize, UtsegBatchItem)>,
    /// Utterances that were classified as NotApplicable and not dispatched,
    /// each carrying a structured reason.
    pub not_applicable: Vec<UtsegOutcome>,
}

/// Collect utseg payloads from all multi-word utterances in a ChatFile.
///
/// Single-word and empty utterances are classified as
/// [`UtsegNotApplicableReason::SingleWord`] / `Empty` and returned as
/// [`UtsegOutcome::NotApplicable`] entries — no worker call, no silent
/// skip.
pub fn collect_utseg_payloads(chat_file: &ChatFile) -> UtsegPayloadCollection {
    let mut batch_items = Vec::new();
    let mut not_applicable = Vec::new();
    let mut utt_idx = 0usize;

    for line in chat_file.lines.iter() {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        let mut words = Vec::new();
        extract::collect_utterance_content(&utt.main.content.content, TierDomain::Mor, &mut words);

        let speaker = SpeakerCode::new(utt.main.speaker.as_str());
        match words.len() {
            0 => {
                not_applicable.push(UtsegOutcome {
                    utt_ordinal: utt_idx,
                    speaker,
                    kind: UtsegOutcomeKind::NotApplicable {
                        reason: UtsegNotApplicableReason::Empty,
                    },
                });
            }
            1 => {
                not_applicable.push(UtsegOutcome {
                    utt_ordinal: utt_idx,
                    speaker,
                    kind: UtsegOutcomeKind::NotApplicable {
                        reason: UtsegNotApplicableReason::SingleWord,
                    },
                });
            }
            _ => {
                // Single pass: build both `text` (space-joined) and `word_texts` together
                let mut text = String::new();
                let mut word_texts = Vec::with_capacity(words.len());
                for (i, w) in words.iter().enumerate() {
                    if i > 0 {
                        text.push(' ');
                    }
                    let s = w.text.as_str();
                    text.push_str(s);
                    word_texts.push(s.to_string());
                }

                batch_items.push((
                    utt_idx,
                    UtsegBatchItem {
                        words: word_texts,
                        text,
                    },
                ));
            }
        }

        utt_idx += 1;
    }

    UtsegPayloadCollection {
        batch_items,
        not_applicable,
    }
}

/// Validate one utseg worker response against the dispatched batch item.
///
/// Returns the classified outcome kind. [`UtsegOutcomeKind::MisalignmentBug`]
/// is emitted when the worker's `assignments` vector has a different
/// length than the dispatched `words` — always a worker-contract bug.
pub fn validate_utseg_response(
    batch_item: &UtsegBatchItem,
    response: &UtsegResponse,
) -> UtsegOutcomeKind {
    let expected = batch_item.words.len();
    let actual = response.assignments.len();
    if expected != actual {
        return UtsegOutcomeKind::MisalignmentBug(UtsegMisalignmentDiagnostic {
            expected_assignments: expected,
            actual_assignments: actual,
            words: batch_item.words.clone(),
        });
    }
    // Count distinct segment IDs in the assignments to report n_segments.
    let mut distinct = std::collections::BTreeSet::new();
    for &a in &response.assignments {
        distinct.insert(a);
    }
    UtsegOutcomeKind::Aligned {
        n_words: expected,
        n_segments: distinct.len(),
    }
}

// ---------------------------------------------------------------------------
// Result application
// ---------------------------------------------------------------------------

/// Apply utseg assignments to a ChatFile, splitting utterances as needed.
///
/// `assignment_map` maps `utt_ordinal` to assignments (parallel to extracted words).
/// Utterances whose ordinals are not in the map are left unchanged.
pub fn apply_utseg_results<S: ValidationState>(
    chat_file: &mut ChatFile<S>,
    assignment_map: &HashMap<usize, Vec<usize>>,
) {
    if assignment_map.is_empty() {
        return;
    }

    let old_lines = std::mem::take(&mut chat_file.lines.0);
    let mut new_lines: Vec<Line> = Vec::with_capacity(old_lines.len());
    let mut utt_ordinal = 0usize;

    for line in old_lines {
        let utt = match line {
            Line::Utterance(u) => u,
            other => {
                new_lines.push(other);
                continue;
            }
        };

        if let Some(assignments) = assignment_map.get(&utt_ordinal) {
            let split_utts = split_utterance(*utt, assignments);
            for split_utt in split_utts {
                new_lines.push(Line::Utterance(Box::new(split_utt)));
            }
        } else {
            new_lines.push(Line::Utterance(utt));
        }

        utt_ordinal += 1;
    }

    chat_file.lines.0 = new_lines;
}

/// Build a mapping from extracted-word index to top-level content item index.
pub fn build_word_to_content_map(content: &[UtteranceContent]) -> Vec<usize> {
    let mut word_to_content = Vec::new();

    for (content_idx, item) in content.iter().enumerate() {
        let mut words = Vec::new();
        extract::collect_utterance_content(std::slice::from_ref(item), TierDomain::Mor, &mut words);
        for _ in &words {
            word_to_content.push(content_idx);
        }
    }

    word_to_content
}

/// Per-tier behavior when an utterance is split into multiple children.
///
/// Splitting an utterance is a transformation that invalidates some
/// dependent-tier data and not others. This enum makes the per-tier
/// decision explicit and grep-able. `policy_for_tier` is the single
/// dispatch site; tests cover each variant.
///
/// History: BA3 deliberately removed parse-time `%wor` alignment in
/// commits `3c178f49` / `ca18388f` / `f7d86537` (2026-04-09) because
/// `chatter validate` was firing `%wor`-count errors on every shift in
/// token-classification semantics. That rename addressed *validation*
/// thrash. This policy addresses a separate concern: when `split_utterance`
/// repartitions words, the per-word data on `%wor` (and similar tiers)
/// should still travel with its words even though no validator demands
/// positional alignment. The rename made staleness *legal*; this policy
/// makes data preservation *useful*. They are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TierSplitPolicy {
    /// Walk the parent's items in lockstep with main-tier words and
    /// distribute them across children by the existing word→child
    /// mapping. Falls back to `Drop` if positional counts mismatch
    /// (stale `%wor` from prior edits, or tokenization drift). The
    /// fallback is silent — it emits `tracing::debug!`, never a
    /// validation error, preserving the stale-`%wor`-is-fine stance.
    Partition,
    /// Drop the tier from all children. The tier's data is
    /// semantically invalidated by the split: morphological analysis
    /// assumed the original utterance boundary; dependency arcs
    /// reference word indices that no longer match; coreference
    /// chains span document positions that the split changes. The
    /// user regenerates via `morphotag` / `coref`.
    Drop,
    /// Attach the tier (unchanged) to the first child only. The data
    /// is utterance-level free-form (`%com` comments, `%xtra`
    /// translations, user-defined `%x*` annotations) with no
    /// positional semantics to violate. Stale-on-first-child is
    /// strictly better than silent loss: the user can re-translate or
    /// correct manually, and improves on BA2 which dropped these
    /// unconditionally.
    AttachFirst,
}

/// Map a dependent tier to its split policy.
///
/// Word-positional, context-free tiers (`%wor`) get [`Partition`]. Word-positional
/// but context-dependent tiers (`%mor`, `%gra`) get [`Drop`] — the data is
/// invalid in the new context. Document- or analysis-scoped tiers (`%coref`)
/// also `Drop`. Other word-positional tiers we don't yet have a partition
/// implementation for (`%pho`, `%mod`, `%sin`, etc.) `Drop` rather than
/// `AttachFirst`, because attaching the parent's full per-word data to one
/// child would falsely claim the data covers all the original words. Free-form
/// utterance-level tiers (`%com`, `%xtra`, `%add`, etc., text tiers, user-defined,
/// unsupported) default to `AttachFirst` — preserve the data on the first child.
///
/// [`Partition`]: TierSplitPolicy::Partition
/// [`Drop`]: TierSplitPolicy::Drop
/// [`AttachFirst`]: TierSplitPolicy::AttachFirst
fn policy_for_tier(tier: &DependentTier) -> TierSplitPolicy {
    match tier {
        // Per-word timing — partitionable by word index.
        DependentTier::Wor(_) => TierSplitPolicy::Partition,

        // Context-dependent or reference-structured: drop, regenerate downstream.
        DependentTier::Mor(_) | DependentTier::Gra(_) => TierSplitPolicy::Drop,

        // Word-positional but no partition implementation yet. Dropping is
        // honest: attaching to first child would claim phonological / sign
        // data covering all original words, which is wrong. Add to Partition
        // explicitly when a partition impl lands for each shape.
        DependentTier::Pho(_)
        | DependentTier::Mod(_)
        | DependentTier::Sin(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_) => TierSplitPolicy::Drop,

        // Free-form / loosely-structured utterance-level annotations:
        // preserve on first child rather than silently lose.
        _ => TierSplitPolicy::AttachFirst,
    }
}

/// Build a per-child `%wor` from the parent tier by walking main-tier words
/// in lockstep with `%wor` Word-items.
///
/// Returns `None` if positional counts mismatch (stale `%wor` from prior
/// edits, or main-tier token policy drift). On `None`, the caller drops the
/// tier from all children — matching the existing stale-`%wor`-is-fine
/// behavior, never raising a validation error.
///
/// `main_word_groups` is the per-main-tier-word child-group assignment, in
/// main-tier word order, restricted to `%wor`-eligible words (the same
/// filtering `TierDomain::Wor` uses: untranscribed, fragments, and nonwords
/// are excluded; fillers are included).
fn partition_wor_tier(
    parent: &WorTier,
    main_word_groups: &[usize],
    num_groups: usize,
) -> Option<Vec<WorTier>> {
    let parent_word_count = parent.word_count();
    if parent_word_count != main_word_groups.len() {
        tracing::debug!(
            parent_wor_words = parent_word_count,
            main_eligible_words = main_word_groups.len(),
            "%wor count mismatch on split — dropping tier (stale %wor expected after prior edits)"
        );
        return None;
    }

    // Walk parent items, tracking which main-tier word index we're on for
    // Word items. Separators have no main-tier counterpart; we attach them
    // to the same child as the most recent Word, falling back to group 0
    // if we haven't seen any Word yet.
    let mut per_child: Vec<Vec<WorItem>> = vec![Vec::new(); num_groups];
    let mut next_word_idx = 0usize;
    let mut last_seen_group: Option<usize> = None;
    for item in &parent.items {
        match item {
            WorItem::Word(_) => {
                let group = main_word_groups[next_word_idx];
                last_seen_group = Some(group);
                per_child[group].push(item.clone());
                next_word_idx += 1;
            }
            WorItem::Separator { .. } => {
                let group = last_seen_group.unwrap_or(0);
                per_child[group].push(item.clone());
            }
        }
    }

    // Build a WorTier for each child. Children with empty item lists get an
    // empty WorTier; the caller filters those out (we don't emit empty
    // `%wor:` tiers).
    Some(
        per_child
            .into_iter()
            .map(|items| WorTier {
                language_code: parent.language_code.clone(),
                items,
                terminator: parent.terminator.clone(),
                bullet: parent.bullet.clone(),
                span: Span::DUMMY,
            })
            .collect(),
    )
}

/// Compute the child-group assignment for each main-tier word that is
/// `%wor`-eligible.
///
/// "Eligible" matches `TierDomain::Wor`: untranscribed words (`xxx`/`yyy`/
/// `www`), phonological fragments (`&+`), and nonwords (`&~`) are excluded;
/// fillers (`&-`) are included. The returned Vec has one entry per eligible
/// word, in main-tier order; entries are child-group indices.
///
/// Implementation: walk content_items one at a time, count `%wor`-eligible
/// words inside each via `extract::collect_utterance_content` with
/// `TierDomain::Wor`. Each such word inherits its enclosing content item's
/// group.
fn wor_eligible_word_groups(
    content_items: &[UtteranceContent],
    content_item_group: &[Option<usize>],
) -> Vec<usize> {
    let mut groups = Vec::new();
    for (content_idx, item) in content_items.iter().enumerate() {
        let mut buf = Vec::new();
        extract::collect_utterance_content(std::slice::from_ref(item), TierDomain::Wor, &mut buf);
        let group = content_item_group[content_idx].unwrap_or(0);
        for _ in &buf {
            groups.push(group);
        }
    }
    groups
}

/// Split an utterance into multiple utterances based on word assignments.
///
/// `assignments` is a Vec parallel to the extracted words, where each element
/// is the 0-based utterance ID that word belongs to.
pub fn split_utterance(utt: Utterance, assignments: &[usize]) -> Vec<Utterance> {
    let content_items = &utt.main.content.content;
    let word_to_content = build_word_to_content_map(content_items);

    if assignments.is_empty() || word_to_content.is_empty() {
        return vec![utt];
    }

    let first = assignments[0];
    if assignments.iter().all(|&a| a == first) {
        return vec![utt];
    }

    let num_content_items = content_items.len();
    let mut content_item_group: Vec<Option<usize>> = vec![None; num_content_items];

    for (word_idx, &content_idx) in word_to_content.iter().enumerate() {
        if word_idx < assignments.len() && content_item_group[content_idx].is_none() {
            content_item_group[content_idx] = Some(assignments[word_idx]);
        }
    }

    // Retraced material is excluded from the morphology word domain, so a
    // Retrace node has no direct assignment. It binds forward to the retry or
    // correction; assign it before generic back-fill can strand it with the
    // preceding child as a dangling marker.
    for idx in 0..num_content_items {
        if content_item_group[idx].is_some()
            || !matches!(content_items[idx], UtteranceContent::Retrace(_))
        {
            continue;
        }
        if let Some(next_group) = content_item_group[idx + 1..]
            .iter()
            .find_map(|group| *group)
        {
            content_item_group[idx] = Some(next_group);
        }
    }

    // Back-fill unassigned items
    let mut last_group: Option<usize> = None;
    for group in content_item_group.iter_mut() {
        if group.is_some() {
            last_group = *group;
        } else {
            *group = last_group;
        }
    }
    // Forward-fill remaining None at the start
    let mut next_group: Option<usize> = None;
    for group in content_item_group.iter_mut().rev() {
        if group.is_some() {
            next_group = *group;
        } else {
            *group = next_group;
        }
    }

    let max_group = assignments.iter().copied().max().unwrap_or(0);

    let mut groups: Vec<Vec<UtteranceContent>> = vec![Vec::new(); max_group + 1];
    for (content_idx, item) in content_items.iter().enumerate() {
        if content_item_group[content_idx].is_none() {
            tracing::warn!(
                content_idx,
                "content item has no group assignment, defaulting to group 0"
            );
        }
        let group_id = content_item_group[content_idx].unwrap_or(0);
        if group_id <= max_group {
            groups[group_id].push(item.clone());
        }
    }

    let speaker = &utt.main.speaker;
    // Capture the parent's main-tier bullet before consuming `utt`. We
    // re-attach it to the LAST child below so the original utterance's
    // end-of-span timing anchor is preserved across the split — without
    // this, every split utterance loses its `_NNN` bullet and any
    // `@Media` linkage assertion the file relied on. See
    // `docs/postmortems/2026-04-26-utseg-split-bullet-loss.md`.
    let parent_bullet = utt.main.content.bullet.clone();

    // Capture the rest of the parent's main-tier metadata so each child
    // can inherit the right slice of it. Per-field propagation policy
    // (linkers → first only, terminator/postcodes → last only, language
    // code/spans → all) is documented in
    // `docs/postmortems/2026-04-26-utseg-split-bullet-loss.md` (F1.6).
    let parent_linkers = utt.main.content.linkers.clone();
    let parent_terminator = utt.main.content.terminator.clone();
    let parent_language_code = utt.main.content.language_code.clone();
    let parent_postcodes = utt.main.content.postcodes.clone();
    let parent_main_span = utt.main.span;
    let parent_speaker_span = utt.main.speaker_span;

    // Compute per-child %wor item lists if the parent has a Wor tier and
    // counts align with main-tier wor-eligible words. None means either
    // no Wor tier was present or counts mismatched (graceful drop).
    let num_groups = max_group + 1;
    let partitioned_wor: Option<Vec<WorTier>> = utt
        .dependent_tiers
        .iter()
        .find_map(|tier| match tier {
            DependentTier::Wor(wor) => {
                let main_groups = wor_eligible_word_groups(content_items, &content_item_group);
                Some(partition_wor_tier(wor, &main_groups, num_groups))
            }
            _ => None,
        })
        .flatten();

    // Track (original_group_idx, utterance) so we can later look up the
    // partitioned %wor for each kept child even after empty/all-separator
    // groups are skipped.
    let mut result: Vec<(usize, Utterance)> = Vec::new();

    for (group_idx, mut group_content) in groups.into_iter().enumerate() {
        if group_content.is_empty() {
            continue;
        }

        // Strip leading Separator nodes (comma, tag, vocative) that landed
        // at the start of this group after the split. A Separator at
        // utterance-initial position is invalid CHAT — it belongs with the
        // preceding content or should be dropped.
        let first_non_sep = group_content
            .iter()
            .position(|item| !matches!(item, UtteranceContent::Separator(_)))
            .unwrap_or(group_content.len());
        if first_non_sep > 0 {
            group_content.drain(..first_non_sep);
        }
        if group_content.is_empty() {
            continue;
        }

        let mut main = MainTier::new(
            speaker.clone(),
            group_content,
            Terminator::Period { span: Span::DUMMY },
        );
        // Language code applies to every child (utterance-scope) — set
        // it at construction time. Linkers, terminator, postcodes, and
        // bullet are positional and applied to the right child after
        // the loop.
        if let Some(ref lang) = parent_language_code {
            main.content = main.content.with_language_code(lang.clone());
        }
        // Source spans: inherit the parent's so children retain a
        // useful (if coarse) source pointer instead of `Span::DUMMY`.
        main.span = parent_main_span;
        main.speaker_span = parent_speaker_span;
        let new_utt = Utterance::new(main);
        result.push((group_idx, new_utt));
    }

    if result.is_empty() {
        tracing::warn!("utterance segmentation produced no groups, returning original");
        return vec![utt];
    }

    // Per-tier policy. Walk the parent's dependent tiers once, dispatching
    // each to its policy. `partitioned_wor` (if Some) is the precomputed
    // per-group payload; AttachFirst tiers go to result[0]; Drop tiers
    // produce no output.
    let parent_dep_tiers = utt.dependent_tiers.clone();
    let mut wor_index: Option<usize> = None;
    for (i, tier) in parent_dep_tiers.iter().enumerate() {
        if matches!(tier, DependentTier::Wor(_)) {
            wor_index = Some(i);
            break;
        }
    }

    // Attach partitioned %wor to each child whose group has any items.
    if let Some(wor_idx) = wor_index
        && let Some(per_group) = partitioned_wor
    {
        let _ = wor_idx; // kept for symmetry / future re-ordering needs
        for (group_idx, child) in result.iter_mut() {
            if let Some(child_wor) = per_group.get(*group_idx)
                && !child_wor.items.is_empty()
            {
                child
                    .dependent_tiers
                    .push(DependentTier::Wor(child_wor.clone()));
            }
        }
    }

    if let Some((_, first_child)) = result.first_mut() {
        for tier in &parent_dep_tiers {
            if matches!(policy_for_tier(tier), TierSplitPolicy::AttachFirst) {
                first_child.dependent_tiers.push(tier.clone());
            }
        }
    }

    // Linkers go on the FIRST child only — they describe relation to the
    // *prior* (different) utterance, which only the first piece is adjacent
    // to. Use a non-empty check so we don't bother cloning the empty
    // SmallVec for the common case.
    if !parent_linkers.is_empty()
        && let Some((_, first_child)) = result.first_mut()
    {
        first_child.main.content.linkers = parent_linkers;
    }

    // Terminator and postcodes go on the LAST child only. Terminator
    // describes how the original utterance ended — that's the last child.
    // Postcodes are utterance-level analysis tags; placing them on the
    // last child matches the conventional after-terminator serialization.
    if let Some((_, last)) = result.last_mut() {
        if let Some(term) = parent_terminator {
            last.main.content.terminator = Some(term);
        }
        if !parent_postcodes.is_empty() {
            last.main.content.postcodes = parent_postcodes;
        }
    }

    // Re-attach the parent's main-tier bullet to the LAST child. The parent's
    // end_ms correctly describes the last child's end timestamp; we make no
    // fabricated claim about non-last children. F2 (proportional UTR hints
    // across all children) is a future refinement.
    if let Some(bullet) = parent_bullet
        && let Some((_, last)) = result.last_mut()
    {
        last.main.content.bullet = Some(bullet);
    }

    result.into_iter().map(|(_, u)| u).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::{DependentTier, Terminator, UtteranceContent, WorTier, WriteChat};
    use talkbank_parser::TreeSitterParser;

    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().unwrap();
        parser.parse_chat_file(text).unwrap()
    }

    fn get_utterance(chat: &ChatFile, idx: usize) -> &Utterance {
        let mut utt_idx = 0;
        for line in &chat.lines.0 {
            if let Line::Utterance(utt) = line {
                if utt_idx == idx {
                    return utt;
                }
                utt_idx += 1;
            }
        }
        panic!("Utterance {idx} not found");
    }

    fn count_utterances(chat: &ChatFile) -> usize {
        chat.lines
            .iter()
            .filter(|l| matches!(l, Line::Utterance(_)))
            .count()
    }

    #[test]
    fn test_split_no_change() {
        let chat_text = include_str!("../../../../resources/fixtures/eng_i_eat_cookies.cha");
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();
        let result = split_utterance(utt, &[0, 0, 0]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_split_two_groups() {
        let chat_text =
            include_str!("../../../../resources/fixtures/eng_i_eat_cookies_and_he_likes_cake.cha");
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();
        let result = split_utterance(utt, &[0, 0, 0, 1, 1, 1, 1]);
        assert_eq!(result.len(), 2);

        let out0 = result[0].to_chat_string();
        let out1 = result[1].to_chat_string();
        assert!(out0.contains("I eat cookies"), "First split: {out0}");
        assert!(out1.contains("and he likes cake"), "Second split: {out1}");
    }

    #[test]
    fn test_collect_utseg_payloads() {
        // 3 utterances: 1 single-word, 2 multi-word
        let chat_text = include_str!("../../../../resources/fixtures/eng_three_utterances.cha");
        let chat = parse_chat(chat_text);
        let collected = collect_utseg_payloads(&chat);
        let payloads = &collected.batch_items;

        // Single-word utterance "hello" should be classified NotApplicable.
        assert_eq!(payloads.len(), 2);
        assert_eq!(collected.not_applicable.len(), 1);
        match &collected.not_applicable[0].kind {
            UtsegOutcomeKind::NotApplicable { reason } => {
                assert_eq!(*reason, UtsegNotApplicableReason::SingleWord);
            }
            other => panic!("expected NotApplicable(SingleWord), got {other:?}"),
        }
        assert_eq!(payloads[0].0, 1); // utt_ordinal of "I eat cookies"
        assert_eq!(payloads[0].1.words, vec!["I", "eat", "cookies"]);
        assert_eq!(payloads[0].1.text, "I eat cookies");
        assert_eq!(payloads[1].0, 2); // utt_ordinal of "he likes cake too"
        assert_eq!(payloads[1].1.words, vec!["he", "likes", "cake", "too"]);
    }

    #[test]
    fn test_apply_utseg_results() {
        let chat_text =
            include_str!("../../../../resources/fixtures/eng_i_eat_cookies_and_he_likes_cake.cha");
        let mut chat = parse_chat(chat_text);
        assert_eq!(count_utterances(&chat), 1);

        let mut assignment_map = HashMap::new();
        assignment_map.insert(0, vec![0, 0, 0, 1, 1, 1, 1]);

        apply_utseg_results(&mut chat, &assignment_map);
        assert_eq!(count_utterances(&chat), 2);

        let out0 = get_utterance(&chat, 0).to_chat_string();
        let out1 = get_utterance(&chat, 1).to_chat_string();
        assert!(out0.contains("I eat cookies"), "First: {out0}");
        assert!(out1.contains("and he likes cake"), "Second: {out1}");
    }

    #[test]
    fn test_apply_utseg_empty_map() {
        let chat_text = include_str!("../../../../resources/fixtures/eng_i_eat_cookies.cha");
        let mut chat = parse_chat(chat_text);
        let original_count = count_utterances(&chat);

        apply_utseg_results(&mut chat, &HashMap::new());
        assert_eq!(count_utterances(&chat), original_count);
    }

    /// After utseg splits, no utterance should start with a Separator node.
    ///
    /// Rev.AI returns "dishes , or she didn't order them" and Stanza puts
    /// the boundary after "dishes". The comma is correctly modeled as
    /// `UtteranceContent::Separator(Separator::Comma)` by build_chat.rs.
    /// But after the split, it lands as the first content item of the second
    /// utterance — which is invalid CHAT. Leading separators must be stripped.
    ///
    /// Bug report: a user, 2026-04-02, 25-3.cha — `*INV: , or she didn't...`
    #[test]
    fn utseg_split_strips_leading_separator() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tINV Investigator\n\
            @ID:\teng|test|INV|||||Investigator|||\n\
            *INV:\tshe's washing dishes , or she didn't order them .\n@End\n";
        let mut chat = parse_chat(chat_text);

        // Stanza boundary: words 0-2 = group 0, words 3-7 = group 1.
        // The comma separator sits between groups.
        let mut assignment_map = HashMap::new();
        assignment_map.insert(0, vec![0, 0, 0, 1, 1, 1, 1, 1]);

        apply_utseg_results(&mut chat, &assignment_map);

        // Verify the split produced two utterances
        let utt_count = count_utterances(&chat);
        assert!(
            utt_count >= 2,
            "expected at least 2 utterances, got {utt_count}"
        );

        // No utterance's first content item should be a Separator
        for (i, line) in chat.lines.iter().enumerate() {
            if let Line::Utterance(u) = line
                && let Some(first) = u.main.content.content.first()
            {
                assert!(
                    !matches!(first, UtteranceContent::Separator(_)),
                    "utterance at line {i} starts with a Separator node \
                         (should have been stripped): {}",
                    u.to_chat_string()
                );
            }
        }
    }

    /// REGRESSION: the parent's main-tier timing bullet must not be silently
    /// dropped when an utterance is split.
    ///
    /// The utseg pipeline produced bullet-less output across 854/885 MOST
    /// corpus files on 2026-04-26 because `split_utterance` constructed each
    /// child's `MainTier` fresh via `MainTier::new(...)` — which sets
    /// `TierContent.bullet = None` — without copying `utt.main.content.bullet`
    /// from the parent. The aggregate signal: 223,277 → 152,192 bullets
    /// (−31.8%) corpus-wide. Files whose only timing came from to-be-split
    /// utterances ended up with no timing at all and tripped E544
    /// (@Media-linkage assertion).
    ///
    /// Conservative invariant tested here: at least one child of a split
    /// must carry the parent's bullet. We attach it to the LAST child —
    /// the original utterance ended at the bullet's end timestamp, and
    /// the last child of the split contains the last words and ends at
    /// that same end timestamp.
    ///
    /// See: docs/postmortems/2026-04-26-utseg-split-bullet-loss.md
    #[test]
    fn utseg_split_preserves_parent_bullet_on_last_child() {
        // Bullet syntax: NAK-delimited "start_end" appended after the
        // terminator. \u{15} is NAK (0x15). Real example from MOST:
        // `*PAR0: ... . 0_668430` (the 0_668430 is the bullet).
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI eat cookies and he likes cake . \u{15}1000_5000\u{15}\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();

        // Sanity-check the fixture: the parent utterance carries a bullet.
        assert!(
            utt.main.content.bullet.is_some(),
            "fixture pre-condition: parent must have a bullet"
        );
        let parent_bullet = utt.main.content.bullet.as_ref().unwrap().clone();

        // Split into two children: words 0-2 → child 0, words 3-6 → child 1.
        let result = split_utterance(utt, &[0, 0, 0, 1, 1, 1, 1]);
        assert_eq!(result.len(), 2, "expected 2 children from the split");

        // The LAST child must carry the parent's bullet (same start_ms /
        // end_ms). The earlier children may have no bullet — we simply
        // don't know their per-child timing without realignment, and we
        // refuse to fabricate one.
        let last_child_bullet = &result.last().unwrap().main.content.bullet;
        assert!(
            last_child_bullet.is_some(),
            "last child of a split must inherit the parent's bullet \
             (got None — parent timing was dropped). Output: {}",
            result.last().unwrap().to_chat_string()
        );
        let last = last_child_bullet.as_ref().unwrap();
        assert_eq!(
            last.timing.start_ms, parent_bullet.timing.start_ms,
            "last child's bullet start must equal parent's"
        );
        assert_eq!(
            last.timing.end_ms, parent_bullet.timing.end_ms,
            "last child's bullet end must equal parent's"
        );
    }

    /// %wor partitioning: when a parent has %wor with timing for every
    /// main-tier word, splitting must distribute the WorItems to the
    /// children matching their words. F1.5: BA2-equivalent per-word
    /// timing preservation across split.
    #[test]
    fn utseg_split_partitions_wor_tier_across_children() {
        // 4-word utterance with %wor giving each word its own timing.
        // Split 2/2: child 0 gets words "I eat", child 1 gets "the cookies".
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI eat the cookies . \u{15}0_4000\u{15}\n\
            %wor:\tI \u{15}0_500\u{15} eat \u{15}500_1500\u{15} the \u{15}1500_2200\u{15} \
            cookies \u{15}2200_4000\u{15} .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        // Sanity: parent has 4 wor items
        let parent_wor = parent
            .dependent_tiers
            .iter()
            .find_map(|t| match t {
                DependentTier::Wor(w) => Some(w),
                _ => None,
            })
            .expect("fixture should parse a %wor tier");
        assert_eq!(parent_wor.word_count(), 4, "fixture sanity check");

        let result = split_utterance(parent, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2);

        let wor_of = |u: &Utterance| -> Option<WorTier> {
            u.dependent_tiers.iter().find_map(|t| match t {
                DependentTier::Wor(w) => Some(w.clone()),
                _ => None,
            })
        };
        let child0_wor = wor_of(&result[0]).expect("child 0 must carry %wor");
        let child1_wor = wor_of(&result[1]).expect("child 1 must carry %wor");
        assert_eq!(
            child0_wor.word_count(),
            2,
            "child 0 should carry 2 wor words (I, eat)"
        );
        assert_eq!(
            child1_wor.word_count(),
            2,
            "child 1 should carry 2 wor words (the, cookies)"
        );
    }

    /// %wor partitioning falls back to dropping the tier when item counts
    /// don't match main-tier eligible-word counts (stale %wor). No panic,
    /// no validation error — silent drop matches the rename's intent that
    /// stale %wor is legal.
    #[test]
    fn utseg_split_drops_wor_on_count_mismatch() {
        // Parent has 4 main-tier words, but only 3 wor items (stale).
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI eat the cookies .\n\
            %wor:\tI \u{15}0_500\u{15} eat \u{15}500_1500\u{15} cookies \u{15}1500_2000\u{15} .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        let result = split_utterance(parent, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2);

        for (i, child) in result.iter().enumerate() {
            let has_wor = child
                .dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Wor(_)));
            assert!(
                !has_wor,
                "child {i} should not carry %wor when parent counts mismatched (graceful drop)"
            );
        }
    }

    /// %mor and %gra are dropped on split. Their analysis depends on
    /// utterance-scope context and is invalidated by re-segmentation;
    /// the user reruns morphotag to regenerate.
    #[test]
    fn utseg_split_drops_mor_and_gra() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI eat the cookies .\n\
            %mor:\tpron|I v|eat det|the n|cookies .\n\
            %gra:\t1|2|SUBJ 2|0|ROOT 3|4|DET 4|2|OBJ 5|2|PUNCT\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        let result = split_utterance(parent, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2);

        for (i, child) in result.iter().enumerate() {
            for tier in &child.dependent_tiers {
                assert!(
                    !matches!(tier, DependentTier::Mor(_) | DependentTier::Gra(_)),
                    "child {i} should not carry %mor or %gra after split (dropped by policy)"
                );
            }
        }
    }

    /// %com and other free-form / utterance-level annotations attach to
    /// the first child. Strictly better than BA2's silent drop.
    #[test]
    fn utseg_split_attaches_com_to_first_child() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI eat the cookies .\n\
            %com:\tchild was excited\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        let result = split_utterance(parent, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2);

        let first_has_com = result[0]
            .dependent_tiers
            .iter()
            .any(|t| matches!(t, DependentTier::Com(_)));
        let second_has_com = result[1]
            .dependent_tiers
            .iter()
            .any(|t| matches!(t, DependentTier::Com(_)));
        assert!(first_has_com, "first child must inherit the %com");
        assert!(!second_has_com, "second child must not carry %com");
    }

    /// Terminator propagation: the LAST child inherits the parent's
    /// terminator; non-last children get the default `Period`. This
    /// preserves quote-introducer linkage (`+"/.` parent → next-utterance
    /// `+"` quoted speech), interruption markers, and any other
    /// non-default terminator. See resources/spec/errors/E341_auto.md for the
    /// `+"/.` ↔ `+"` validation pairing.
    #[test]
    fn utseg_split_inherits_terminator_on_last_child() {
        // Parent ends with +"/. (quote-introducer). Real shape from
        // childes-eng-na-data/Eng-NA/Kuczaj/030115.cha.
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tand he says +\"/.\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        // Sanity: parent terminator is the quote-introducer, not Period.
        let parent_term = parent
            .main
            .content
            .terminator
            .as_ref()
            .expect("fixture must have a terminator");
        assert!(
            !matches!(parent_term, Terminator::Period { .. }),
            "fixture sanity: parent must end in +\"/. (non-default), got {parent_term:?}"
        );

        let result = split_utterance(parent, &[0, 0, 1]);
        assert_eq!(result.len(), 2, "expected 2 children");

        let first_term = result[0]
            .main
            .content
            .terminator
            .as_ref()
            .expect("child 0 must have a terminator");
        assert!(
            matches!(first_term, Terminator::Period { .. }),
            "non-last child must default to Period, got {first_term:?}"
        );

        let last_term = result[1]
            .main
            .content
            .terminator
            .as_ref()
            .expect("last child must have a terminator");
        assert!(
            !matches!(last_term, Terminator::Period { .. }),
            "LAST child must inherit the parent's non-default terminator, got {last_term:?}"
        );
    }

    /// Linker propagation: the FIRST child inherits the parent's
    /// linkers; non-first children get none. Linkers describe the
    /// utterance's relationship to the *prior* (different) utterance,
    /// so only the first split-piece is adjacent to that prior turn.
    #[test]
    fn utseg_split_inherits_linkers_on_first_child() {
        // Parent starts with `+,` (SelfCompletion linker). Real shape
        // from clan-info/examples/Adler/adler15a.cha.
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tINV Investigator\n\
            @ID:\teng|test|INV|||||Investigator|||\n\
            *INV:\t+, with that letter and one more thing .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        // Sanity: parent has at least one linker.
        assert!(
            !parent.main.content.linkers.is_empty(),
            "fixture sanity: parent must carry at least one linker"
        );
        let parent_linkers_len = parent.main.content.linkers.0.len();
        assert!(parent_linkers_len > 0);

        let result = split_utterance(parent, &[0, 0, 0, 1, 1, 1, 1]);
        assert_eq!(result.len(), 2, "expected 2 children");

        assert_eq!(
            result[0].main.content.linkers.0.len(),
            parent_linkers_len,
            "FIRST child must inherit the parent's linkers"
        );
        assert!(
            result[1].main.content.linkers.is_empty(),
            "non-first child must have no linkers"
        );
    }

    /// Language code propagation: utterance-level `[- code]` applies
    /// to all of the utterance's words, so every child carries it.
    #[test]
    fn utseg_split_propagates_language_code_to_all_children() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng, spa\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\t[- spa] hola amigo y como estas .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        let parent_lang = parent
            .main
            .content
            .language_code
            .clone()
            .expect("fixture must parse [- spa] language code");

        let result = split_utterance(parent, &[0, 0, 1, 1, 1, 1]);
        assert!(result.len() >= 2);

        for (i, child) in result.iter().enumerate() {
            assert_eq!(
                child.main.content.language_code.as_ref(),
                Some(&parent_lang),
                "child {i} must carry the parent's [- spa] language code"
            );
        }
    }

    /// Postcode propagation: utterance-level `[+ exc]` and similar
    /// analysis tags attach to the LAST child only. They describe the
    /// original utterance as a unit; placing them on the last child
    /// (where they serialize after the terminator) keeps each tag
    /// attached exactly once and matches the conventional position.
    #[test]
    fn utseg_split_inherits_postcodes_on_last_child() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tone two three four . [+ exc]\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();

        let parent_postcode_len = parent.main.content.postcodes.0.len();
        assert!(
            parent_postcode_len > 0,
            "fixture sanity: parent must carry at least one postcode"
        );

        let result = split_utterance(parent, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2, "expected 2 children");

        assert!(
            result[0].main.content.postcodes.is_empty(),
            "non-last child must have no postcodes"
        );
        assert_eq!(
            result[1].main.content.postcodes.0.len(),
            parent_postcode_len,
            "LAST child must inherit the parent's postcodes"
        );
    }

    /// Replaced-word handling: a `ReplacedWord(wanna [: want to])` is one
    /// main-tier slot but contributes N replacement words to TierDomain::Mor
    /// (the BERT classifier sees N words). `split_utterance` builds its
    /// word→content mapping with TierDomain::Mor too, so the assignment
    /// vector lengths match. The first-assignment-wins logic in
    /// `split_utterance` correctly attributes each ReplacedWord to ONE
    /// child group regardless of where the boundary lands relative to
    /// the replacement words.
    ///
    /// Coverage gap discovered 2026-04-27 while writing replacements
    /// docs (`book/src/architecture/replacements-handling.md`); analogous
    /// to the FA bug shape from 2026-04-08. The current code is correct
    /// by construction (extract + split both use TierDomain::Mor); these
    /// tests pin that invariant against future drift.
    #[test]
    fn utseg_split_handles_replaced_word_boundary_before() {
        // Boundary BEFORE the ReplacedWord. Mor walks 4 words: I, want, to, go.
        // assignments=[0, 1, 1, 1] → "I" alone in group 0; "wanna" + "go" in group 1.
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI wanna [: want to] go .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();
        let result = split_utterance(utt, &[0, 1, 1, 1]);
        assert_eq!(result.len(), 2, "expected 2 children");
        let s0 = result[0].to_chat_string();
        let s1 = result[1].to_chat_string();
        // child 0: just "I" — no fragment of the ReplacedWord or "go"
        assert!(
            !s0.contains("wanna") && !s0.contains("want") && !s0.contains("go"),
            "child 0 should be just I, got: {s0}"
        );
        // child 1: ReplacedWord preserved intact, plus "go"
        assert!(
            s1.contains("wanna [: want to]"),
            "child 1 should keep the ReplacedWord intact, got: {s1}"
        );
        assert!(s1.contains("go"), "child 1 should contain go, got: {s1}");
    }

    #[test]
    fn utseg_split_handles_replaced_word_boundary_after() {
        // Boundary AFTER the ReplacedWord. assignments=[0, 0, 0, 1] →
        // "I wanna" in group 0; "go" alone in group 1. The ReplacedWord
        // (one main-tier slot) lands fully in group 0.
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI wanna [: want to] go .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();
        let result = split_utterance(utt, &[0, 0, 0, 1]);
        assert_eq!(result.len(), 2);
        let s0 = result[0].to_chat_string();
        let s1 = result[1].to_chat_string();
        assert!(
            s0.contains("wanna [: want to]"),
            "child 0 should keep the ReplacedWord intact, got: {s0}"
        );
        assert!(
            !s1.contains("wanna") && !s1.contains("want"),
            "child 1 should not have any ReplacedWord fragment, got: {s1}"
        );
        assert!(s1.contains("go"), "child 1 should contain go, got: {s1}");
    }

    #[test]
    fn utseg_split_attributes_replaced_word_atomically_on_inconsistent_split() {
        // BERT puts the boundary BETWEEN the replacement words "want" and
        // "to" (assignments=[0, 0, 1, 1]). Structurally we cannot split
        // inside a single main-tier slot. The first-assignment-wins logic
        // attributes the entire ReplacedWord to "want"'s group (0). Net
        // result: "I wanna" in group 0, "go" in group 1 — same as if the
        // boundary had been after the ReplacedWord. This pins the
        // "ReplacedWord is atomic to splits" invariant.
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\tI wanna [: want to] go .\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let utt = get_utterance(&chat, 0).clone();
        let result = split_utterance(utt, &[0, 0, 1, 1]);
        assert_eq!(result.len(), 2);
        let s0 = result[0].to_chat_string();
        let s1 = result[1].to_chat_string();
        // ReplacedWord stayed atomic — went with "want"'s assignment (0).
        assert!(
            s0.contains("wanna [: want to]"),
            "ReplacedWord must be atomic on splits; got s0: {s0}"
        );
        assert!(
            !s1.contains("wanna") && !s1.contains("want"),
            "child 1 should not contain any fragment of the ReplacedWord, got: {s1}"
        );
        assert!(s1.contains("go"), "child 1 should contain go, got: {s1}");
    }

    #[test]
    fn utseg_split_preserves_metadata_while_replaced_word_stays_atomic() {
        let chat_text = "@UTF8\n@Begin\n@Languages:\teng, spa\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|test|CHI|||||Child|||\n\
            *CHI:\t+, [- spa] I wanna [: want to] go now . [+ exc] \u{15}1000_5000\u{15}\n\
            @End\n";
        let chat = parse_chat(chat_text);
        let parent = get_utterance(&chat, 0).clone();
        let parent_span = parent.main.span;
        let parent_speaker_span = parent.main.speaker_span;
        let parent_language = parent.main.content.language_code.clone();
        let parent_bullet = parent.main.content.bullet.clone();

        // TierDomain::Mor sees I, want, to, go, now. The group boundary is
        // deliberately inconsistent inside the replacement phrase.
        let children = split_utterance(parent, &[0, 0, 1, 1, 1]);
        assert_eq!(children.len(), 2);

        let first = &children[0];
        let last = &children[1];
        let first_text = first.to_chat_string();
        let last_text = last.to_chat_string();

        assert!(!first.main.content.linkers.is_empty());
        assert!(last.main.content.linkers.is_empty());
        assert_eq!(first.main.content.language_code, parent_language);
        assert_eq!(last.main.content.language_code, parent_language);
        assert_eq!(first.main.span, parent_span);
        assert_eq!(last.main.span, parent_span);
        assert_eq!(first.main.speaker_span, parent_speaker_span);
        assert_eq!(last.main.speaker_span, parent_speaker_span);
        assert!(first.main.content.postcodes.is_empty());
        assert!(!last.main.content.postcodes.is_empty());
        assert!(first.main.content.bullet.is_none());
        assert_eq!(last.main.content.bullet, parent_bullet);
        assert!(first_text.contains("wanna [: want to]"));
        assert!(!last_text.contains("wanna") && !last_text.contains("want"));
    }

    #[test]
    fn utseg_split_keeps_retrace_with_following_retry() {
        let chat = parse_chat(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n\
             @ID:\teng|test|PAR|||||Participant|||\n\
             *PAR:\tmud dig [/] dig [/] dig fire .\n@End\n",
        );
        let result = split_utterance(get_utterance(&chat, 0).clone(), &[0, 1, 1]);
        assert_eq!(result.len(), 2);

        for child in result {
            let output = child.to_chat_string();
            for marker in ["[/]", "[//]", "[///]"] {
                let mut remainder = output.as_str();
                while let Some((_, after)) = remainder.split_once(marker) {
                    assert!(
                        !after.trim_start().starts_with(['.', '?', '!']),
                        "retrace marker was stranded: {output}"
                    );
                    remainder = after;
                }
            }
        }
    }

    #[test]
    fn snapshot_utseg_batch_item() {
        let item = UtsegBatchItem {
            words: vec!["I".into(), "eat".into(), "cookies".into()],
            text: "I eat cookies".into(),
        };
        insta::assert_json_snapshot!(item, @r#"
        {
          "words": [
            "I",
            "eat",
            "cookies"
          ],
          "text": "I eat cookies"
        }
        "#);
    }

    #[test]
    fn snapshot_utseg_response() {
        let resp = UtsegResponse {
            assignments: vec![0, 0, 0, 1, 1, 1, 1],
        };
        insta::assert_json_snapshot!(resp, @r#"
        {
          "assignments": [
            0,
            0,
            0,
            1,
            1,
            1,
            1
          ]
        }
        "#);
    }

    // ---------------------------------------------------------------------
    // Wave 5 outcome-classification tests
    // ---------------------------------------------------------------------

    #[test]
    fn validate_utseg_response_aligned_matching_counts() {
        let item = UtsegBatchItem {
            words: vec!["I".into(), "eat".into(), "cookies".into()],
            text: "I eat cookies".into(),
        };
        let resp = UtsegResponse {
            assignments: vec![0, 0, 0],
        };
        match validate_utseg_response(&item, &resp) {
            UtsegOutcomeKind::Aligned {
                n_words,
                n_segments,
            } => {
                assert_eq!(n_words, 3);
                assert_eq!(n_segments, 1, "all same group = 1 segment");
            }
            other => panic!("expected Aligned, got {other:?}"),
        }
    }

    #[test]
    fn validate_utseg_response_counts_distinct_segments() {
        let item = UtsegBatchItem {
            words: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            text: "a b c d".into(),
        };
        let resp = UtsegResponse {
            assignments: vec![0, 0, 1, 1],
        };
        match validate_utseg_response(&item, &resp) {
            UtsegOutcomeKind::Aligned { n_segments, .. } => assert_eq!(n_segments, 2),
            other => panic!("expected Aligned(2 segments), got {other:?}"),
        }
    }

    #[test]
    fn validate_utseg_response_length_mismatch_is_misalignment_bug() {
        let item = UtsegBatchItem {
            words: vec!["I".into(), "eat".into(), "cookies".into()],
            text: "I eat cookies".into(),
        };
        // Worker returned 2 assignments for 3 input words — contract violation.
        let resp = UtsegResponse {
            assignments: vec![0, 0],
        };
        match validate_utseg_response(&item, &resp) {
            UtsegOutcomeKind::MisalignmentBug(diag) => {
                assert_eq!(diag.expected_assignments, 3);
                assert_eq!(diag.actual_assignments, 2);
                assert_eq!(diag.words, vec!["I", "eat", "cookies"]);
            }
            other => panic!("expected MisalignmentBug, got {other:?}"),
        }
    }

    #[test]
    fn collect_utseg_emits_not_applicable_for_single_word() {
        let chat_text = include_str!("../../../../resources/fixtures/eng_three_utterances.cha");
        let chat = parse_chat(chat_text);
        let collected = collect_utseg_payloads(&chat);

        assert_eq!(
            collected.not_applicable.len(),
            1,
            "expected the single-word utterance (\"hello\") to be classified NotApplicable",
        );
        let outcome = &collected.not_applicable[0];
        match &outcome.kind {
            UtsegOutcomeKind::NotApplicable { reason } => {
                assert_eq!(*reason, UtsegNotApplicableReason::SingleWord);
            }
            other => panic!("expected NotApplicable(SingleWord), got {other:?}"),
        }
    }

    #[test]
    fn utseg_outcome_to_decision_record_aligned_is_none() {
        let outcome = UtsegOutcome {
            utt_ordinal: 0,
            speaker: SpeakerCode::new("CHI"),
            kind: UtsegOutcomeKind::Aligned {
                n_words: 3,
                n_segments: 1,
            },
        };
        assert!(outcome.to_decision_record(5).is_none());
    }

    #[test]
    fn utseg_outcome_to_decision_record_misalignment_bug_flags_review() {
        let outcome = UtsegOutcome {
            utt_ordinal: 0,
            speaker: SpeakerCode::new("CHI"),
            kind: UtsegOutcomeKind::MisalignmentBug(UtsegMisalignmentDiagnostic {
                expected_assignments: 3,
                actual_assignments: 2,
                words: vec!["hello".into(), "world".into(), "bye".into()],
            }),
        };
        let record = outcome.to_decision_record(5).expect("record for bug");
        assert!(matches!(
            record.strategy,
            crate::decisions::DecisionStrategy::Utseg(
                crate::decisions::UtsegStrategy::MisalignmentBug
            )
        ));
        assert!(record.needs_review);
        assert!(record.reason.contains("expected_assignments=3"));
        assert!(record.reason.contains("actual_assignments=2"));
    }
}
