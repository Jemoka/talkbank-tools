//! Typed per-command options for job submission and processing.
//!
//! Replaces the stringly-typed `HashMap<String, serde_json::Value>` that
//! previously carried command options through the system. Each command has
//! a dedicated struct with compile-time checked fields and serde defaults
//! matching the CLI defaults.
//!
//! # Wire format
//!
//! [`CommandOptions`] serializes as an internally-tagged JSON object:
//!
//! ```json
//! {
//!   "command": "morphotag",
//!   "retokenize": true,
//!   "skipmultilang": false,
//!   "merge_abbrev": false,
//!   "override_media_cache": false,
//!   "engine_overrides": {}
//! }
//! ```
//!
//! The `command` tag doubles as the command name for routing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use super::params::{MergeAbbrevPolicy, UtsegFallbackPolicy, WorTierPolicy};

// ---------------------------------------------------------------------------
// Default helpers
// ---------------------------------------------------------------------------

fn default_batch_window() -> usize {
    25
}

/// Default forced-alignment engine for serialized command options.
fn default_fa_engine() -> FaEngineName {
    FaEngineName::Wave2Vec
}

/// Default ASR engine for serialized command options.
fn default_asr_engine() -> AsrEngineName {
    AsrEngineName::RevAi
}

/// Default Whisper batch size.
fn default_batch_size() -> i32 {
    8
}

/// Default `%wor` policy for commands that enable the tier by default.
fn default_wor_tier_include() -> WorTierPolicy {
    WorTierPolicy::Include
}

/// Default openSMILE feature set.
fn default_feature_set() -> String {
    "eGeMAPSv02".to_string()
}

/// Shared behavior for all engine backend selectors.
///
/// Implement this on each engine enum (`AsrEngineName`, `FaEngineName`,
/// `UtrEngine`) so generic code can work across engine categories without
/// knowing which specific enum it holds.
pub use super::engines::*;

// ---------------------------------------------------------------------------
// CommonOptions
// ---------------------------------------------------------------------------

/// Options shared by all processing commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommonOptions {
    /// Bypass the media analysis cache.
    #[serde(default)]
    pub override_media_cache: bool,

    /// Engine overrides selected for this job (e.g., ASR=tencent, FA=cantonese_fa).
    /// Typed struct with `Option<AsrEngineName>` and `Option<FaEngineName>`.
    #[serde(default, skip_serializing_if = "EngineOverrides::is_empty")]
    pub engine_overrides: EngineOverrides,

    /// Multi-word token (MWT) lexicon: maps a surface form (e.g. "gonna")
    /// to its expansion tokens (e.g. `["going", "to"]`).
    /// Loaded from `--lexicon` CSV on the CLI side.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mwt: BTreeMap<String, Vec<String>>,

    /// Optional directory for pipeline debug artifact dumps. Always carries
    /// an absolute path: the CLI canonicalizes the user-supplied
    /// `--debug-dir` value via `canonicalize_debug_dir` in
    /// `batchalign-cli::args::options` before constructing this struct, so
    /// the server never has to resolve a relative path against its own
    /// (opaque) working directory. `serde` serializes `PathBuf` as a JSON
    /// string, so the wire format is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_dir: Option<PathBuf>,

    /// Per-task cache override specifications (comma-separated task names).
    /// When non-empty, only the listed tasks skip cache; others use cache normally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub override_media_cache_tasks: Vec<String>,

    /// Number of files per batch window for text NLP commands.
    /// Smaller windows show progress sooner; larger windows batch more
    /// efficiently (Stanza batching is 7.4x faster than per-sentence).
    /// Default: 25. Set to 0 for all-in-one (no windowing).
    #[serde(default = "default_batch_window")]
    pub batch_window: usize,
}

impl CommonOptions {
    /// Serialize `engine_overrides` to a JSON string for pool worker keying.
    ///
    /// Returns an empty string when no overrides are set (matching the pool
    /// config's default). This ensures `pre_scale_with_overrides` produces
    /// the same key that `dispatch_execute_v2` will look up.
    /// Serialize engine overrides to a JSON string for pool worker keying.
    ///
    /// Returns empty string when no overrides are set.
    pub fn engine_overrides_json(&self) -> String {
        self.engine_overrides.to_json_string()
    }
}

impl Default for CommonOptions {
    fn default() -> Self {
        Self {
            override_media_cache: false,
            engine_overrides: EngineOverrides::default(),
            mwt: BTreeMap::new(),
            debug_dir: None,
            override_media_cache_tasks: Vec::new(),
            batch_window: default_batch_window(),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-command option structs
// ---------------------------------------------------------------------------

/// How `+<` overlap utterances are handled during UTR.
///
/// Selects the alignment strategy for utterance timing recovery. The trait-based
/// architecture in `batchalign-chat-ops` allows plugging in different strategies
/// at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtrOverlapStrategy {
    /// Automatically select: two-pass when `+<` utterances are present,
    /// global otherwise.
    #[default]
    Auto,
    /// Single global DP pass. `+<` utterances get no special treatment.
    Global,
    /// Two-pass overlap-aware strategy. Pass 1 excludes `+<` utterances,
    /// pass 2 recovers their timing from the predecessor's audio window.
    TwoPass,
}

/// Options for the `align` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// FA engine selector (`wav2vec_fa`, `whisper_fa`, or plugin name).
    #[serde(default = "default_fa_engine")]
    pub fa_engine: FaEngineName,

    /// UTR engine selection.
    ///
    /// `None` means utterance timing recovery is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utr_engine: Option<UtrEngine>,

    /// How `+<` overlap utterances are handled during UTR.
    #[serde(default)]
    pub utr_overlap_strategy: UtrOverlapStrategy,

    /// Two-pass UTR configuration (CA markers, density threshold, buffers).
    #[serde(default)]
    pub utr_two_pass: crate::chat_ops::fa::TwoPassConfig,

    /// Include pause durations in forced alignment.
    #[serde(default)]
    pub pauses: bool,

    /// Generate `%wor` tier with word-level timing bullets.
    #[serde(default = "default_wor_tier_include")]
    pub wor: WorTierPolicy,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,

    /// Apply post-FA bullet repair to fix timing violations.
    ///
    /// Uses boundary averaging, gap filling, and selective removal instead
    /// of CLAN FIXBULLETS. Experimental.
    #[serde(default)]
    pub bullet_repair: bool,

    /// Review tier verbosity (none / low-confidence / all).
    #[serde(default)]
    pub review_level: crate::chat_ops::fa::ReviewLevel,

    /// Directory to search for media files (audio/video).
    /// When set, the aligner looks here in addition to the standard
    /// media resolution paths (alongside .cha file, server media roots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_dir: Option<String>,
}

impl Default for AlignOptions {
    fn default() -> Self {
        Self {
            common: CommonOptions::default(),
            fa_engine: default_fa_engine(),
            utr_engine: None,
            utr_overlap_strategy: UtrOverlapStrategy::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: default_wor_tier_include(),
            merge_abbrev: MergeAbbrevPolicy::default(),
            bullet_repair: false,
            review_level: crate::chat_ops::fa::ReviewLevel::default(),
            media_dir: None,
        }
    }
}

impl AlignOptions {
    /// Get the two-pass UTR configuration.
    pub fn two_pass_config(&self) -> &crate::chat_ops::fa::TwoPassConfig {
        &self.utr_two_pass
    }

    /// Return the effective FA engine after applying any shared `fa` override.
    pub fn effective_fa_engine(&self) -> FaEngineName {
        self.common
            .engine_overrides
            .fa
            .clone()
            .unwrap_or_else(|| self.fa_engine.clone())
    }
}

/// Options for the `transcribe` and `transcribe_s` commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscribeOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// ASR engine selector (`rev`, `whisper`, `whisperx`, `whisper_oai`, or
    /// plugin name).
    #[serde(default = "default_asr_engine")]
    pub asr_engine: AsrEngineName,

    /// Enable speaker diarization.
    #[serde(default)]
    pub diarize: bool,

    /// Generate `%wor` tier with word-level timing bullets.
    #[serde(default)]
    pub wor: WorTierPolicy,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,

    /// Operator opt-in to the legacy Stanza constituency-parser
    /// fallback for utseg when no language-specific TalkBank BERT
    /// model is configured for the job's language. Driven by the
    /// `--utseg-fallback-stanza` CLI flag; default refuses
    /// substitution.
    #[serde(default)]
    pub utseg_fallback: UtsegFallbackPolicy,

    /// Whisper batch size.
    #[serde(default = "default_batch_size")]
    pub batch_size: i32,
}

impl TranscribeOptions {
    /// Return the effective ASR engine after applying any shared `asr` override.
    pub fn effective_asr_engine(&self) -> AsrEngineName {
        self.common
            .engine_overrides
            .asr
            .clone()
            .unwrap_or_else(|| self.asr_engine.clone())
    }
}

/// Options for the `morphotag` command.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MorphotagOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Re-tokenize words before morphosyntactic analysis.
    #[serde(default)]
    pub retokenize: bool,

    /// Skip files with multiple `@Languages`.
    #[serde(default)]
    pub skipmultilang: bool,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,

    /// Opt-out: if `true`, suppress the default L2 dispatch and emit
    /// `L2|xxx` placeholders for `@s` (code-switched) words. Default
    /// `false` — L2 dispatch is on.
    #[serde(default)]
    pub no_l2_morphotag: bool,

    /// Opt-out: if `true`, suppress the default `$POS`-hint
    /// post-pass over the morphotagged `%mor` tier. By default,
    /// every main-tier word carrying a `$POS` suffix has its CLAN
    /// tag mapped to a UD UPOS
    /// (`talkbank_model::...::clan_to_ud_upos`); on disagreement
    /// with Stanza's UPOS the `%mor` POS is overridden. Lemma and
    /// features from Stanza are preserved. Default `false` — POS
    /// hints are respected; set via `--no-pos-hints` to opt out.
    #[serde(default)]
    pub no_pos_hints: bool,
}

/// Options for the `translate` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslateOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,
}

/// Options for the `coref` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorefOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,
}

/// Options for the `utseg` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UtsegOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,

    /// Operator opt-in to the legacy Stanza constituency-parser
    /// fallback for utseg when no language-specific TalkBank BERT
    /// model is configured for the job's language. Driven by the
    /// `--utseg-fallback-stanza` CLI flag; default refuses
    /// substitution.
    #[serde(default)]
    pub utseg_fallback: UtsegFallbackPolicy,
}

/// Options for the `benchmark` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// ASR engine selector.
    #[serde(default = "default_asr_engine")]
    pub asr_engine: AsrEngineName,

    /// Generate `%wor` tier with word-level timing bullets.
    #[serde(default)]
    pub wor: WorTierPolicy,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,
}

impl BenchmarkOptions {
    /// Return the effective ASR engine after applying any shared `asr` override.
    pub fn effective_asr_engine(&self) -> AsrEngineName {
        self.common
            .engine_overrides
            .asr
            .clone()
            .unwrap_or_else(|| self.asr_engine.clone())
    }
}

/// Options for the `opensmile` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpensmileOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Feature set to extract (e.g. `"eGeMAPSv02"`, `"ComParE_2016"`).
    #[serde(default = "default_feature_set")]
    pub feature_set: String,
}

/// Options for the `compare` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,

    /// Merge abbreviated forms during processing.
    #[serde(default)]
    pub merge_abbrev: MergeAbbrevPolicy,
}

/// Options for the `avqi` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvqiOptions {
    /// Shared options.
    #[serde(flatten)]
    pub common: CommonOptions,
}

// ---------------------------------------------------------------------------
// CommandOptions tagged enum
// ---------------------------------------------------------------------------

/// Typed per-command options with an internally-tagged `command` discriminator.
///
/// Each variant holds a struct with all options for that command. The `command`
/// tag in the JSON matches the job submission command name, enabling
/// deserialization from the wire format without a separate `command` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum CommandOptions {
    /// `align` — forced alignment.
    Align(AlignOptions),
    /// `transcribe` — ASR transcription.
    Transcribe(TranscribeOptions),
    /// `transcribe_s` — ASR with speaker diarization.
    #[serde(rename = "transcribe_s")]
    TranscribeS(TranscribeOptions),
    /// `translate` — translation.
    Translate(TranslateOptions),
    /// `morphotag` — morphosyntactic analysis.
    Morphotag(MorphotagOptions),
    /// `coref` — coreference resolution.
    Coref(CorefOptions),
    /// `utseg` — utterance segmentation.
    Utseg(UtsegOptions),
    /// `benchmark` — ASR benchmarking.
    Benchmark(BenchmarkOptions),
    /// `opensmile` — audio feature extraction.
    Opensmile(OpensmileOptions),
    /// `compare` — transcript comparison against gold standard.
    Compare(CompareOptions),
    /// `avqi` — voice quality index.
    Avqi(AvqiOptions),
}

impl CommandOptions {
    /// Get the common options shared by all commands.
    pub fn common(&self) -> &CommonOptions {
        match self {
            Self::Align(o) => &o.common,
            Self::Transcribe(o) | Self::TranscribeS(o) => &o.common,
            Self::Translate(o) => &o.common,
            Self::Morphotag(o) => &o.common,
            Self::Coref(o) => &o.common,
            Self::Utseg(o) => &o.common,
            Self::Benchmark(o) => &o.common,
            Self::Opensmile(o) => &o.common,
            Self::Compare(o) => &o.common,
            Self::Avqi(o) => &o.common,
        }
    }

    /// Get a mutable reference to the common options.
    pub fn common_mut(&mut self) -> &mut CommonOptions {
        match self {
            Self::Align(o) => &mut o.common,
            Self::Transcribe(o) | Self::TranscribeS(o) => &mut o.common,
            Self::Translate(o) => &mut o.common,
            Self::Morphotag(o) => &mut o.common,
            Self::Coref(o) => &mut o.common,
            Self::Utseg(o) => &mut o.common,
            Self::Benchmark(o) => &mut o.common,
            Self::Opensmile(o) => &mut o.common,
            Self::Compare(o) => &mut o.common,
            Self::Avqi(o) => &mut o.common,
        }
    }

    /// Abbreviation-merging policy for this command.
    ///
    /// Commands without this option use [`MergeAbbrevPolicy::Keep`].
    pub fn merge_abbrev_policy(&self) -> MergeAbbrevPolicy {
        match self {
            Self::Align(o) => o.merge_abbrev,
            Self::Transcribe(o) | Self::TranscribeS(o) => o.merge_abbrev,
            Self::Translate(o) => o.merge_abbrev,
            Self::Morphotag(o) => o.merge_abbrev,
            Self::Coref(o) => o.merge_abbrev,
            Self::Utseg(o) => o.merge_abbrev,
            Self::Benchmark(o) => o.merge_abbrev,
            Self::Compare(o) => o.merge_abbrev,
            Self::Opensmile(_) | Self::Avqi(_) => MergeAbbrevPolicy::Keep,
        }
    }

    /// Whether abbreviation merging is enabled for this command.
    pub fn merge_abbrev(&self) -> bool {
        self.merge_abbrev_policy().should_merge()
    }

    /// Operator opt-in to the Stanza utseg fallback for this command.
    ///
    /// Commands without an utseg surface return
    /// [`UtsegFallbackPolicy::Refuse`].
    pub fn utseg_fallback_policy(&self) -> UtsegFallbackPolicy {
        match self {
            Self::Transcribe(o) | Self::TranscribeS(o) => o.utseg_fallback,
            Self::Utseg(o) => o.utseg_fallback,
            _ => UtsegFallbackPolicy::Refuse,
        }
    }

    /// Compute the engine overrides string that worker dispatch will actually
    /// use for this command's primary GPU task.
    ///
    /// For align commands, FA dispatch always sets `{"fa":"<engine>"}` in the
    /// request payload (see `execute_v2_engine_overrides` in `execute_v2.rs`),
    /// so the pre-scaled worker must use the same key. Without this, pre-scale
    /// creates a worker with key `""` but FA dispatch looks for `{"fa":"wave2vec"}`,
    /// causing a second worker spawn that the memory guard may block on small
    /// machines (32 GB). See a user incident 2026-04-02.
    ///
    /// The override names here MUST match those in `fa_backend_override_name()`
    /// and `asr_backend_override_name()` in `worker/pool/execute_v2.rs`.
    pub fn dispatch_engine_overrides_json(&self) -> String {
        // If the user explicitly set engine overrides, those take precedence.
        let user = &self.common().engine_overrides;
        if !user.is_empty() {
            return user.to_json_string();
        }

        // Otherwise, derive from the command-specific engine field using the
        // same override names that execute_v2_engine_overrides() produces.
        // Build a typed map and serialize via serde_json to avoid format!-based
        // JSON construction.
        let mut overrides = std::collections::BTreeMap::new();
        match self {
            Self::Align(o) => {
                overrides.insert("fa", o.fa_engine.dispatch_override_name());
            }
            Self::Transcribe(o) | Self::TranscribeS(o) => {
                if let Some(name) = o.effective_asr_engine().dispatch_override_name() {
                    overrides.insert("asr", name);
                }
            }
            Self::Benchmark(o) => {
                if let Some(name) = o.effective_asr_engine().dispatch_override_name() {
                    overrides.insert("asr", name);
                }
            }
            _ => {}
        }
        if overrides.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&overrides).unwrap_or_default()
        }
    }

    /// Get the command name as a string (matches the serde tag value).
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::Align(_) => "align",
            Self::Transcribe(_) => "transcribe",
            Self::TranscribeS(_) => "transcribe_s",
            Self::Translate(_) => "translate",
            Self::Morphotag(_) => "morphotag",
            Self::Coref(_) => "coref",
            Self::Utseg(_) => "utseg",
            Self::Benchmark(_) => "benchmark",
            Self::Opensmile(_) => "opensmile",
            Self::Compare(_) => "compare",
            Self::Avqi(_) => "avqi",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morphotag_roundtrip() {
        let opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),
            retokenize: true,

            ..Default::default()
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn align_roundtrip() {
        let opts = CommandOptions::Align(AlignOptions {
            common: CommonOptions::default(),
            fa_engine: FaEngineName::Whisper,
            utr_engine: Some(UtrEngine::RevAi),
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: true,
            wor: true.into(),
            merge_abbrev: false.into(),
            bullet_repair: false,
            review_level: Default::default(),
            media_dir: None,
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn transcribe_roundtrip() {
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::WhisperX,
            diarize: true,
            wor: false.into(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
            batch_size: 16,
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn transcribe_s_roundtrip() {
        let opts = CommandOptions::TranscribeS(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: true,
            wor: false.into(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
            batch_size: 8,
        });
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains("\"command\":\"transcribe_s\""));
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn command_name_matches_tag() {
        let cases: Vec<(CommandOptions, &str)> = vec![
            (
                CommandOptions::Align(AlignOptions {
                    common: CommonOptions::default(),
                    fa_engine: FaEngineName::Wave2Vec,
                    utr_engine: None,
                    utr_overlap_strategy: Default::default(),
                    utr_two_pass: Default::default(),
                    pauses: false,
                    wor: true.into(),
                    merge_abbrev: false.into(),
                    bullet_repair: false,
                    review_level: Default::default(),
                    media_dir: None,
                }),
                "align",
            ),
            (
                CommandOptions::Morphotag(MorphotagOptions {
                    common: CommonOptions::default(),

                    ..Default::default()
                }),
                "morphotag",
            ),
            (
                CommandOptions::Opensmile(OpensmileOptions {
                    common: CommonOptions::default(),
                    feature_set: "eGeMAPSv02".into(),
                }),
                "opensmile",
            ),
            (
                CommandOptions::Compare(CompareOptions {
                    common: CommonOptions::default(),
                    merge_abbrev: false.into(),
                }),
                "compare",
            ),
            (
                CommandOptions::Avqi(AvqiOptions {
                    common: CommonOptions::default(),
                }),
                "avqi",
            ),
        ];

        for (opts, expected_name) in cases {
            assert_eq!(opts.command_name(), expected_name);
            let json = serde_json::to_string(&opts).unwrap();
            assert!(
                json.contains(&format!("\"command\":\"{expected_name}\"")),
                "JSON should contain command tag '{expected_name}': {json}"
            );
        }
    }

    #[test]
    fn common_accessor() {
        let opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions {
                override_media_cache: true,
                engine_overrides: EngineOverrides::default(),
                mwt: BTreeMap::new(),
                ..Default::default()
            },
            retokenize: true,

            ..Default::default()
        });
        assert!(opts.common().override_media_cache);
    }

    #[test]
    fn engine_overrides_roundtrip() {
        let overrides = EngineOverrides {
            asr: Some(AsrEngineName::HkTencent),
            fa: Some(FaEngineName::Wav2vecCanto),
        };

        let opts = CommandOptions::Align(AlignOptions {
            common: CommonOptions {
                override_media_cache: false,
                engine_overrides: overrides.clone(),
                mwt: BTreeMap::new(),
                ..Default::default()
            },
            fa_engine: FaEngineName::Wav2vecCanto,
            ..AlignOptions::default()
        });

        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.common().engine_overrides, overrides);
    }

    /// `CommonOptions::debug_dir` is typed as `Option<PathBuf>`, but the wire
    /// format must stay a JSON string so existing clients (and the dashboard
    /// schema) keep working. Lock that invariant here.
    #[test]
    fn debug_dir_serializes_as_json_string() {
        let opts = CommonOptions {
            debug_dir: Some(PathBuf::from("/tmp/some/abs/path")),
            ..Default::default()
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(
            json.contains(r#""debug_dir":"/tmp/some/abs/path""#),
            "expected debug_dir as JSON string, got: {json}"
        );
        let back: CommonOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.debug_dir, Some(PathBuf::from("/tmp/some/abs/path")));
    }

    #[test]
    fn transcribe_asr_override_effective_engine_prefers_override() {
        let overrides = EngineOverrides {
            asr: Some(AsrEngineName::HkTencent),
            fa: None,
        };
        let opts = TranscribeOptions {
            common: CommonOptions {
                engine_overrides: overrides,
                ..CommonOptions::default()
            },
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
            batch_size: 8,
        };

        assert_eq!(opts.effective_asr_engine(), AsrEngineName::HkTencent);
    }

    #[test]
    fn benchmark_asr_override_effective_engine_prefers_override() {
        let overrides = EngineOverrides {
            asr: Some(AsrEngineName::HkAliyun),
            fa: None,
        };
        let opts = BenchmarkOptions {
            common: CommonOptions {
                engine_overrides: overrides,
                ..CommonOptions::default()
            },
            asr_engine: AsrEngineName::RevAi,
            wor: true.into(),
            merge_abbrev: false.into(),
        };

        assert_eq!(opts.effective_asr_engine(), AsrEngineName::HkAliyun);
    }

    #[test]
    fn minimal_json_deserializes_with_defaults() {
        let json = r#"{"command": "morphotag"}"#;
        let opts: CommandOptions = serde_json::from_str(json).unwrap();
        assert_eq!(opts.command_name(), "morphotag");
        if let CommandOptions::Morphotag(m) = &opts {
            assert!(!m.retokenize);
            assert!(!m.skipmultilang);
            assert!(!m.merge_abbrev.should_merge());
        } else {
            panic!("expected Morphotag");
        }
    }

    #[test]
    fn avqi_roundtrip() {
        let opts = CommandOptions::Avqi(AvqiOptions {
            common: CommonOptions::default(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn compare_roundtrip() {
        let opts = CommandOptions::Compare(CompareOptions {
            common: CommonOptions::default(),
            merge_abbrev: true.into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn translate_roundtrip() {
        let opts = CommandOptions::Translate(TranslateOptions {
            common: CommonOptions::default(),
            merge_abbrev: true.into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn coref_roundtrip() {
        let opts = CommandOptions::Coref(CorefOptions {
            common: CommonOptions::default(),
            merge_abbrev: false.into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn utseg_roundtrip() {
        let opts = CommandOptions::Utseg(UtsegOptions {
            common: CommonOptions::default(),
            merge_abbrev: true.into(),
            utseg_fallback: false.into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn benchmark_roundtrip() {
        let opts = CommandOptions::Benchmark(BenchmarkOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::WhisperOai,
            wor: true.into(),
            merge_abbrev: false.into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn opensmile_roundtrip() {
        let opts = CommandOptions::Opensmile(OpensmileOptions {
            common: CommonOptions::default(),
            feature_set: "ComParE_2016".into(),
        });
        let json = serde_json::to_string(&opts).unwrap();
        let back: CommandOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn utr_engine_roundtrip_preserves_wire_names() {
        let rev_json = serde_json::to_string(&UtrEngine::RevAi).unwrap();
        let whisper_json = serde_json::to_string(&UtrEngine::Whisper).unwrap();
        let custom_json = serde_json::to_string(&UtrEngine::HkTencent).unwrap();

        assert_eq!(rev_json, "\"rev_utr\"");
        assert_eq!(whisper_json, "\"whisper_utr\"");
        assert_eq!(custom_json, "\"tencent_utr\"");

        assert_eq!(
            serde_json::from_str::<UtrEngine>(&rev_json).unwrap(),
            UtrEngine::RevAi
        );
        assert_eq!(
            serde_json::from_str::<UtrEngine>(&whisper_json).unwrap(),
            UtrEngine::Whisper
        );
        assert_eq!(
            serde_json::from_str::<UtrEngine>(&custom_json).unwrap(),
            UtrEngine::HkTencent
        );
    }

    #[test]
    fn fa_engine_roundtrip_preserves_wire_names() {
        let wav2vec_json = serde_json::to_string(&FaEngineName::Wave2Vec).unwrap();
        let whisper_json = serde_json::to_string(&FaEngineName::Whisper).unwrap();
        let custom_json = serde_json::to_string(&FaEngineName::Wav2vecCanto).unwrap();

        assert_eq!(wav2vec_json, "\"wav2vec_fa\"");
        assert_eq!(whisper_json, "\"whisper_fa\"");
        assert_eq!(custom_json, "\"cantonese_fa\"");

        assert_eq!(
            serde_json::from_str::<FaEngineName>(&wav2vec_json).unwrap(),
            FaEngineName::Wave2Vec
        );
        assert_eq!(
            serde_json::from_str::<FaEngineName>(&whisper_json).unwrap(),
            FaEngineName::Whisper
        );
        assert_eq!(
            serde_json::from_str::<FaEngineName>(&custom_json).unwrap(),
            FaEngineName::Wav2vecCanto
        );
    }

    #[test]
    fn asr_engine_roundtrip_preserves_wire_names() {
        let rev_json = serde_json::to_string(&AsrEngineName::RevAi).unwrap();
        let whisperx_json = serde_json::to_string(&AsrEngineName::WhisperX).unwrap();
        let custom_json = serde_json::to_string(&AsrEngineName::HkTencent).unwrap();

        assert_eq!(rev_json, "\"rev\"");
        assert_eq!(whisperx_json, "\"whisperx\"");
        assert_eq!(custom_json, "\"tencent\"");

        assert_eq!(
            serde_json::from_str::<AsrEngineName>(&rev_json).unwrap(),
            AsrEngineName::RevAi
        );
        assert_eq!(
            serde_json::from_str::<AsrEngineName>(&whisperx_json).unwrap(),
            AsrEngineName::WhisperX
        );
        assert_eq!(
            serde_json::from_str::<AsrEngineName>(&custom_json).unwrap(),
            AsrEngineName::HkTencent
        );
    }

    // -----------------------------------------------------------------------
    // dispatch_engine_overrides_json — pre-scale key must match dispatch key
    // -----------------------------------------------------------------------

    /// Build a `CommandOptions::Align` with the given FA engine and defaults
    /// for all other fields. Uses `AlignOptions::default()` + struct update.
    fn align_opts(fa: FaEngineName) -> CommandOptions {
        CommandOptions::Align(AlignOptions {
            fa_engine: fa,
            ..AlignOptions::default()
        })
    }

    /// Build a `CommandOptions::Align` with the given FA engine and common opts.
    fn align_opts_with(fa: FaEngineName, common: CommonOptions) -> CommandOptions {
        CommandOptions::Align(AlignOptions {
            fa_engine: fa,
            common,
            ..AlignOptions::default()
        })
    }

    #[test]
    fn default_align_dispatch_overrides_include_fa_engine() {
        // A default align job (no user --engine-overrides) uses Wave2Vec FA.
        // The dispatch override must be {"fa":"wave2vec"} — NOT empty string.
        // Empty string was the old bug that caused pre-scale/dispatch key mismatch
        // and memory guard deadlock on 32 GB machines (a user 2026-04-02).
        let opts = align_opts(FaEngineName::Wave2Vec);
        assert_eq!(
            opts.dispatch_engine_overrides_json(),
            r#"{"fa":"wave2vec"}"#,
            "pre-scale key must match FA dispatch key"
        );
    }

    #[test]
    fn whisper_fa_align_dispatch_overrides() {
        let opts = align_opts(FaEngineName::Whisper);
        assert_eq!(opts.dispatch_engine_overrides_json(), r#"{"fa":"whisper"}"#,);
    }

    #[test]
    fn user_engine_override_takes_precedence_over_command_default() {
        // When user sets --engine-overrides '{"fa":"whisper"}', that wins
        // over the command's default fa_engine.
        let opts = align_opts_with(
            FaEngineName::Wave2Vec,
            CommonOptions {
                engine_overrides: EngineOverrides {
                    asr: None,
                    fa: Some(FaEngineName::Whisper),
                },
                ..CommonOptions::default()
            },
        );

        let json = opts.dispatch_engine_overrides_json();
        assert!(
            json.contains("whisper"),
            "user override should take precedence: {json}"
        );
    }

    #[test]
    fn morphotag_dispatch_overrides_empty() {
        // Text-only commands have no command-level engine override.
        let opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        });

        assert_eq!(opts.dispatch_engine_overrides_json(), "");
    }

    #[test]
    fn transcribe_revai_dispatch_overrides_empty() {
        // Rev.AI is cloud-only — no local worker override needed.
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
            batch_size: 8,
        });

        assert_eq!(opts.dispatch_engine_overrides_json(), "");
    }

    #[test]
    fn transcribe_whisper_dispatch_overrides_include_asr() {
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::Whisper,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
            batch_size: 8,
        });

        assert_eq!(
            opts.dispatch_engine_overrides_json(),
            r#"{"asr":"whisper"}"#,
        );
    }
}
