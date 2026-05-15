//! Typed data model for the L2 morphotag evaluation analyzer.
//!
//! Every boundary that carries domain meaning — pair keys, flag names,
//! serialized MOR items, surface forms — has a named type. Primitives are
//! parsed once and carried as typed wrappers so the CSV/summary serializer
//! receives already-validated values.
//!
//! This mirrors the Python `analyze.py` module's data classes but replaces
//! stringly typing with the `talkbank-model` types where available
//! (`PosCategory`, `GrammaticalRelationType`, `LanguageCode`) and adds
//! newtypes where no upstream type exists.

use std::fmt;
use std::path::PathBuf;

use talkbank_model::alignment::helpers::{MorAlignableWordCount, MorItemCount};
use talkbank_model::model::{
    GrammaticalRelationType, LanguageCode, PosCategory, content::word::WordLanguageMarker,
};

// ---------------------------------------------------------------------------
// String-boundary newtypes
//
// Each newtype documents the invariants of a stable domain string. They are
// constructible from &str / String (cheap Arc or interned where available)
// and auto-`Deref` to `&str` for zero-friction use in format strings.
// ---------------------------------------------------------------------------

/// Canonical language-pair key (e.g. `"deu,eng"`), used to group files.
///
/// The format is ISO-639-3 codes separated by a comma with no spaces — it
/// matches the `pair_key` field in `eval-set.jsonl`. Construction does not
/// validate the internal shape because callers feed it directly from the
/// eval-set file (which is the source of truth for pair keys).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PairKey(String);

impl PairKey {
    /// Wraps a raw string as a `PairKey` without validation.
    ///
    /// The eval-set file is authoritative; we never invent or transform the
    /// key. Construction is a zero-cost newtype wrap.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow as `&str` for format strings and CSV output.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PairKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Surface form of a word on the main tier (e.g. `"wake"`, `"tienda"`).
///
/// CHAT orthography preserved as-written — no case folding applied at the
/// newtype level. Heuristics that need case-insensitive lookup lowercase
/// their comparison values locally.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceWord(String);

impl SurfaceWord {
    /// Wraps a raw main-tier word as a typed surface form.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SurfaceWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Serialized `%mor` item text (e.g. `"verb|wake-Fin-Imp-S"`).
///
/// Carried for CSV output and for `L2|xxx` detection. The *structured*
/// POS / lemma / features analysis comes from the typed `MorWord` carried
/// alongside in [`AtSAnalysis`] — never from regexing this string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MorItemText(String);

impl MorItemText {
    /// Wraps a serialized MOR item as a typed value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the serialized form is the L2-fallback sentinel.
    ///
    /// `L2|xxx` is emitted by the morphotag pipeline when secondary dispatch
    /// failed (unsupported language, empty response, etc.). The analyzer
    /// counts these against the feature's dispatch-rate gate.
    pub fn is_l2_fallback(&self) -> bool {
        self.0.contains("L2|xxx")
    }
}

impl fmt::Display for MorItemText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Serialized `%gra` item text (e.g. `"4|3|COMPOUND-PRT"`).
///
/// Kept for CSV fidelity. The typed deprel is carried in the associated
/// [`GrammaticalRelationType`] field of [`AtSAnalysis`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GraItemText(String);

impl GraItemText {
    /// Wraps a serialized GRA item as a typed value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GraItemText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Dash-joined feature string (e.g. `"Fin-Imp-S"`), reconstructed from the
/// typed `MorWord.features` list for CSV output.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FeatureSet(String);

impl FeatureSet {
    /// Wraps a feature string as a typed value.
    ///
    /// Empty strings are permitted and represent "no features" — callers
    /// that want to distinguish "no features" from "no MOR item" should use
    /// `Option<FeatureSet>` at the call site.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the feature set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for FeatureSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Language marker + effective-language resolution
// ---------------------------------------------------------------------------

/// Typed representation of a CHAT `@s` marker attached to a word.
///
/// `@s` with no language code is the bilingual-conventional "secondary
/// language" marker — its effective language is the second `@Languages`
/// code. Explicit forms (`@s:spa`, `@s:eng+fra`, `@s:eng&spa`) carry the
/// codes inline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageMarkerKind {
    /// `@s` with no language code. Resolve via `@Languages`.
    Bare,
    /// `@s:xxx`, `@s:a+b`, or `@s:a&b`. Never empty.
    Explicit(Vec<LanguageCode>),
}

impl LanguageMarkerKind {
    /// Resolve to a single effective language for dispatch accounting.
    ///
    /// - `Explicit` uses the first listed code (consistent with the
    ///   dispatcher's current multi-code behavior).
    /// - `Bare` uses the secondary language if the transcript is
    ///   bi-or-multi-lingual; otherwise falls back to the primary.
    ///
    /// Returns `None` only when the header is empty (pathological CHAT).
    pub fn effective_language(&self, languages: &[LanguageCode]) -> Option<LanguageCode> {
        match self {
            LanguageMarkerKind::Explicit(codes) => codes.first().cloned(),
            LanguageMarkerKind::Bare => {
                if languages.len() >= 2 {
                    Some(languages[1].clone())
                } else {
                    languages.first().cloned()
                }
            }
        }
    }
}

/// Lossless lowering from the AST's `WordLanguageMarker` to our typed kind.
///
/// The AST distinguishes `Shortcut` (bare), `Explicit(one)`, `Multiple(many)`,
/// `Ambiguous(many)` — but for evaluation accounting we only care about
/// "has a code list" vs "bare". We preserve the code list without rewriting
/// it so the CSV round-trips.
impl From<&WordLanguageMarker> for LanguageMarkerKind {
    fn from(marker: &WordLanguageMarker) -> Self {
        match marker {
            WordLanguageMarker::Shortcut => LanguageMarkerKind::Bare,
            WordLanguageMarker::Explicit(code) => LanguageMarkerKind::Explicit(vec![code.clone()]),
            WordLanguageMarker::Multiple(codes) | WordLanguageMarker::Ambiguous(codes) => {
                LanguageMarkerKind::Explicit(codes.clone())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Status classification
// ---------------------------------------------------------------------------

/// Outcome of L2 dispatch for a single `@s` word.
///
/// `Spliced` means the secondary model returned a real analysis and the
/// merge produced a MOR item. `L2Xxx` is the sentinel fallback. `MissingMor`
/// means no MOR item existed at the expected position — in the AST-based
/// analyzer this should be vanishingly rare (it was common in the regex-
/// based Python analyzer because retrace markers shifted position counts).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AtSStatus {
    /// MOR item present and not the `L2|xxx` fallback.
    Spliced,
    /// MOR item is `L2|xxx` — dispatch failed.
    L2Xxx,
    /// No MOR item paired at this main-tier position.
    MissingMor,
}

impl AtSStatus {
    /// CSV spelling matching the Python analyzer for downstream compatibility.
    pub fn as_csv_str(&self) -> &'static str {
        match self {
            AtSStatus::Spliced => "spliced",
            AtSStatus::L2Xxx => "l2xxx",
            AtSStatus::MissingMor => "missing_mor",
        }
    }
}

impl fmt::Display for AtSStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_csv_str())
    }
}

/// Classify a paired MOR-item option as a splice outcome.
///
/// Free function (not a method on `Option`) because its meaning is domain-
/// specific: presence is *not* success — the item can be present but equal
/// to `L2|xxx`.
pub fn classify_status(mor_item: Option<&MorItemText>) -> AtSStatus {
    match mor_item {
        None => AtSStatus::MissingMor,
        Some(m) if m.is_l2_fallback() => AtSStatus::L2Xxx,
        Some(_) => AtSStatus::Spliced,
    }
}

// ---------------------------------------------------------------------------
// Heuristic flags
// ---------------------------------------------------------------------------

/// Rule-based detectors over a spliced `@s` word.
///
/// Each variant names a class of suspicious output. Heuristics err on the
/// side of recall: flags are candidates for manual review, not confirmed
/// errors. The precision of each heuristic is estimated on spot-checks.
///
/// The `L2Xxx` / `MissingMor` variants are produced only from the paired
/// status (not from POS/deprel inspection) — they represent failures of
/// the feature itself, not surface-model quality issues.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeuristicFlag {
    /// L2 dispatch produced the `L2|xxx` sentinel.
    L2Xxx,
    /// No MOR item paired at this position.
    MissingMor,
    /// PROPN tag assigned to a known function word in the effective
    /// language (conservative closed-class lists in `heuristics.rs`).
    PropnForFunctionWord,
    /// POS tag carries morphological features of an incompatible class
    /// (e.g. NOUN with `Fin`/`Mood`, VERB with `Plur`/`Case`).
    FeaturePosMismatch,
}

impl HeuristicFlag {
    /// Short name used in CSV / CSV flag columns (matches Python names).
    pub fn name(&self) -> &'static str {
        match self {
            HeuristicFlag::L2Xxx => "L2Xxx",
            HeuristicFlag::MissingMor => "MissingMor",
            HeuristicFlag::PropnForFunctionWord => "PropnForFunctionWord",
            HeuristicFlag::FeaturePosMismatch => "FeaturePosMismatch",
        }
    }
}

impl fmt::Display for HeuristicFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// Per-word record
// ---------------------------------------------------------------------------

/// One `@s` word observation, after pairing with `%mor` and `%gra`.
///
/// Constructed by [`crate::cli::eval_cmd::l2_morphotag::analysis`] when walking a
/// post-morphotag CHAT file. Consumed by heuristics and by the report writer.
#[derive(Clone, Debug, PartialEq)]
pub struct AtSOccurrence {
    /// Source file (for CSV traceability).
    pub file: PathBuf,
    /// Language-pair label from the eval set.
    pub pair_key: PairKey,
    /// The `@s` marker kind (bare or explicit codes).
    pub marker: LanguageMarkerKind,
    /// Resolved effective language (per `effective_language` rules).
    pub effective_lang: LanguageCode,
    /// Surface orthography of the word on the main tier.
    pub surface: SurfaceWord,
    /// 0-based MOR-domain position within the utterance — the index used
    /// to pair with `mor_tier.items` and `gra_tier.relations`.
    pub mor_position: usize,
    /// Serialized `%mor` item for CSV (`None` if no MOR item paired).
    pub mor_item: Option<MorItemText>,
    /// Serialized `%gra` item for CSV (`None` if no GRA item paired).
    pub gra_item: Option<GraItemText>,
}

/// Full analysis of one `@s` word: occurrence plus derived POS / features
/// / deprel / heuristic flags.
///
/// `pos`, `lemma`, `features`, `gra_deprel` are `Option` because a missing
/// MOR item (`AtSStatus::MissingMor`) has none of these.
#[derive(Clone, Debug, PartialEq)]
pub struct AtSAnalysis {
    /// The occurrence this analysis describes.
    pub occurrence: AtSOccurrence,
    /// UD POS tag (from typed `MorWord.pos`).
    pub pos: Option<PosCategory>,
    /// Lemma (from typed `MorWord.lemma`).
    pub lemma: Option<String>,
    /// Dash-joined features (reconstructed from typed features list).
    pub features: Option<FeatureSet>,
    /// GRA deprel token (from typed `GrammaticalRelation.relation`).
    pub gra_deprel: Option<GrammaticalRelationType>,
    /// Splice outcome classification.
    pub status: AtSStatus,
    /// Heuristic flags fired on this analysis.
    pub flags: Vec<HeuristicFlag>,
}

/// All `@s` analyses from one post-morphotag CHAT file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileAnalysis {
    /// Post-morphotag file path.
    pub path: PathBuf,
    /// Language-pair key this file was labeled with in the eval set.
    pub pair_key: PairKey,
    /// The `@Languages` header codes, in declared order.
    pub languages: Vec<LanguageCode>,
    /// Per-`@s`-word analyses.
    pub analyses: Vec<AtSAnalysis>,
    /// Per-utterance morphotag outcomes. One entry per `Utterance` line
    /// in the file, observed post-hoc from the morphotag output (the
    /// file has already been morphotagged; we cannot re-derive the
    /// pipeline's internal [`MorOutcome`]). See
    /// [`UtteranceOutcome`] for the observation model.
    pub utterance_outcomes: Vec<UtteranceOutcome>,
}

// ---------------------------------------------------------------------------
// Post-morphotag utterance outcomes
//
// The morphotag pipeline emits an internal `MorOutcome` per utterance (see
// `crate::chat_ops::morphosyntax_ops::outcome::MorOutcome`). That outcome
// is not serialized into the CHAT file — we cannot read it back. What we
// CAN observe post-hoc is whether the utterance has a %mor tier, whether
// its `%mor` item count matches the CHAT main-tier Mor-alignable count,
// and whether the utterance had any alignable content to begin with.
//
// The four-variant classification below is the external-observation
// equivalent of the internal MorOutcome. An idealized pipeline produces:
//   - NotApplicable        — when utt had zero alignable content
//   - Aligned              — when utt had N > 0 alignable words and got N %mor items
//   - CountMismatchInFile  — pipeline emitted %mor with wrong cardinality
//                             (should never happen post-2026-04-21 fix;
//                             presence indicates a new invariant leak)
//   - PipelineAbsorbedFailure — alignable content present but no %mor tier
//                             (pipeline absorbed a MisalignmentBug via the
//                             file-level boundary and emitted no tier)
//
// Wave 4 of the morphotag reconciliation architecture. See
// `book/src/architecture/morphotag-invariants.md`.
// ---------------------------------------------------------------------------

/// Post-hoc classification of one utterance's morphotag outcome, inferred
/// from the morphotag-output CHAT file alone (no pipeline metadata).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UtteranceOutcome {
    /// Utterance had zero Mor-alignable words and correctly produced no
    /// `%mor` tier. Matches [`MorOutcomeKind::NotApplicable`] at the
    /// pipeline layer.
    NotApplicable,
    /// Utterance had N > 0 Mor-alignable words and produced N `%mor`
    /// items. Happy path.
    Aligned {
        /// The agreed-upon count on both sides. Carried as the
        /// CHAT-side type since `MorItemCount` and `MorAlignableWordCount`
        /// both reduce to the same integer on the happy path.
        n_words: MorAlignableWordCount,
    },
    /// `%mor` tier present but item count ≠ CHAT alignable-word count.
    /// This is the external-observation equivalent of a
    /// [`MorOutcomeKind::MisalignmentBug`] that somehow reached the
    /// output file. Post-2026-04-21, `inject_morphosyntax`'s guard
    /// prevents this from happening at all — if this variant fires in
    /// the eval, either a new mismatch path exists or someone has
    /// manually edited the file.
    CountMismatchInFile {
        /// CHAT-side Mor-alignable word count.
        n_alignable: MorAlignableWordCount,
        /// Observed `%mor` item count.
        n_mor: MorItemCount,
    },
    /// Utterance had N > 0 Mor-alignable words but no `%mor` tier.
    /// Most commonly this is the pipeline having absorbed a
    /// [`MorOutcomeKind::MisalignmentBug`] at the file-level boundary
    /// and emitted no tier for this utterance. The companion
    /// `DecisionRecord` (if `%xalign` was enabled) will tell us
    /// which `MisalignmentClass` was involved.
    PipelineAbsorbedFailure {
        /// CHAT-side Mor-alignable word count (> 0).
        n_alignable: MorAlignableWordCount,
    },
}

impl UtteranceOutcome {
    /// Short label for CSV output (stable across releases).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Aligned { .. } => "aligned",
            Self::CountMismatchInFile { .. } => "count_mismatch_in_file",
            Self::PipelineAbsorbedFailure { .. } => "pipeline_absorbed_failure",
        }
    }

    /// Whether this outcome represents a pipeline anomaly that requires
    /// developer attention. NotApplicable and Aligned are expected
    /// behavior; the other two variants are not.
    pub fn is_anomaly(&self) -> bool {
        matches!(
            self,
            Self::CountMismatchInFile { .. } | Self::PipelineAbsorbedFailure { .. }
        )
    }
}
