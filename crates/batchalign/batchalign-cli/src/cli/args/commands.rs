//! Per-command argument structs and supporting enums.
//!
//! Each processing command (align, transcribe, morphotag, etc.) has its own
//! `*Args` struct embedding [`CommonOpts`](super::CommonOpts) for shared file
//! I/O flags. Utility commands (serve, jobs, logs, cache, etc.) have
//! their own structs and sub-enums here as well.

use clap::{Args, Subcommand, ValueEnum};

use super::{CommonOpts, IncrementalOpts};

// ---------------------------------------------------------------------------
// Engine choice enums
// ---------------------------------------------------------------------------

/// UTR (utterance timing recovery) engine for the `align` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum UtrEngine {
    /// Use Rev.AI utterance timing recovery (default).
    #[default]
    Rev,
    /// Use Whisper for utterance timing recovery.
    Whisper,
}

/// UTR overlap strategy for the `align` command.
///
/// Controls how `+<` (lazy overlap) utterances are handled during
/// utterance timing recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum UtrOverlapStrategy {
    /// Currently equivalent to `global` — the language/content-aware
    /// gate was disabled 2026-03-30 because the two-pass algorithm had
    /// not been validated on operator-reported regression files. See
    /// `runner/dispatch/utr.rs::resolve_strategy()` for the inline
    /// rationale and the book chapter on align for the historical
    /// context. Pass `two-pass` explicitly to opt into the
    /// experimental TwoPassOverlapUtr path.
    #[default]
    Auto,
    /// Single global DP pass (original algorithm). All utterances
    /// participate in one alignment. `+<` utterances get no special
    /// treatment.
    Global,
    /// Two-pass overlap-aware strategy. Pass 1 excludes `+<` utterances
    /// from the global DP. Pass 2 recovers `+<` timing from the
    /// previous utterance's audio window.
    TwoPass,
}

/// Whether CA overlap markers (⌈⌉⌊⌋) are used for alignment windowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CaMarkerPolicy {
    /// Use CA markers for onset windowing when present (default).
    #[default]
    Enabled,
    /// Ignore CA markers — treat all overlaps as `+<` only.
    Disabled,
}

/// Review tier verbosity for the `align` command.
///
/// Controls whether `%xalign` (machine decisions) and `%xrev` (human review
/// prompts) are injected into the output CHAT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum CliReviewLevel {
    /// No review tiers.
    None,
    /// Only flag low-confidence utterances with `%xrev: [?]` (default).
    #[default]
    #[value(name = "low-confidence")]
    LowConfidence,
    /// Add `%xalign` on every bulleted utterance + `%xrev: [?]` on uncertain ones.
    All,
}

/// Forced-alignment engine for the `align` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum FaEngine {
    /// Use Wav2Vec forced alignment (default).
    #[default]
    Wav2vec,
    /// Use Whisper for forced alignment.
    Whisper,
}

/// ASR engine for the `transcribe` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum AsrEngine {
    /// Use Rev.AI ASR (default).
    #[default]
    Rev,
    /// Use Huggingface's Whisper implementation.
    Whisper,
    /// Use a HuggingFace Whisper fine-tune loaded by model_id. The
    /// per-language default is resolved from
    /// ``python/batchalign/models/resolve.py``; override with
    /// ``--engine-overrides '{"asr":"whisper_hub","model_id":"<owner>/<model>"}'``.
    /// See ``book/src/reference/whisper-hub-asr.md``.
    #[value(name = "whisper_hub")]
    WhisperHub,
    /// Use WhisperX.
    #[value(name = "whisperx")]
    WhisperX,
    /// Use OpenAI's Whisper API.
    #[value(name = "whisper-oai")]
    WhisperOai,
}

/// Speaker diarization mode for the `transcribe` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum DiarizationMode {
    /// Automatic (currently defaults to disabled).
    #[default]
    Auto,
    /// Enable speaker diarization.
    Enabled,
    /// Disable speaker diarization.
    Disabled,
}

/// ASR engine for the `benchmark` command (subset of AsrEngine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum BenchAsrEngine {
    /// Use Rev.AI ASR (default).
    #[default]
    Rev,
    /// Use Huggingface's Whisper implementation.
    Whisper,
    /// Use OpenAI's Whisper API.
    #[value(name = "whisper-oai")]
    WhisperOai,
}

// ---------------------------------------------------------------------------
// Processing commands
// ---------------------------------------------------------------------------

/// Arguments for the `align` command.
#[derive(Args, Debug, Clone)]
pub struct AlignArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Incremental-processing options.
    #[command(flatten)]
    pub incremental: IncrementalOpts,

    /// UTR engine: rev (default) or whisper.
    #[arg(long, value_enum, default_value_t)]
    pub utr_engine: UtrEngine,

    /// Explicit custom UTR engine name (e.g. tencent_utr).
    /// Overrides --utr-engine when set.
    #[arg(long)]
    pub utr_engine_custom: Option<String>,

    /// Forced-alignment engine: wav2vec (default) or whisper.
    #[arg(long, value_enum, default_value_t)]
    pub fa_engine: FaEngine,

    /// Explicit custom FA engine name (e.g. wav2vec_fa_canto).
    /// Overrides --fa-engine when set.
    #[arg(long)]
    pub fa_engine_custom: Option<String>,

    /// Directory containing media files for alignment.
    /// Matches by filename stem (file.cha looks for file.mp3/mp4/wav).
    #[arg(long, value_name = "PATH")]
    pub media_dir: Option<String>,

    /// Apply post-FA bullet repair to fix timing violations.
    ///
    /// Uses boundary averaging (small overlaps), gap filling (same-speaker),
    /// and selective removal (large violations) instead of CLAN FIXBULLETS.
    /// Experimental: test on real data before enabling in production.
    #[arg(long)]
    pub bullet_repair: bool,

    /// Emit %xalign/%xrev review tiers documenting alignment decisions.
    ///
    /// low-confidence (default when --bullet-repair is set): only flag
    /// uncertain utterances with %xrev: [?] for human review.
    /// all: add %xalign on every bulleted utterance.
    /// none: no review tiers.
    #[arg(long, value_enum, default_value_t)]
    pub review_level: CliReviewLevel,

    /// Try to add pauses between words by grouping them.
    #[arg(long)]
    pub pauses: bool,

    /// Write word-level alignment (%wor) tier.
    #[arg(long, default_value_t = true)]
    pub wor: bool,

    /// Disable %wor tier.
    #[arg(long, conflicts_with = "wor")]
    pub nowor: bool,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,

    /// Include utterance timing recovery before forced alignment.
    #[arg(long, default_value_t = true)]
    pub utr: bool,

    /// Skip UTR (faster, but untimed files may get incomplete alignment).
    #[arg(long, conflicts_with = "utr")]
    pub no_utr: bool,

    /// UTR overlap strategy: auto (default), global, or two-pass.
    #[arg(long, value_enum, default_value_t)]
    pub utr_strategy: UtrOverlapStrategy,

    /// Use CA overlap markers (⌈⌉⌊⌋) for alignment windowing: enabled (default), disabled.
    #[arg(long, value_enum, default_value_t)]
    pub utr_ca_markers: CaMarkerPolicy,

    /// Max overlap density before skipping pass-1 exclusion (0.0–1.0, default 0.30).
    #[arg(long, default_value_t = 0.30)]
    pub utr_density_threshold: f64,

    /// Tight window buffer for pass-2 backchannel recovery (ms, default 500).
    #[arg(long, default_value_t = 500)]
    pub utr_tight_buffer: u64,

    /// UTR word matching threshold. Default: 0.85 (fuzzy matching enabled).
    ///
    /// Uses Jaro-Winkler similarity to match ASR words against transcript
    /// words even when they differ slightly (e.g., "gonna"/"gona"). Set to
    /// 1.0 for exact matching only. The threshold controls how similar words
    /// must be (0.0–1.0, higher = stricter).
    #[arg(long)]
    pub utr_fuzzy: Option<f64>,

    // -- Hidden BA2 compatibility aliases --
    /// BA2 compat: use --utr-engine whisper instead.
    #[arg(long, hide = true)]
    pub whisper: bool,

    /// BA2 compat: use --utr-engine rev instead.
    #[arg(long, hide = true)]
    pub rev: bool,

    /// BA2 compat: use --fa-engine whisper instead.
    #[arg(long, hide = true)]
    pub whisper_fa: bool,

    /// BA2 compat: use --fa-engine wav2vec instead.
    #[arg(long, hide = true)]
    pub wav2vec: bool,
}

/// Arguments for the `transcribe` command.
#[derive(Args, Debug, Clone)]
pub struct TranscribeArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// ASR engine: rev (default), whisper, whisperx, or whisper-oai.
    #[arg(long, value_enum, default_value_t)]
    pub asr_engine: AsrEngine,

    /// Explicit custom ASR engine name (e.g. tencent, funaudio).
    /// Overrides --asr-engine when set.
    #[arg(long)]
    pub asr_engine_custom: Option<String>,

    /// Speaker diarization mode: auto (default), enabled, or disabled.
    #[arg(long, value_enum, default_value_t)]
    pub diarization: DiarizationMode,

    /// Write word-level alignment (%wor) tier.
    #[arg(long, conflicts_with = "nowor")]
    pub wor: bool,

    /// Disable %wor tier (default).
    #[arg(long, conflicts_with = "wor")]
    pub nowor: bool,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,

    /// Opt in to the legacy Stanza constituency-parser fallback for
    /// utterance segmentation when no language-specific TalkBank BERT
    /// model is configured for `--lang`. Default refuses substitution;
    /// pass this flag to permit the same Stanza-based segmenter that
    /// Batchalign 2 used for unsupported languages (quality varies).
    #[arg(long)]
    pub utseg_fallback_stanza: bool,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Number of speakers.
    #[arg(short = 'n', long, default_value_t = 2)]
    pub num_speakers: u32,

    // -- Hidden BA2 compatibility aliases --
    /// BA2 compat: use --asr-engine whisper instead.
    #[arg(long, hide = true)]
    pub whisper: bool,

    /// BA2 compat: use --asr-engine whisperx instead.
    #[arg(long, hide = true)]
    pub whisperx: bool,

    /// BA2 compat: use --asr-engine whisper-oai instead.
    #[arg(long, hide = true)]
    pub whisper_oai: bool,

    /// BA2 compat: use --asr-engine rev instead.
    #[arg(long, hide = true)]
    pub rev: bool,

    /// BA2 compat: use --diarization enabled instead.
    #[arg(long, hide = true)]
    pub diarize: bool,

    /// BA2 compat: use --diarization disabled instead.
    #[arg(long, hide = true)]
    pub nodiarize: bool,
}

/// Arguments for the `translate` command.
///
/// **No `--lang` flag.** BA2 parity (`~/batchalign2-master/batchalign/cli/cli.py`
/// `translate` command takes no `--lang`). Source language is read per-file
/// from the CHAT file's `@Languages:` header (BA2
/// `pipelines/translate/seamless.py:40` uses `doc.langs[0]`); the
/// translation target is hardcoded to English (BA2 `seamless.py:41`
/// `tgt_lang="eng"`). The 2026-05-03 morphotag incident showed that a
/// job-level lang sentinel silently overrides per-file routing — do not
/// re-introduce `--lang` here without re-reading that postmortem.
#[derive(Args, Debug, Clone)]
pub struct TranslateArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,
}

/// Arguments for the `morphotag` command.
///
/// **No `--lang` flag.** BA2 parity (`~/batchalign2-master/batchalign/cli/cli.py`
/// `morphotag` command takes no `--lang`). The processing language is read
/// per-file from the CHAT file's `@Languages:` header (see
/// `pipeline/morphosyntax.rs::stage_parse`). Files whose primary language
/// is not Stanza-supported hard-error out; the daemon does not silently
/// rewrite them with English morphotag (the 2026-05-03 incident).
#[derive(Args, Debug, Clone)]
pub struct MorphotagArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Incremental-processing options.
    #[command(flatten)]
    pub incremental: IncrementalOpts,

    /// Retokenize the main line to fit UD tokenizations.
    ///
    /// WARNING: This modifies the main tier text to match Stanza's UD
    /// tokenization (splitting/merging words). Existing word-level timing
    /// bullets and %wor tiers may become stale. Use --before for incremental
    /// processing to preserve unaffected utterances.
    #[arg(long, conflicts_with = "keeptokens")]
    pub retokenize: bool,

    /// Keep existing tokenization (default).
    #[arg(long, conflicts_with = "retokenize")]
    pub keeptokens: bool,

    /// Skip code switching.
    #[arg(long, conflicts_with = "multilang")]
    pub skipmultilang: bool,

    /// Keep multilingual spans (default).
    #[arg(long, conflicts_with = "skipmultilang")]
    pub multilang: bool,

    /// Opt out of L2 dispatch for `@s` (code-switched) words.
    ///
    /// By default morphotag routes `@s` words to the secondary-language
    /// Stanza path and splices the resulting morphology back into `%mor`.
    /// Pass `--no-l2-morphotag` to keep the legacy `L2|xxx` placeholders
    /// instead.
    #[arg(long, default_value_t = false)]
    pub no_l2_morphotag: bool,

    /// Opt out of transcriber-supplied `$POS` hint respect. By
    /// default batchalign3 walks every main-tier word carrying a
    /// `$POS` suffix after Stanza finishes morphotag, maps the CLAN
    /// tag to a UD UPOS, and overrides the `%mor` POS category when
    /// Stanza disagrees. Lemma and morphological features from
    /// Stanza are preserved. Pass `--no-pos-hints` to suppress the
    /// override pass and keep Stanza's POS as-is.
    #[arg(long, default_value_t = false)]
    pub no_pos_hints: bool,

    /// Comma-separated manual lexicon override file.
    #[arg(long)]
    pub lexicon: Option<String>,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,
}

/// Arguments for the `coref` command.
/// Arguments for the `coref` command.
///
/// **No `--lang` flag.** BA2 parity (`~/batchalign2-master/batchalign/cli/cli.py`
/// `coref` command takes no `--lang`). Coref is English-only — non-English
/// files pass through unchanged based on the per-file `@Languages:` header
/// (see `coref.rs::file_has_english`). Re-introducing `--lang` here would
/// recreate the 2026-05-03 morphotag failure mode where a job-level sentinel
/// silently overrode per-file language routing.
#[derive(Args, Debug, Clone)]
pub struct CorefArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,
}

/// Arguments for the `compare` command.
#[derive(Args, Debug, Clone)]
pub struct CompareArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Number of speakers.
    #[arg(short = 'n', long, default_value_t = 2)]
    pub num_speakers: u32,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,
}

/// Arguments for the `utseg` command.
#[derive(Args, Debug, Clone)]
pub struct UtsegArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Number of speakers.
    #[arg(short = 'n', long, default_value_t = 2)]
    pub num_speakers: u32,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,

    /// Opt in to the legacy Stanza constituency-parser fallback for
    /// utterance segmentation when no language-specific TalkBank BERT
    /// model is configured for `--lang`. Default refuses substitution;
    /// pass this flag to permit the same Stanza-based segmenter that
    /// Batchalign 2 used for unsupported languages (quality varies).
    #[arg(long)]
    pub utseg_fallback_stanza: bool,
}

/// Arguments for the `benchmark` command.
#[derive(Args, Debug, Clone)]
pub struct BenchmarkArgs {
    /// Shared file I/O options.
    #[command(flatten)]
    pub common: CommonOpts,

    /// ASR engine: rev (default), whisper, or whisper-oai.
    #[arg(long, value_enum, default_value_t)]
    pub asr_engine: BenchAsrEngine,

    /// Explicit custom ASR engine name (e.g. tencent, funaudio).
    /// Overrides --asr-engine when set.
    #[arg(long)]
    pub asr_engine_custom: Option<String>,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Number of speakers.
    #[arg(short = 'n', long, default_value_t = 2)]
    pub num_speakers: u32,

    /// Write word-level alignment (%wor) tier.
    #[arg(long, conflicts_with = "nowor")]
    pub wor: bool,

    /// Disable %wor tier (default).
    #[arg(long, conflicts_with = "wor")]
    pub nowor: bool,

    /// Merge abbreviations in output.
    #[arg(long, conflicts_with = "no_merge_abbrev")]
    pub merge_abbrev: bool,

    /// Do not merge abbreviations in output (default).
    #[arg(long = "no-merge-abbrev", conflicts_with = "merge_abbrev")]
    pub no_merge_abbrev: bool,

    /// Server media bank name (from server.yaml media_mappings).
    #[arg(long)]
    pub bank: Option<String>,

    /// Subdirectory under the bank.
    #[arg(long)]
    pub subdir: Option<String>,

    // -- Hidden BA2 compatibility aliases --
    /// BA2 compat: use --asr-engine whisper instead.
    #[arg(long, hide = true)]
    pub whisper: bool,

    /// BA2 compat: use --asr-engine whisper-oai instead.
    #[arg(long, hide = true)]
    pub whisper_oai: bool,

    /// BA2 compat: use --asr-engine rev instead.
    #[arg(long, hide = true)]
    pub rev: bool,
}

/// Arguments for the `opensmile` command.
#[derive(Args, Debug, Clone)]
pub struct OpensmileArgs {
    /// Input directory.
    pub input_dir: std::path::PathBuf,
    /// Output directory.
    pub output_dir: std::path::PathBuf,

    /// Feature set to extract.
    #[arg(long, default_value = "eGeMAPSv02",
          value_parser = ["eGeMAPSv02", "eGeMAPSv01b", "GeMAPSv01b", "ComParE_2016"])]
    pub feature_set: String,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Server media bank name.
    #[arg(long)]
    pub bank: Option<String>,

    /// Subdirectory under the bank.
    #[arg(long)]
    pub subdir: Option<String>,
}

/// Arguments for the `avqi` command.
#[derive(Args, Debug, Clone)]
pub struct AvqiArgs {
    /// Input directory containing paired .cs/.sv audio files.
    pub input_dir: std::path::PathBuf,
    /// Output directory.
    pub output_dir: std::path::PathBuf,

    /// Language (3-letter ISO code).
    #[arg(long, default_value = "eng")]
    pub lang: String,
}

/// ASR engine choice for the `setup` command.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupEngine {
    /// Rev.AI ASR engine.
    Rev,
    /// Huggingface Whisper ASR engine.
    Whisper,
}

/// Processing command target for the `bench` subcommand.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchTarget {
    /// Forced alignment.
    Align,
    /// Transcription (single speaker).
    Transcribe,
    /// Transcription with speaker diarization.
    #[value(name = "transcribe_s")]
    TranscribeS,
    /// Morphosyntactic tagging.
    Morphotag,
    /// Translation.
    Translate,
    /// Utterance segmentation.
    Utseg,
    /// WER benchmarking.
    Benchmark,
    /// OpenSMILE feature extraction.
    Opensmile,
    /// Coreference resolution.
    Coref,
    /// Transcript comparison against gold standard.
    Compare,
}

impl BenchTarget {
    /// Return the server-side command name string for this target.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Align => "align",
            Self::Transcribe => "transcribe",
            Self::TranscribeS => "transcribe_s",
            Self::Morphotag => "morphotag",
            Self::Translate => "translate",
            Self::Utseg => "utseg",
            Self::Benchmark => "benchmark",
            Self::Opensmile => "opensmile",
            Self::Coref => "coref",
            Self::Compare => "compare",
        }
    }
}

/// Arguments for the `bench` subcommand (repeated benchmark runs).
#[derive(Args, Debug, Clone)]
pub struct BenchArgs {
    /// Command to benchmark.
    pub command: BenchTarget,

    /// Input directory.
    pub in_dir: std::path::PathBuf,

    /// Output directory.
    pub out_dir: std::path::PathBuf,

    /// Number of benchmark runs.
    #[arg(long, default_value_t = 1)]
    pub runs: usize,

    /// Dataset label for structured output.
    #[arg(long)]
    pub dataset: Option<String>,

    /// Number of workers to use.
    #[arg(long)]
    pub workers: Option<usize>,

    /// Use cache for benchmark runs (default is to bypass cache).
    #[arg(long)]
    pub use_cache: bool,
}

/// Arguments for the `models` subcommand (model training utilities).
#[derive(Args, Debug, Clone)]
pub struct ModelsArgs {
    /// Subcommand.
    #[command(subcommand)]
    pub action: ModelsAction,
}

/// Model training subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum ModelsAction {
    /// Extract training text from CHAT files (Rust-native, no CLAN needed).
    Prep(ModelsPrepArgs),
    /// Train a model (forwards to Python training runtime).
    Train(ModelsTrainArgs),
}

/// Arguments for `models prep`.
#[derive(Args, Debug, Clone)]
pub struct ModelsPrepArgs {
    /// Run name (used as prefix for output files).
    pub run_name: String,

    /// Input directory containing .cha files.
    pub input_dir: std::path::PathBuf,

    /// Output directory for prepared .train.txt and .val.txt files.
    pub output_dir: std::path::PathBuf,

    /// Minimum word count per utterance (shorter utterances are excluded).
    #[arg(long, default_value_t = 10)]
    pub min_length: usize,

    /// Separate validation directory. If not given, splits from input.
    #[arg(long)]
    pub val_dir: Option<String>,

    /// Fraction of data to use for validation when --val-dir is not given.
    #[arg(long, default_value_t = 0.1)]
    pub val_fraction: f64,
}

/// Arguments for `models train` (forwarded to Python).
#[derive(Args, Debug, Clone)]
pub struct ModelsTrainArgs {
    /// Arguments passed through to `python -m batchalign.models.training.run`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Arguments for the `setup` subcommand (initialize `~/.batchalign.ini`).
#[derive(Args, Debug, Clone)]
pub struct SetupArgs {
    /// Default ASR engine to persist in ~/.batchalign.ini.
    #[arg(long, value_enum)]
    pub engine: Option<SetupEngine>,

    /// Rev.ai API key (required with --engine rev in non-interactive mode).
    #[arg(long)]
    pub rev_key: Option<String>,

    /// Disable prompts and rely only on flags.
    #[arg(long)]
    pub non_interactive: bool,
}

// ---------------------------------------------------------------------------
// Utility commands
// ---------------------------------------------------------------------------

/// Arguments for the `serve` subcommand.
#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Serve action (start, stop, status).
    #[command(subcommand)]
    pub action: ServeAction,
}

/// Server lifecycle actions.
#[derive(Subcommand, Debug, Clone)]
pub enum ServeAction {
    /// Start the processing server.
    Start(ServeStartArgs),
    /// Stop the processing server.
    Stop,
    /// Check server health and status.
    Status(ServeStatusArgs),
}

/// Arguments for `serve start`.
#[derive(Args, Debug, Clone)]
pub struct ServeStartArgs {
    /// Port to listen on (defaults to server.yaml or 8000).
    #[arg(long)]
    pub port: Option<u16>,

    /// Host to bind to (defaults to server.yaml or 0.0.0.0).
    #[arg(long)]
    pub host: Option<String>,

    /// Path to server.yaml config file.
    #[arg(long)]
    pub config: Option<String>,

    /// Python executable used to spawn worker processes.
    #[arg(long, env = "BATCHALIGN_PYTHON")]
    pub python: Option<String>,

    /// Start workers in test-echo mode (debugging only; no ML models).
    #[arg(long)]
    pub test_echo: bool,

    /// Run in foreground (don't daemonize).
    #[arg(long)]
    pub foreground: bool,

    /// Warmup configuration: a preset (`off`, `minimal`, `full`) or a
    /// comma-separated list of commands to pre-load (e.g. `align,morphotag`).
    ///
    /// Presets expand to built-in command lists:
    ///   - `off`     — no warmup (workers spawn on first job)
    ///   - `minimal` — morphotag only
    ///   - `full`    — morphotag, align, transcribe
    ///
    /// When a command list is given, only those commands are warmed up.
    /// Default (no flag): uses `server.yaml` config or `full`.
    #[arg(long)]
    pub warmup: Option<String>,

    /// Maximum concurrent files per job. Overrides the `max_workers_per_job`
    /// value from `server.yaml`. 0 = auto-tune.
    #[arg(long)]
    pub workers: Option<usize>,

    /// Inference timeout in seconds for audio tasks (ASR, FA, speaker).
    /// Increase for very long recordings. Default: 1800 (30 minutes).
    #[arg(long)]
    pub timeout: Option<u64>,
}

/// Arguments for `serve status`.
#[derive(Args, Debug, Clone)]
pub struct ServeStatusArgs {
    /// Server URL to check.
    #[arg(long)]
    pub server: Option<String>,
}

/// Arguments for the `jobs` subcommand.
#[derive(Args, Debug, Clone)]
pub struct JobsArgs {
    /// Sub-action (e.g. `cancellations`). Backward-compat: no
    /// sub-action falls through to the original positional /
    /// flag-driven `list` / `show` behaviour.
    #[command(subcommand)]
    pub action: Option<JobsAction>,

    /// Job ID to inspect (legacy positional form — equivalent to
    /// `jobs show <id>`). Without `--server`, this inspects local
    /// job artifacts.
    pub job_id: Option<String>,

    /// Server URL (or set BATCHALIGN_SERVER env var) for remote job listing/detail.
    #[arg(long, env = "BATCHALIGN_SERVER")]
    pub server: Option<String>,

    /// Emit machine-readable JSON instead of the default human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// `jobs` sub-actions. New work nests here; legacy callers using
/// the positional `jobs <id>` form continue working through the
/// `action: None` fallback in `JobsArgs`.
#[derive(Subcommand, Debug, Clone)]
pub enum JobsAction {
    /// Print the cancellation audit history for one job. Use this
    /// when a user reports "I didn't cancel that job" — every
    /// cancel attempt is recorded with `source` (tui / api /
    /// dashboard / staging / signal), `host`, `pid`, `reason`, and
    /// `in_flight_filename`.
    Cancellations(JobsCancellationsArgs),
}

/// Arguments for `jobs cancellations <id>`.
#[derive(Args, Debug, Clone)]
pub struct JobsCancellationsArgs {
    /// Job ID whose cancellation history should be printed.
    pub job_id: String,

    /// Server URL (or set BATCHALIGN_SERVER env var).
    #[arg(long, env = "BATCHALIGN_SERVER")]
    pub server: Option<String>,

    /// Emit machine-readable JSON instead of the default human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the `logs` subcommand.
#[derive(Args, Debug, Clone)]
pub struct LogsArgs {
    /// Show the most recent run log.
    #[arg(long)]
    pub last: bool,

    /// Show raw JSONL event lines (with --last).
    #[arg(long)]
    pub raw: bool,

    /// Export recent logs to a zip file.
    #[arg(long)]
    pub export: bool,

    /// Delete all log files.
    #[arg(long)]
    pub clear: bool,

    /// Live-tail the newest log file (Ctrl-C to stop).
    #[arg(long)]
    pub follow: bool,

    /// Number of recent runs to list.
    #[arg(short = 'n', long, default_value_t = 10)]
    pub count: usize,
}

/// Arguments for the `openapi` subcommand.
#[derive(Args, Debug, Clone)]
pub struct OpenapiArgs {
    /// Output path for OpenAPI JSON.
    ///
    /// In normal mode, if omitted, schema is written to stdout.
    /// In `--check` mode, if omitted, defaults to `openapi.json`.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Verify that the target file already matches the generated schema.
    ///
    /// This mode does not modify files and exits non-zero on schema drift.
    #[arg(long)]
    pub check: bool,
}

/// Arguments for the `ipc-schema` subcommand.
#[derive(Args, Debug, Clone)]
pub struct IpcSchemaArgs {
    /// Output directory for JSON Schema files.
    ///
    /// If omitted, schemas are written to stdout as a single JSON object.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Verify that the target directory already matches the generated schemas.
    ///
    /// This mode does not modify files and exits non-zero on schema drift.
    #[arg(long)]
    pub check: bool,
}

/// Arguments for the `cache` subcommand.
#[derive(Args, Debug, Clone)]
pub struct CacheArgs {
    /// Cache action (stats or clear).
    #[command(subcommand)]
    pub action: Option<CacheAction>,

    /// Show cache statistics (BA2-compatible flag form).
    #[arg(long)]
    pub stats: bool,

    /// Clear cache (BA2-compatible flag form).
    #[arg(long)]
    pub clear: bool,

    /// Also remove permanent UTR cache entries (with --clear).
    #[arg(long, requires = "clear")]
    pub all: bool,

    /// Skip confirmation prompt (with --clear).
    #[arg(short = 'y', long, requires = "clear")]
    pub yes: bool,
}

/// Cache management actions.
#[derive(Subcommand, Debug, Clone)]
pub enum CacheAction {
    /// Show cache statistics.
    Stats,
    /// Clear cached data.
    Clear(CacheClearArgs),
}

/// Arguments for `cache clear`.
#[derive(Args, Debug, Clone)]
pub struct CacheClearArgs {
    /// Also remove permanent UTR cache entries.
    #[arg(long)]
    pub all: bool,
    /// Skip confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,
}

// ---------------------------------------------------------------------------
// Worker daemon management
// ---------------------------------------------------------------------------

/// Arguments for `batchalign3 worker`.
#[derive(Args, Debug, Clone)]
pub struct WorkerArgs {
    /// Worker action (start, list, stop).
    #[command(subcommand)]
    pub action: WorkerAction,
}

/// Worker management actions.
#[derive(Subcommand, Debug, Clone)]
pub enum WorkerAction {
    /// Start a worker as a foreground daemon.
    Start(WorkerStartArgs),
    /// List active workers from the registry.
    List,
    /// Stop one or all workers.
    Stop(WorkerStopArgs),
}

/// Arguments for `worker start`.
#[derive(Args, Debug, Clone)]
pub struct WorkerStartArgs {
    /// Worker profile: gpu, stanza, or io.
    #[arg(long)]
    pub profile: String,
    /// 3-letter ISO language code (e.g. eng, fra, yue).
    #[arg(long, default_value = "eng")]
    pub lang: String,
    /// TCP port to listen on (0 = auto-assign from 9100-9199).
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// TCP bind address.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Engine overrides as JSON (e.g. '{"asr":"tencent"}').
    #[arg(long, default_value = "")]
    pub engine_overrides: String,
}

/// Arguments for `worker stop`.
#[derive(Args, Debug, Clone)]
pub struct WorkerStopArgs {
    /// Stop the worker on this port.
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// Stop all workers matching this profile.
    #[arg(long, default_value = "")]
    pub profile: String,
    /// Stop all workers matching this language.
    #[arg(long, default_value = "")]
    pub lang: String,
    /// Stop all registered workers.
    #[arg(long)]
    pub all: bool,
}

// ---------------------------------------------------------------------------
// Doctor command
// ---------------------------------------------------------------------------

/// Pre-flight diagnostic arguments.
#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {
    /// Language to test (default: eng).
    #[arg(long, default_value = "eng")]
    pub lang: String,

    /// Output format.
    #[arg(long, default_value = "human")]
    pub format: DoctorFormat,

    /// Custom Python path (overrides BATCHALIGN_PYTHON).
    #[arg(long)]
    pub python: Option<String>,

    /// Skip the Python worker-pipeline checks; only inspect host
    /// facts and validate the deployed `server.yaml`. Fast (no
    /// Python spawn, no model load) and intended for operators
    /// verifying config sanity before deploying or restarting.
    /// Exits non-zero on host-facts validation errors.
    #[arg(long)]
    pub check: bool,

    /// Trace why one resolved knob has its current value. Prints the
    /// resolved value, whether it came from an operator override or
    /// the host-facts recommendation, the rule that produced the
    /// recommendation, and the relevant detected facts. Implies
    /// `--check` (skips worker pipeline). Valid knob names:
    /// `gpu_thread_pool_size`, `force_cpu`, `max_total_workers`,
    /// `max_concurrent_jobs`, `max_workers_per_key`,
    /// `memory_gate_mb`.
    #[arg(long, value_name = "KNOB")]
    pub explain: Option<String>,

    /// Treat host-facts validation warnings as fatal: exit non-zero
    /// when any warning fires, not only when an error fires.
    /// Intended for CI gates that want zero-warning deployments.
    /// Has no effect outside `--check` and `--explain` paths
    /// (the worker-pipeline path doesn't run host-facts validation
    /// today).
    #[arg(long)]
    pub warnings_as_errors: bool,
}

/// Doctor output format.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum DoctorFormat {
    /// Human-readable summary.
    Human,
    /// JSON output for machine consumption.
    Json,
}

// ---------------------------------------------------------------------------
// Replay command
// ---------------------------------------------------------------------------

/// Replay a captured failed IPC request.
#[derive(Args, Debug, Clone)]
pub struct ReplayArgs {
    /// Path to a failed_ipc_*.json dump file.
    pub dump_file: std::path::PathBuf,

    /// Language override (uses dump file's worker label if omitted).
    #[arg(long)]
    pub lang: Option<String>,

    /// Custom Python path (overrides BATCHALIGN_PYTHON).
    #[arg(long)]
    pub python: Option<String>,
}

// ---------------------------------------------------------------------------
// Eval subcommand tree
// ---------------------------------------------------------------------------

/// Arguments for `batchalign3 eval`.
#[derive(Args, Debug, Clone)]
pub struct EvalArgs {
    /// Evaluation action.
    #[command(subcommand)]
    pub action: EvalAction,
}

/// Evaluation actions — starts with `l2-morphotag`; more can land here.
#[derive(Subcommand, Debug, Clone)]
pub enum EvalAction {
    /// L2 morphotag evaluation: pair `@s` words with `%mor` / `%gra` items
    /// using a typed AST walk (supersedes `scripts/l2-eval/analyze.py`).
    #[command(name = "l2-morphotag")]
    L2Morphotag(L2MorphotagEvalArgs),
}

/// Arguments for `batchalign3 eval l2-morphotag`.
///
/// The typical workflow:
/// 1. Run `batchalign3 morphotag` (L2 dispatch is on by default) on the
///    eval set and place the output CHAT files in a directory.
/// 2. Run `batchalign3 eval l2-morphotag --eval-set eval-set.jsonl
///    --morphotag-output <dir> --output <report-dir>/`.
///
/// The eval set is a JSONL file with one `{ "path": ..., "pair_key": ... }`
/// object per line (produced by `scripts/l2-eval/select_eval_set.py`).
#[derive(Args, Debug, Clone)]
pub struct L2MorphotagEvalArgs {
    /// JSONL file listing input CHAT files with their `pair_key` labels.
    #[arg(long, value_name = "JSONL")]
    pub eval_set: std::path::PathBuf,

    /// Directory (flat or nested) of post-morphotag CHAT files.
    /// Matched against the eval set by filename basename.
    #[arg(long, value_name = "DIR")]
    pub morphotag_output: std::path::PathBuf,

    /// Directory to write `per-word.csv`, `per-pair.csv`,
    /// `flagged.csv`, and `summary.md`.
    #[arg(long, value_name = "DIR")]
    pub output: std::path::PathBuf,
}
