//! CLI argument definitions using clap derive.
//!
//! This module defines the complete argument tree for the `batchalign3` binary:
//!
//! - [`Cli`] -- top-level parser with global options and a subcommand.
//! - [`GlobalOpts`] -- flags that apply to every command (verbosity, server
//!   URL, cache bypass, worker count, etc.). Per-command BA2 aliases (e.g.
//!   `--whisper`, `--rev`, `--diarize` on `transcribe`) live on the
//!   per-command arg structs and are marked `hide = true`; they translate to
//!   the current typed flag values. The Feb-9-BA2 *global* compatibility
//!   flags (`--memlog`, `--adaptive-workers`, `--shared-models`, `--pool`,
//!   etc.) are NOT carried forward — BA3 rejects them at parse time.
//! - [`Commands`] -- the subcommand enum (align, transcribe, morphotag, ...).
//! - Per-command arg structs ([`AlignArgs`], [`TranscribeArgs`], etc.) that
//!   embed [`CommonOpts`] for shared file I/O flags (input paths, output dir,
//!   file list, in-place mode).
//!
//! [`build_typed_options()`] converts the parsed args into a [`CommandOptions`]
//! enum variant for type-safe job submission, translating boolean flag pairs
//! (e.g. `--retokenize` / `--keeptokens`) into their canonical form.

mod commands;
mod global_opts;
mod options;

pub use commands::*;
pub use global_opts::GlobalOpts;
pub use options::*;

use crate::api::ReleasedCommand;
use clap::{Args, Parser, Subcommand};

/// batchalign3 — process .cha and/or audio files.
#[derive(Parser, Debug)]
#[command(name = "batchalign3", version, about)]
pub struct Cli {
    /// Global flags (verbosity, server URL, cache, etc.).
    #[command(flatten)]
    pub global: GlobalOpts,
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Shared options for file I/O across processing commands.
#[derive(Args, Debug, Clone)]
pub struct CommonOpts {
    /// Input paths (files and/or directories).
    pub paths: Vec<std::path::PathBuf>,

    /// Output directory. Omit for in-place modification.
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,

    /// Read input file paths from a text file (one per line).
    #[arg(long)]
    pub file_list: Option<std::path::PathBuf>,

    /// Treat all paths as inputs and modify in-place.
    #[arg(long)]
    pub in_place: bool,
}

/// Incremental-processing options supported only on selected commands.
#[derive(Args, Debug, Clone)]
pub struct IncrementalOpts {
    /// Reference "before" file or directory for incremental processing.
    ///
    /// When provided, the diff engine compares each input file against
    /// its corresponding "before" version and only reprocesses changed
    /// utterances. Unchanged utterances preserve their existing dependent
    /// tiers (%mor, %gra, timing bullets).
    ///
    /// Supported commands: morphotag, align.
    #[arg(long, value_name = "PATH")]
    pub before: Option<std::path::PathBuf>,
}

/// Top-level command enum.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Align transcripts against corresponding media files.
    Align(AlignArgs),
    /// Create a transcript from audio files.
    Transcribe(TranscribeArgs),
    /// Translate the transcript to English.
    Translate(TranslateArgs),
    /// Perform morphosyntactic analysis on transcripts.
    Morphotag(MorphotagArgs),
    /// Perform coreference analysis on transcripts.
    Coref(CorefArgs),
    /// Perform utterance segmentation.
    Utseg(UtsegArgs),
    /// Benchmark ASR word accuracy.
    Benchmark(BenchmarkArgs),
    /// Extract openSMILE audio features.
    Opensmile(OpensmileArgs),
    /// Compare transcripts against gold-standard references.
    Compare(CompareArgs),
    /// Calculate AVQI from paired .cs/.sv audio files.
    Avqi(AvqiArgs),
    /// Initialize ~/.batchalign.ini (ASR defaults / Rev.ai key).
    Setup(SetupArgs),
    /// Manage the processing server.
    Serve(ServeArgs),
    /// List or inspect remote jobs.
    Jobs(JobsArgs),
    /// View, export, or clear run logs.
    Logs(LogsArgs),
    /// Emit Rust-server OpenAPI schema.
    Openapi(OpenapiArgs),
    /// Emit JSON Schema for Rust→Python IPC types.
    IpcSchema(IpcSchemaArgs),
    /// Print version info.
    Version,

    /// Manage the analysis and media caches.
    Cache(CacheArgs),
    /// Model training utilities (delegates to Python training runtime).
    Models(ModelsArgs),
    /// Benchmark command execution time across repeated runs.
    Bench(BenchArgs),

    /// Manage persistent worker daemons (fleet deployment only).
    #[command(hide = true)]
    Worker(WorkerArgs),

    /// Pre-flight diagnostic: validate the worker pipeline on this machine.
    ///
    /// Spawns a test worker, sends known inputs through the morphosyntax
    /// pipeline, and validates the output structure. Catches machine-specific
    /// issues (stale models, missing processors, MWT quirks) before they
    /// become production failures.
    Doctor(DoctorArgs),

    /// Replay a captured failed IPC request against a fresh worker.
    ///
    /// Takes a dump file from ~/.batchalign3/debug/ and sends the exact
    /// request to a new worker, reporting the response.
    Replay(ReplayArgs),

    /// Evaluation subcommands (e.g. `eval l2-morphotag`).
    Eval(EvalArgs),
}

/// Stable processing-command metadata derived from parsed CLI args.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandProfile<'a> {
    /// Typed released command sent to the server/runtime.
    pub command: ReleasedCommand,
    /// Primary language argument for this command.
    pub lang: &'a str,
    /// Requested speaker count for this command.
    pub num_speakers: u32,
    /// File extensions this command should discover.
    pub extensions: &'static [&'static str],
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl CommonOpts {
    /// Extract the stable processing profile for one command.
    pub fn command_profile(cmd: &Commands) -> CommandProfile<'_> {
        match cmd {
            Commands::Align(_) => CommandProfile {
                command: ReleasedCommand::Align,
                lang: "eng",
                num_speakers: 1,
                extensions: &["cha"],
            },
            Commands::Transcribe(a) => {
                let diarize = if a.diarize {
                    true
                } else if a.nodiarize {
                    false
                } else {
                    a.diarization == DiarizationMode::Enabled
                };
                let command = if diarize {
                    ReleasedCommand::TranscribeS
                } else {
                    ReleasedCommand::Transcribe
                };
                CommandProfile {
                    command,
                    lang: &a.lang,
                    num_speakers: a.num_speakers,
                    extensions: &["mp3", "mp4", "wav"],
                }
            }
            Commands::Translate(_a) => CommandProfile {
                command: ReleasedCommand::Translate,
                // Translate has no `--lang`. Source language for each file is
                // read from the file's `@Languages:` header by
                // `dispatch_translate_job`. The translation target is fixed
                // (English) and not surfaced through this field.
                //
                // The wire-level `"per-file"` value parses to
                // `LanguageSpec::PerFile`, which is a first-class state
                // distinct from `Auto` and `Resolved(_)`. Job records,
                // dashboards, and worker pre-warming all see and respect it,
                // so no English placeholder is ever stored or displayed for
                // a translate job.
                lang: "per-file",
                num_speakers: 1,
                extensions: &["cha"],
            },
            Commands::Morphotag(_a) => CommandProfile {
                command: ReleasedCommand::Morphotag,
                // Morphotag has no `--lang`. Per-file inference and provenance
                // come from each file's `@Languages:` header, resolved in
                // `pipeline/morphosyntax.rs::stage_parse` via
                // `resolve_per_file_lang` and read through
                // `MorphosyntaxPipelineContext::require_resolved_lang`.
                //
                // The wire-level `"per-file"` value parses to
                // `LanguageSpec::PerFile`. This replaces the prior
                // English-placeholder hack that survived the 2026-05-03
                // incident only because pipeline code stopped reading it —
                // the placeholder still leaked into job records and worker
                // pre-warming, where it lied about what was actually being
                // processed. `PerFile` makes the absence of a job-level
                // language a first-class type-system state.
                lang: "per-file",
                num_speakers: 1,
                extensions: &["cha"],
            },
            Commands::Coref(_a) => CommandProfile {
                command: ReleasedCommand::Coref,
                // Coref is English-only and takes no `--lang`. The coref
                // pipeline reads English-ness from each file's `@Languages:`
                // header (see `coref.rs::file_has_english`) and hardcodes
                // the inference language to `LanguageCode3::eng()` for the
                // files that pass that gate.
                //
                // The wire-level `"per-file"` value parses to
                // `LanguageSpec::PerFile`. The job record will not carry an
                // English placeholder.
                lang: "per-file",
                num_speakers: 1,
                extensions: &["cha"],
            },
            Commands::Compare(a) => CommandProfile {
                command: ReleasedCommand::Compare,
                lang: &a.lang,
                num_speakers: a.num_speakers,
                extensions: &["cha"],
            },
            Commands::Utseg(a) => CommandProfile {
                command: ReleasedCommand::Utseg,
                lang: &a.lang,
                num_speakers: a.num_speakers,
                extensions: &["cha"],
            },
            Commands::Benchmark(a) => CommandProfile {
                command: ReleasedCommand::Benchmark,
                lang: &a.lang,
                num_speakers: a.num_speakers,
                extensions: &["mp3", "mp4", "wav"],
            },
            Commands::Opensmile(a) => CommandProfile {
                command: ReleasedCommand::Opensmile,
                lang: &a.lang,
                num_speakers: 1,
                extensions: &["mp3", "mp4", "wav"],
            },
            Commands::Avqi(a) => CommandProfile {
                command: ReleasedCommand::Avqi,
                lang: &a.lang,
                num_speakers: 1,
                extensions: &["mp3", "mp4", "wav"],
            },
            // Caller-contract invariant: this method is only called
            // for processing commands (Align, Transcribe, Translate,
            // ...). Non-processing commands (Daemon, Doctor, etc.)
            // are routed by the dispatcher in `dispatch.rs` before
            // reaching here.
            #[allow(clippy::unreachable)]
            _ => unreachable!("not a processing command"),
        }
    }
}

#[cfg(test)]
mod tests;
