//! Pipeline decision provenance: tracking machine decisions for user review.
//!
//! Every batchalign3 command makes decisions that alter output: clamping
//! timestamps, stripping timing, skipping utterances, defaulting values,
//! normalizing text. These decisions are currently logged via `tracing` but
//! invisible to the user in the output CHAT file.
//!
//! This module defines `DecisionRecord` — a structured representation of a
//! machine decision that can be injected as `%xalign` / `%xrev` tiers so
//! users can review what the pipeline did and why.
//!
//! # Architecture
//!
//! Each pipeline stage (FA, UTR, morphosyntax, utseg, etc.) collects
//! `Vec<DecisionRecord>` during processing. The command orchestrator passes
//! them to [`inject_decision_tiers()`] before serialization. The
//! `--review-level` flag controls emission:
//!
//! - `None` — no decision tiers
//! - `LowConfidence` — only decisions with `needs_review = true`
//! - `All` — every decision
//!
//! The existing `%xalign` / `%xrev` tier format from `fa/review_tiers.rs`
//! is reused — `DecisionRecord` is a generalization of `RepairDecision`.

use talkbank_model::Span;
use talkbank_model::model::{
    ChatFile, DependentTier, Line, NonEmptyString, UserDefinedDependentTier,
};

/// Which pipeline module made the decision.
///
/// Derivable from a [`DecisionStrategy`] via [`DecisionStrategy::module`],
/// but retained as its own type for call sites that want to filter or
/// display by module without caring about the specific strategy variant
/// (e.g. "show me all FA decisions").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionModule {
    /// Forced alignment (grouping, injection, postprocessing).
    Fa,
    /// Utterance timing recovery.
    Utr,
    /// Monotonicity enforcement (end-time clamping, start-time stripping).
    Monotonicity,
    /// Morphosyntax (Stanza mapping, retokenization).
    Morphosyntax,
    /// Coreference resolution (sparse `%xcoref` injection).
    Coref,
    /// Utterance segmentation.
    Utseg,
}

impl DecisionModule {
    /// Short label for `%xalign` tier output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fa => "fa",
            Self::Utr => "utr",
            Self::Monotonicity => "monotonicity",
            Self::Morphosyntax => "morphosyntax",
            Self::Coref => "coref",
            Self::Utseg => "utseg",
        }
    }
}

// ---------------------------------------------------------------------------
// Typed decision strategies
//
// Every strategy the pipeline can emit is declared as a variant of one of
// the per-module enums below, then wrapped in [`DecisionStrategy`] at the
// boundary with [`DecisionRecord`]. This replaces the previous stringly
// typed `strategy: &'static str` field so that:
//
// - Typos at construction sites fail to compile instead of producing a
//   novel strategy label consumers silently can't match.
// - Consumers can match exhaustively on the strategy set per module.
// - Adding a new strategy requires declaring its name in exactly one
//   place, and serialization + tracing derive from that declaration.
//
// The `as_str()` name on each per-module enum is the *label* that was
// previously typed as a string literal. Migration rule: if downstream
// consumers read `record.strategy == "end_clamped"`, their new read is
// `matches!(record.strategy, DecisionStrategy::Monotonicity(MonotonicityStrategy::EndClamped))`.
// ---------------------------------------------------------------------------

/// Forced-alignment repair strategies (`fa::repair`, `fa::orchestrate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaStrategy {
    /// Same-speaker gap filling narrowed an overlap into a gap-fill.
    GapFilled,
    /// Two bullets' overlap was split at the midpoint.
    BoundaryAveraged,
    /// Longest-increasing-subsequence selective timing removal.
    LisRemoval,
    /// Utterance-bullet timing was stripped under a monotonicity violation.
    TimingStripped,
    /// Per-word timings were dropped (e.g. clamped to utterance boundary).
    WordsTimingDropped,
    /// A too-narrow utterance bullet was expanded to fit its word count.
    NarrowBulletRescued,
}

impl FaStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GapFilled => "gap_filled",
            Self::BoundaryAveraged => "boundary_averaged",
            Self::LisRemoval => "lis_removal",
            Self::TimingStripped => "timing_stripped",
            Self::WordsTimingDropped => "words_timing_dropped",
            Self::NarrowBulletRescued => "narrow_bullet_rescued",
        }
    }
}

/// Utterance-timing-recovery (UTR) strategies (`fa::utr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtrStrategy {
    /// Untimed utterance matched a zero-duration span and was left alone.
    ZeroDurationSkipped,
    /// No ASR alignment found for an untimed utterance.
    Unmatched,
}

impl UtrStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZeroDurationSkipped => "zero_duration_skipped",
            Self::Unmatched => "unmatched",
        }
    }
}

/// Monotonicity-enforcement strategies applied to utterance bullets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonotonicityStrategy {
    /// End time was clamped down to fit under the next utterance's start.
    EndClamped,
    /// Bullet timing stripped because monotonicity could not be restored.
    TimingStripped,
}

impl MonotonicityStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EndClamped => "end_clamped",
            Self::TimingStripped => "timing_stripped",
        }
    }
}

/// Morphosyntax (Stanza mapping / injection) strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphosyntaxStrategy {
    /// Utterance had no Mor-alignable content.
    NotApplicable,
    /// 1-to-1 invariant violated post-mapping.
    MisalignmentBug,
    /// UD→Mor mapping returned an error (e.g. multi-root UD).
    MappingFailed,
    /// Stanza retokenization rewrite failed.
    RetokenizationFailed,
    /// `inject_morphosyntax` rejected the utterance (re-raised as a decision).
    InjectionFailed,
    /// Stanza returned zero sentences for the dispatched utterance.
    NlpNoSentences,
}

impl MorphosyntaxStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::MisalignmentBug => "misalignment_bug",
            Self::MappingFailed => "mapping_failed",
            Self::RetokenizationFailed => "retokenization_failed",
            Self::InjectionFailed => "injection_failed",
            Self::NlpNoSentences => "nlp_no_sentences",
        }
    }
}

/// Utterance-segmentation strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtsegStrategy {
    /// Single-word or empty utterance — not dispatched.
    NotApplicable,
    /// Worker returned the wrong number of assignments.
    MisalignmentBug,
}

impl UtsegStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::MisalignmentBug => "misalignment_bug",
        }
    }
}

/// Coreference-injection strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorefStrategy {
    /// Worker returned a sentence_idx that doesn't map to a valid line.
    SentenceIndexOutOfBounds,
    /// `%xcoref` tier construction failed (NonEmptyString, etc.).
    InjectionFailed,
}

impl CorefStrategy {
    /// Stable wire/tracing label for `%xalign` output and structured tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SentenceIndexOutOfBounds => "sentence_index_out_of_bounds",
            Self::InjectionFailed => "injection_failed",
        }
    }
}

/// The typed strategy carried by a [`DecisionRecord`].
///
/// Subsumes the previous `(module: DecisionModule, strategy: &'static str)`
/// pair into a single enum. [`DecisionStrategy::module`] recovers the
/// module when a consumer wants that level of grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStrategy {
    /// Forced alignment.
    Fa(FaStrategy),
    /// Utterance timing recovery.
    Utr(UtrStrategy),
    /// Monotonicity enforcement.
    Monotonicity(MonotonicityStrategy),
    /// Morphosyntax.
    Morphosyntax(MorphosyntaxStrategy),
    /// Coreference.
    Coref(CorefStrategy),
    /// Utterance segmentation.
    Utseg(UtsegStrategy),
}

impl DecisionStrategy {
    /// The pipeline module this strategy belongs to.
    pub fn module(&self) -> DecisionModule {
        match self {
            Self::Fa(_) => DecisionModule::Fa,
            Self::Utr(_) => DecisionModule::Utr,
            Self::Monotonicity(_) => DecisionModule::Monotonicity,
            Self::Morphosyntax(_) => DecisionModule::Morphosyntax,
            Self::Coref(_) => DecisionModule::Coref,
            Self::Utseg(_) => DecisionModule::Utseg,
        }
    }

    /// Stable label for `%xalign` tier output and structured tracing.
    ///
    /// Matches the previous `&'static str` strategy literals (so existing
    /// consumers that string-match the `%xalign` content keep working).
    pub fn strategy_name(&self) -> &'static str {
        match self {
            Self::Fa(s) => s.as_str(),
            Self::Utr(s) => s.as_str(),
            Self::Monotonicity(s) => s.as_str(),
            Self::Morphosyntax(s) => s.as_str(),
            Self::Coref(s) => s.as_str(),
            Self::Utseg(s) => s.as_str(),
        }
    }
}

/// A machine decision that altered the output and should be reviewable.
///
/// Every silent clamp, skip, default, or normalization that changes the
/// output should produce a `DecisionRecord`. These are collected during
/// processing and optionally injected as `%xalign` tiers.
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// Index into `ChatFile.lines` (the affected utterance).
    pub line_idx: usize,
    /// Speaker code for the affected utterance.
    pub speaker: String,
    /// Typed module + strategy. Replaces the prior separate `module` /
    /// `strategy: &'static str` fields; use `strategy.module()` and
    /// `strategy.strategy_name()` to recover the old components.
    pub strategy: DecisionStrategy,
    /// Structured key=value reason for `%xalign` content.
    ///
    /// Example: `"overlap=1200ms prev_end=5000 next_start=3800"`
    pub reason: String,
    /// Whether a human should review this decision (`%xrev: [?]`).
    pub needs_review: bool,
}

impl DecisionRecord {
    /// Format the `%xalign` tier content: `module:strategy reason_string`.
    pub fn xalign_content(&self) -> String {
        format!(
            "{}:{} {}",
            self.strategy.module().as_str(),
            self.strategy.strategy_name(),
            self.reason
        )
    }

    /// Emit a structured tracing event for this decision.
    ///
    /// This is the single logging point — callers should NOT separately call
    /// `tracing::warn!` with the same information. The decision record is the
    /// source of truth; tracing and `%xalign` tiers are derived outputs.
    pub fn trace(&self) {
        let module = self.strategy.module().as_str();
        let strategy = self.strategy.strategy_name();
        if self.needs_review {
            tracing::warn!(
                module,
                strategy,
                speaker = %self.speaker,
                line_idx = self.line_idx,
                reason = %self.reason,
                "pipeline decision (needs review)"
            );
        } else {
            tracing::info!(
                module,
                strategy,
                speaker = %self.speaker,
                line_idx = self.line_idx,
                reason = %self.reason,
                "pipeline decision"
            );
        }
    }

    /// Create a decision, emit its trace, and return it.
    ///
    /// Convenience for the common pattern at decision points:
    /// ```ignore
    /// decisions.push(DecisionRecord::new_and_trace(...));
    /// ```
    pub fn new_and_trace(
        line_idx: usize,
        speaker: String,
        strategy: DecisionStrategy,
        reason: String,
        needs_review: bool,
    ) -> Self {
        let record = Self {
            line_idx,
            speaker,
            strategy,
            reason,
            needs_review,
        };
        record.trace();
        record
    }
}

/// Controls how many decision tiers are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewLevel {
    /// No review tiers at all.
    None,
    /// Only `%xrev: [?]` on low-confidence utterances (default).
    #[default]
    LowConfidence,
    /// `%xalign` on every bulleted utterance + `%xrev: [?]` on low-confidence.
    All,
}

/// Inject `%xalign` and `%xrev` tiers from `DecisionRecord`s.
///
/// Generalizes `fa::review_tiers::inject_review_tiers` to accept any
/// pipeline's decisions, not just FA repair decisions.
pub fn inject_decision_tiers(
    chat_file: &mut ChatFile,
    decisions: &[DecisionRecord],
    review_level: ReviewLevel,
) {
    if review_level == ReviewLevel::None || decisions.is_empty() {
        return;
    }

    // Strip any existing %xalign / %xrev tiers from every utterance so that
    // re-running a command (align, morphotag, etc.) on a file that already
    // has decision tiers replaces them rather than appending a second set.
    strip_decision_tiers(chat_file);

    // Index decisions by line_idx.
    let mut decision_map: std::collections::HashMap<usize, Vec<&DecisionRecord>> =
        std::collections::HashMap::new();
    for d in decisions {
        decision_map.entry(d.line_idx).or_default().push(d);
    }

    for (line_idx, line) in chat_file.lines.iter_mut().enumerate() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        if let Some(decisions_for_utt) = decision_map.get(&line_idx) {
            // Merge all decisions for this utterance into a single %xalign tier.
            // CHAT E401 forbids duplicate dependent tiers — one %xalign per
            // utterance maximum, regardless of how many pipeline stages fired.
            let merged_content: String = decisions_for_utt
                .iter()
                .map(|d| d.xalign_content())
                .collect::<Vec<_>>()
                .join("; ");
            utt.dependent_tiers
                .push(make_user_tier("xalign", &merged_content));

            // One %xrev: [?] if ANY decision needs review.
            let any_needs_review = decisions_for_utt.iter().any(|d| d.needs_review);
            if any_needs_review {
                utt.dependent_tiers.push(make_user_tier("xrev", "[?]"));
            }
        } else if review_level == ReviewLevel::All {
            // Informational: no decisions were made for this utterance.
            if utt.main.content.bullet.is_some() {
                utt.dependent_tiers
                    .push(make_user_tier("xalign", "no_decisions"));
            }
        }
    }
}

/// Remove all `%xalign` and `%xrev` tiers from every utterance in the file.
///
/// Called at the top of [`inject_decision_tiers`] and
/// [`fa::review_tiers::inject_review_tiers`] so that re-running any pipeline
/// command replaces existing tiers rather than accumulating duplicates.
pub fn strip_decision_tiers(chat_file: &mut ChatFile) {
    for line in &mut chat_file.lines {
        let Line::Utterance(utt) = line else {
            continue;
        };
        utt.dependent_tiers.retain(|tier| {
            !matches!(
                tier,
                DependentTier::UserDefined(t)
                    if t.label.as_str() == "xalign" || t.label.as_str() == "xrev"
            )
        });
    }
}

/// Construct a `DependentTier::UserDefined` with the given label and content.
///
/// # Safety (panic-freedom)
///
/// All call sites pass compile-time string literals that are visibly non-empty,
/// so `NonEmptyString::new` cannot fail.
#[allow(clippy::expect_used)]
fn make_user_tier(label: &str, content: &str) -> DependentTier {
    DependentTier::UserDefined(UserDefinedDependentTier {
        label: NonEmptyString::new(label).expect("tier label must be non-empty"),
        content: NonEmptyString::new(content).expect("tier content must be non-empty"),
        span: Span::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::WriteChat;
    use talkbank_parser::TreeSitterParser;

    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().expect("parser init");
        parser.parse_chat_file(text).unwrap()
    }

    #[test]
    fn decision_record_xalign_content_format() {
        let d = DecisionRecord {
            line_idx: 5,
            speaker: "CHI".into(),
            strategy: DecisionStrategy::Monotonicity(MonotonicityStrategy::EndClamped),
            reason: "overlap=1200ms prev_end=5000 next_start=3800".into(),
            needs_review: false,
        };
        assert_eq!(
            d.xalign_content(),
            "monotonicity:end_clamped overlap=1200ms prev_end=5000 next_start=3800"
        );
    }

    #[test]
    fn inject_decision_tiers_produces_xalign() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
*CHI:\tworld . \u{0015}4000_5000\u{0015}
@End
";
        let mut chat = parse_chat(chat_text);

        let decisions = vec![DecisionRecord {
            line_idx: 5,
            speaker: "CHI".into(),
            strategy: DecisionStrategy::Monotonicity(MonotonicityStrategy::EndClamped),
            reason: "overlap=500ms".into(),
            needs_review: true,
        }];

        inject_decision_tiers(&mut chat, &decisions, ReviewLevel::LowConfidence);

        let output = chat.to_chat_string();
        assert!(
            output.contains("%xalign:\tmonotonicity:end_clamped overlap=500ms"),
            "output:\n{output}"
        );
        assert!(output.contains("%xrev:\t[?]"), "output:\n{output}");
    }

    #[test]
    fn inject_decision_tiers_none_produces_nothing() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
@End
";
        let mut chat = parse_chat(chat_text);

        let decisions = vec![DecisionRecord {
            line_idx: 5,
            speaker: "CHI".into(),
            // Arbitrary FA strategy — this test only checks the
            // None-review-level gate, not the specific strategy.
            strategy: DecisionStrategy::Fa(FaStrategy::TimingStripped),
            reason: "test".into(),
            needs_review: true,
        }];

        inject_decision_tiers(&mut chat, &decisions, ReviewLevel::None);

        let output = chat.to_chat_string();
        assert!(!output.contains("%xalign:"), "output:\n{output}");
    }

    #[test]
    fn fa_decision_record_carries_strategy_metadata() {
        let decision = DecisionRecord {
            line_idx: 3,
            speaker: "MOT".into(),
            strategy: DecisionStrategy::Fa(FaStrategy::GapFilled),
            reason: "gap=500ms".into(),
            needs_review: true,
        };
        assert_eq!(decision.strategy.module(), DecisionModule::Fa);
        assert_eq!(decision.strategy.strategy_name(), "gap_filled");
        assert_eq!(decision.xalign_content(), "fa:gap_filled gap=500ms");
    }

    /// Running `align` (or any other pipeline command) on a file that already
    /// has `%xalign`/`%xrev` tiers must REPLACE those tiers, not accumulate
    /// more.  Before the fix, `inject_decision_tiers` only pushed; it never
    /// stripped existing tiers.  This produced two `%xalign` and two `%xrev`
    /// lines after the second run.
    #[test]
    fn inject_decision_tiers_replaces_not_accumulates() {
        // Simulate a file that already has %xalign and %xrev from a previous run.
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
%xalign:\told_decision
%xrev:\t[ok]
@End
";
        let mut chat = parse_chat(chat_text);

        let decisions = vec![DecisionRecord {
            line_idx: 5,
            speaker: "CHI".into(),
            strategy: DecisionStrategy::Fa(FaStrategy::TimingStripped),
            reason: "count=1".into(),
            needs_review: true,
        }];

        inject_decision_tiers(&mut chat, &decisions, ReviewLevel::LowConfidence);

        let output = chat.to_chat_string();
        let xalign_count = output.matches("%xalign:").count();
        let xrev_count = output.matches("%xrev:").count();

        assert_eq!(
            xalign_count, 1,
            "should replace existing %xalign, not accumulate (got {xalign_count}):\n{output}"
        );
        assert_eq!(
            xrev_count, 1,
            "should replace existing %xrev, not accumulate (got {xrev_count}):\n{output}"
        );
        assert!(
            !output.contains("old_decision"),
            "old %xalign content should be gone:\n{output}"
        );
        assert!(
            !output.contains("[ok]"),
            "old %xrev content should be gone:\n{output}"
        );
        assert!(
            output.contains("fa:timing_stripped"),
            "new %xalign content should be present:\n{output}"
        );
    }

    /// When two decisions target the same utterance (e.g. one FA decision and
    /// one monotonicity decision), they must be merged into a single %xalign
    /// tier.  CHAT E401 forbids duplicate dependent tiers on the same utterance.
    #[test]
    fn inject_decision_tiers_merges_multiple_decisions_for_same_utterance() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello world . \u{0015}1000_3000\u{0015}
@End
";
        let mut chat = parse_chat(chat_text);

        // Two decisions on the same utterance — one FA, one Monotonicity.
        let decisions = vec![
            DecisionRecord {
                line_idx: 5,
                speaker: "CHI".into(),
                strategy: DecisionStrategy::Fa(FaStrategy::WordsTimingDropped),
                reason: "count=1 reason=clamped_to_utterance_boundary".into(),
                needs_review: true,
            },
            DecisionRecord {
                line_idx: 5,
                speaker: "CHI".into(),
                strategy: DecisionStrategy::Monotonicity(MonotonicityStrategy::TimingStripped),
                reason: "non_monotonic start_ms=47646 previous_start_ms=49506".into(),
                needs_review: true,
            },
        ];

        inject_decision_tiers(&mut chat, &decisions, ReviewLevel::LowConfidence);

        let output = chat.to_chat_string();
        let xalign_count = output.matches("%xalign:").count();
        let xrev_count = output.matches("%xrev:").count();

        assert_eq!(
            xalign_count, 1,
            "multiple decisions for same utterance must merge into one %xalign (got {xalign_count}):\n{output}"
        );
        assert_eq!(
            xrev_count, 1,
            "multiple decisions for same utterance must produce one %xrev (got {xrev_count}):\n{output}"
        );
        // Both decision contents must appear in the single tier.
        assert!(
            output.contains("fa:words_timing_dropped"),
            "FA decision must be in merged %xalign:\n{output}"
        );
        assert!(
            output.contains("monotonicity:timing_stripped"),
            "Monotonicity decision must be in merged %xalign:\n{output}"
        );
    }
}
