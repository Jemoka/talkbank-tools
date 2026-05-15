//! Typed manifest for the per-command regression-fixture system.
//!
//! Each regression fixture in `resources/fixtures/<command>/regressions/<bug-name>/`
//! ships a `source.json` file describing where the fixture came from, what bug
//! it captures, and how the runner should validate the output. This module
//! parses that JSON into typed Rust structs and exposes the assertion checks
//! the runner applies.
//!
//! See `resources/fixtures/README.md` for the full directory layout and the JSON
//! schema. See `tests/ml_golden/regression_fixtures.rs` for the runner that
//! consumes these manifests.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

// ---------------------------------------------------------------------------
// Newtypes
// ---------------------------------------------------------------------------

/// Identifies which `batchalign3` command a fixture exercises.
///
/// We name the variants explicitly rather than carrying a raw `String` so
/// that the runner's dispatch table can match exhaustively. New commands must
/// be added here before a fixture can target them.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureCommand {
    /// `batchalign3 align` — forced alignment of an existing CHAT against audio.
    Align,
    /// `batchalign3 transcribe` — ASR + utseg from raw audio.
    Transcribe,
    /// `batchalign3 morphotag` — morphosyntactic annotation of an existing CHAT.
    Morphotag,
    /// `batchalign3 utseg` — utterance segmentation of an existing CHAT.
    Utseg,
    /// `batchalign3 translate` — translation tier injection.
    Translate,
    /// `batchalign3 coref` — coreference resolution.
    Coref,
}

/// 0-based main-tier utterance index inside a fixture's input file.
///
/// `source.json` carries 0-based indices because the runner walks
/// `ChatFile.utterances()` directly and counts main-tier utterances in
/// document order. The number written in the JSON corresponds to the same
/// counting that the parser produces, not the raw 1-based file line.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MainTierIndex(pub usize);

/// Lower bound on per-`%wor` word duration, in milliseconds.
///
/// Wrapped to make it visible in function signatures: a bare `u64` parameter
/// is ambiguous, but `MinDurationMs` is self-documenting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MinDurationMs(pub u64);

/// Maximum allowed proportion of an utterance's bullet duration that any
/// single `%wor` word may consume. A value of `0.4` means "no word may take
/// more than 40% of the parent utterance bullet". Used to catch DP-collapse
/// failures where one word dominates the utterance.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MaxProportion(pub f64);

/// Maximum allowed lead, in milliseconds, between a main-tier bullet start and
/// the first timed `%wor` word in that utterance.
///
/// Wrapped to keep signatures explicit: this is not just any count of
/// milliseconds, it is specifically a "main tier may lead first `%wor` by at
/// most N ms" contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MaxLeadMs(pub u64);

/// Maximum allowed overrun, in milliseconds, for the last timed `%wor` word to
/// extend past the utterance's main-tier end.
///
/// Wrapped to make the boundary explicit in signatures and manifest schema:
/// this is not just any millisecond threshold, it is specifically "how far
/// past the main-tier end the last timed `%wor` word may reach".
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MaxOverrunMs(pub u64);

/// Lower bound on the total number of main-tier utterances in the output CHAT.
///
/// Wrapped so signatures and manifest parsing say what the count means instead
/// of carrying a raw `usize`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MinUtteranceCount(pub usize);

/// Upper bound on a word-count contract in one emitted utterance.
///
/// Wrapped so assertion signatures and manifest parsing keep the threshold's
/// meaning explicit instead of carrying a raw `usize`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MaxWordCount(pub usize);

/// Lower bound on UTR coverage, expressed as a whole-percent threshold in
/// `[0, 100]`.
///
/// A value of `80` means "at least 80% of the utterances that were untimed on
/// input must carry a UTR-assigned bullet in the output". Wrapped so the
/// manifest schema and assertion signatures make the unit explicit instead of
/// carrying a bare `u8`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinUtrCoveragePercent(pub u8);

impl<'de> Deserialize<'de> for MinUtrCoveragePercent {
    /// Enforces the `0..=100` invariant at manifest-load time rather than
    /// letting a nonsensical `>100` threshold silently make the assertion
    /// impossible to satisfy. Matches CLAUDE.md #6a: fallible construction
    /// should not live behind an infallible `From`/`#[serde(transparent)]`.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(d)?;
        if v > 100 {
            return Err(serde::de::Error::custom(format!(
                "min_utr_coverage_percent must be in 0..=100, got {v}"
            )));
        }
        Ok(Self(v))
    }
}

/// Lower bound on the number of distinct speaker labels in the output CHAT.
///
/// Wrapped so the manifest and assertion signatures say what the count means
/// instead of carrying a raw `usize`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct MinSpeakerCount(pub usize);

/// `%wor` materialization policy for transcribe fixtures.
///
/// This is fixture-manifest scoped rather than reusing the runtime enum
/// directly so the regression schema can evolve independently from CLI option
/// parsing and keep a narrow, typed surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureWorPolicy {
    /// Emit `%wor` tiers in the transcribe output.
    #[default]
    Include,
    /// Omit `%wor` tiers from the transcribe output.
    Omit,
}

/// ASR engine override for transcribe fixtures.
///
/// Kept manifest-local so the regression schema only exposes engines that real
/// fixtures currently need, instead of mirroring the full CLI enum up front.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureTranscribeAsrEngine {
    /// Use local Whisper ASR.
    #[default]
    Whisper,
    /// Use the Rev.AI backend when credentials are configured.
    RevAi,
}

// ---------------------------------------------------------------------------
// Manifest schema
// ---------------------------------------------------------------------------

/// Provenance metadata captured when the fixture was created.
///
/// None of these fields drive runner behavior — they exist so a future
/// contributor can answer "where did this fixture come from?" without git
/// archaeology. Kept inside the typed manifest so we can validate that the
/// fields are present at JSON parse time, not at first read.
#[derive(Clone, Debug, Deserialize)]
pub struct FixtureSourceMetadata {
    /// Path (workspace-relative) to the originating bug report.
    pub report: String,
    /// Path or URL of the original CHAT file the fixture was trimmed from.
    pub original_chat: String,
    /// Path or URL of the original audio file the fixture was trimmed from.
    #[serde(default)]
    pub audio_source: Option<String>,
    /// 1-based main-tier utterance range that was trimmed from the original
    /// (inclusive, matches the `--lines` flag of `trim_chat_audio.py`).
    #[serde(default)]
    pub trimmed_utterance_range: Option<[usize; 2]>,
    /// Audio offset in ms (audio_start_ms - padding) used at trim time.
    #[serde(default)]
    pub trimmed_audio_offset_ms: Option<u64>,
}

/// Human-readable bug description carried alongside the assertion list.
///
/// `class` lets multiple fixtures group together for analysis even before a
/// fix lands ("which fixtures share the `fa_dp_collapse_to_end` failure
/// mode?"). The runner does not interpret these strings.
#[derive(Clone, Debug, Deserialize)]
pub struct BugDescription {
    /// One-paragraph plain-English summary.
    pub summary: String,
    /// Stable identifier for the bug class. Free-form snake_case.
    pub class: String,
    /// 0-based main-tier index of the utterance the bug affects.
    pub affected_main_tier_index: MainTierIndex,
}

/// Command-local fixture options for `batchalign3 transcribe`.
///
/// Kept under a transcribe-specific struct so audio-first regression fixtures
/// can request a command-shaped override without introducing a premature
/// generic "options blob" for every command family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscribeFixtureOptions {
    /// ASR engine for the transcribe output.
    #[serde(default)]
    pub asr_engine: FixtureTranscribeAsrEngine,
    /// `%wor` tier policy for the transcribe output.
    #[serde(default)]
    pub wor: FixtureWorPolicy,
    /// Whether the fixture should request speaker diarization.
    #[serde(default)]
    pub diarize: bool,
}

/// One assertion against the post-`align` (or post-`<command>`) output CHAT.
///
/// Variants are tagged with the `kind` discriminator in JSON so the runner
/// can dispatch on them in a single `match`. Adding a new variant requires
/// (a) defining it here, (b) implementing it in
/// `regression_fixtures::run_assertion`, and (c) documenting it in
/// `resources/fixtures/README.md`. Do not add variants speculatively — every
/// variant should be backed by a real fixture that needs it.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixtureAssertion {
    /// Every word in the affected utterance's `%wor` tier must have a
    /// non-zero-duration timing bullet (i.e. `start_ms < end_ms`). Catches
    /// the bug class where FA emits all words but with `start_ms == end_ms`
    /// for most of them.
    NoZeroDurationWorWords {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
    },
    /// Every word in the affected utterance's `%wor` tier must have a
    /// duration of at least `threshold_ms`. Catches "DP collapse to end"
    /// where the tail of the word sequence gets crammed into a few
    /// hundred ms with each word getting 40-60 ms.
    MinWorWordDurationMs {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
        /// Minimum allowed per-word duration in ms.
        threshold_ms: MinDurationMs,
    },
    /// No single word in the affected utterance's `%wor` tier may consume
    /// more than `max_proportion` of the parent utterance bullet's
    /// duration. Catches "first-word dominance" where one word eats the
    /// majority of the utterance and the rest are crammed into a sliver.
    MaxWorWordDurationProportion {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
        /// Largest allowed share, e.g. 0.4 means 40%.
        max_proportion: MaxProportion,
    },
    /// The LAST word in the affected utterance's `%wor` tier must have a
    /// duration of at least `threshold_ms`. Catches "tail cutoff" where
    /// the closing word of a sentence gets squished into a sliver
    /// because the FA pipeline ran out of audio for it. The last word
    /// is the most commonly affected by this failure mode because the
    /// DP allocates time greedily from the front.
    MinLastWorWordDurationMs {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
        /// Minimum allowed last-word duration in ms.
        threshold_ms: MinDurationMs,
    },
    /// The main-tier bullet may begin at most `threshold_ms` before the first
    /// timed `%wor` word in the utterance. Catches stale-start bugs where the
    /// utterance inherits the previous utterance's start even though the newly
    /// aligned words begin much later.
    MaxMainTierLeadBeforeFirstWorMs {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
        /// Largest allowed lead before the first timed `%wor` word, in ms.
        threshold_ms: MaxLeadMs,
    },
    /// The last timed `%wor` word in the utterance may end at most
    /// `threshold_ms` after the main-tier bullet end. Catches cutoff/overrun
    /// bugs where the utterance bullet ends too early even though `%wor`
    /// preserves the real tail of the speech.
    MaxLastWorOverrunPastMainEndMs {
        /// Affected main-tier utterance (0-based).
        main_tier_index: MainTierIndex,
        /// Largest allowed overrun past the main-tier end, in ms.
        threshold_ms: MaxOverrunMs,
    },
    /// The output CHAT must contain at least `threshold_count` main-tier
    /// utterances. Useful for transcribe/utseg-style regressions where the
    /// failure mode is under-segmentation of the whole output, not a local
    /// `%wor` timing defect in one utterance.
    MinMainTierUtteranceCount {
        /// Minimum acceptable number of main-tier utterances in the output.
        threshold_count: MinUtteranceCount,
    },
    /// The first emitted main-tier utterance must contain at most
    /// `threshold_count` extracted words.
    ///
    /// Useful for transcribe provider regressions where the output is not just
    /// under-segmented overall, but specifically front-loads an implausibly
    /// giant first utterance.
    MaxFirstMainTierWordCount {
        /// Maximum acceptable number of extracted words in the first utterance.
        threshold_count: MaxWordCount,
    },
    /// The output CHAT must not materialize any `%wor` tiers. Useful for
    /// transcribe fixtures that intentionally exercise the `wor=Omit` path.
    NoWorTiersPresent,
    /// The output CHAT must contain at least `threshold_count` distinct
    /// main-tier speaker labels. Useful for diarize fixtures that should
    /// surface multiple speakers instead of collapsing everything into `PAR0`.
    MinDistinctMainTierSpeakerCount {
        /// Minimum acceptable number of distinct speaker labels.
        threshold_count: MinSpeakerCount,
    },
    /// The serialized `@Media` header must preserve the input audio basename.
    ///
    /// This is a boundary contract on emitted CHAT rather than a semantic AST
    /// invariant, so the runner checks the final header line directly.
    MediaHeaderMatchesInputBasename,
    /// Passes iff no FA group had `start_ms >= end_ms` at build time.
    /// Detects the MICASE 8-failure class: a pre-FA build-time check rejected
    /// the utterance group's audio window as empty or inverted and the job
    /// should have failed early (or emitted an invalid-window marker in the
    /// output) instead of silently producing aligned CHAT.
    NoFaGroupInvalidAudioWindow,
    /// Passes iff the output CHAT has NO `%xalign: monotonicity:*` lines.
    /// Detects the samtale-style silent rescue: when the aligner's output
    /// would have been non-monotonic, the old pipeline would emit a patched
    /// `%xalign` tier with a `monotonicity:` marker instead of failing. Once
    /// the rescue layer is deleted, this becomes a load-bearing negative
    /// assertion that no rescue fired.
    NoMonotonicityRescueEmitted,
    /// Passes iff for every pair of adjacent non-overlap utterances
    /// `u_i`, `u_{i+1}`, we have `u_i.start_ms < u_{i+1}.start_ms`.
    ///
    /// Overlap-continuation utterances (e.g. `+<` markers) are skipped because
    /// they share start timing with their predecessor by design.
    UtteranceBulletMonotonicityPreserved,
    /// Passes iff at least `threshold_percent` of utterances that were
    /// untimed on input now carry a UTR-assigned bullet in the output.
    MinUtrCoveragePercent {
        /// Minimum share of newly-timed formerly-untimed utterances, expressed
        /// as a whole-percent value in `[0, 100]`.
        threshold_percent: MinUtrCoveragePercent,
    },
    /// Passes iff no utterance was silently returned with no bullet AND an
    /// `%xrev: [?]` flag (i.e. no timing_stripped path fired): an utterance
    /// that lost its timing must not also carry the rescue-review marker,
    /// because that pairing signals the silent timing-strip rescue that this
    /// assertion is designed to detect.
    NoSilentTimingStrip,
}

impl FixtureAssertion {
    /// Index of the main-tier utterance this assertion targets, if it targets
    /// one specific utterance.
    pub fn main_tier_index(&self) -> Option<MainTierIndex> {
        match self {
            Self::NoZeroDurationWorWords { main_tier_index } => Some(*main_tier_index),
            Self::MinWorWordDurationMs {
                main_tier_index, ..
            } => Some(*main_tier_index),
            Self::MaxWorWordDurationProportion {
                main_tier_index, ..
            } => Some(*main_tier_index),
            Self::MinLastWorWordDurationMs {
                main_tier_index, ..
            } => Some(*main_tier_index),
            Self::MaxMainTierLeadBeforeFirstWorMs {
                main_tier_index, ..
            } => Some(*main_tier_index),
            Self::MaxLastWorOverrunPastMainEndMs {
                main_tier_index, ..
            } => Some(*main_tier_index),
            Self::MinMainTierUtteranceCount { .. } => None,
            Self::MaxFirstMainTierWordCount { .. } => None,
            Self::NoWorTiersPresent => None,
            Self::MinDistinctMainTierSpeakerCount { .. } => None,
            Self::MediaHeaderMatchesInputBasename => None,
            Self::NoFaGroupInvalidAudioWindow => None,
            Self::NoMonotonicityRescueEmitted => None,
            Self::UtteranceBulletMonotonicityPreserved => None,
            Self::MinUtrCoveragePercent { .. } => None,
            Self::NoSilentTimingStrip => None,
        }
    }
}

/// Top-level deserialized form of `source.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct FixtureManifest {
    /// Which command this fixture exercises.
    pub command: FixtureCommand,
    /// 3-letter ISO language code (e.g. `"eng"`).
    pub language: String,
    /// Filename (relative to the fixture dir) of the input CHAT.
    ///
    /// Commands like `align`, `morphotag`, `utseg`, `translate`, and `coref`
    /// start from CHAT. Audio-first commands like `transcribe` may omit it.
    #[serde(default)]
    pub input_chat: Option<String>,
    /// Filename (relative to the fixture dir) of the input audio. Required
    /// for commands that consume audio (`align`, `transcribe`, etc.); may
    /// be `None` for text-only commands.
    #[serde(default)]
    pub audio: Option<String>,
    /// Command-local options for transcribe fixtures.
    #[serde(default)]
    pub transcribe: Option<TranscribeFixtureOptions>,
    /// Source provenance metadata.
    pub source: FixtureSourceMetadata,
    /// Bug description (human-readable summary + class).
    pub bug: BugDescription,
    /// Assertions to apply to the output CHAT.
    pub assertions: Vec<FixtureAssertion>,
}

impl FixtureManifest {
    /// Parse a `source.json` file at `path` into a typed manifest.
    ///
    /// Returns a stringly error so test code can `expect()` it with the
    /// fixture path embedded in the message.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("failed to parse {}: {e}", path.display()))
    }
}

/// One discovered fixture: its directory + parsed manifest.
///
/// The directory is the parent of `source.json`; all paths in the manifest
/// (`input_chat`, `audio`) are resolved relative to it.
#[derive(Clone, Debug)]
pub struct DiscoveredFixture {
    /// Absolute path to the fixture directory.
    pub dir: PathBuf,
    /// Parsed manifest.
    pub manifest: FixtureManifest,
}

impl DiscoveredFixture {
    /// Absolute path to the fixture's input CHAT.
    pub fn input_chat_path(&self) -> Option<PathBuf> {
        self.manifest
            .input_chat
            .as_ref()
            .map(|name| self.dir.join(name))
    }

    /// Absolute path to the fixture's input audio (if any).
    pub fn audio_path(&self) -> Option<PathBuf> {
        self.manifest.audio.as_ref().map(|name| self.dir.join(name))
    }
}
