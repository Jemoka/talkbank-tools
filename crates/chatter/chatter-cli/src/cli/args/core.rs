use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use super::clan_commands::ClanCommands;

pub use crate::ui::ThemePreset;

/// TalkBank utilities for CHAT format validation and transformation
#[derive(Parser)]
#[command(name = "chatter", version, long_version = concat!(env!("CARGO_PKG_VERSION"), " (build ", env!("BUILD_HASH"), ")"))]
#[command(
    about = "Tools for validating and transforming TalkBank CHAT files",
    long_about = None,
    after_long_help = "\
Getting started:
  chatter validate myfile.cha          Validate a CHAT file
  chatter validate corpus/             Validate an entire corpus
  chatter clan freq myfile.cha         Run frequency analysis
  chatter to-json myfile.cha           Convert to JSON
  chatter to-xml myfile.cha            Export TalkBank XML

Exit codes:
  0    All files valid / command succeeded
  1    Validation errors found or command failed
  2    Invalid arguments or missing required options

Full documentation: https://talkbank.org/tools/"
)]
pub struct Cli {
    /// Logging verbosity level (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Logging output format
    #[arg(long, value_enum, default_value = "text", global = true)]
    pub log_format: LogFormat,

    /// TUI mode: auto (detect terminal), force (always), disable (never)
    #[arg(long, value_enum, default_value_t, global = true)]
    pub tui_mode: TuiMode,

    /// Color theme for TUI mode
    #[arg(long, value_enum, global = true)]
    pub theme: Option<ThemePreset>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Supported formats for tracing output.
#[derive(Debug, Clone, ValueEnum)]
pub enum LogFormat {
    /// Human-readable text format
    Text,
    /// JSON format for observability/telemetry tools
    Json,
}

/// Controls whether the interactive TUI is used for validation output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum TuiMode {
    /// Automatically detect terminal capability (TUI when stdout is a TTY)
    #[default]
    Auto,
    /// Force TUI mode regardless of terminal detection
    Force,
    /// Disable TUI mode even in interactive terminals
    Disable,
}

impl TuiMode {
    /// Resolve the mode into a concrete decision, consulting the terminal when `Auto`.
    pub fn should_use_tui(self) -> bool {
        match self {
            Self::Force => true,
            Self::Disable => false,
            Self::Auto => atty::is(atty::Stream::Stdout),
        }
    }
}

/// Output encodings for command results.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable validation output
    Text,
    /// Structured JSON output
    Json,
}

/// Which parser backend to use for CHAT parsing.
///
/// Tree-sitter (default) supports incremental reparsing and is used by the LSP.
/// Re2c is a DFA-based parser that is faster for batch validation.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ParserBackend {
    /// Tree-sitter parser (default, supports incremental reparsing)
    #[default]
    TreeSitter,
    /// Re2c DFA parser (faster batch validation)
    Re2c,
}

/// Dependent tiers that `show-alignment` can filter on.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum AlignmentTier {
    /// Morphology tier (%mor)
    Mor,
    /// Grammar tier (%gra)
    Gra,
    /// Phonology tier (%pho)
    Pho,
    /// Syntax tier (%sin)
    Sin,
}

/// Top-level `talkbank` subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Validate CHAT file(s)
    Validate {
        /// Path(s) to CHAT file(s) or directory(ies).
        ///
        /// Not required when `--list-checks` is supplied, since that mode
        /// exits before reading any files.
        #[arg(required_unless_present = "list_checks")]
        path: Vec<PathBuf>,

        /// Print every validation check and its Active/Planned status, then exit.
        ///
        /// Does not read any files. The list is derived from the
        /// `ErrorCode` enum and a hard-coded Planned list that mirrors
        /// `resources/spec/errors/*.md` statuses.
        #[arg(
            long,
            help = "List all validation checks with Active/Planned status, then exit"
        )]
        list_checks: bool,

        /// Output format: text (default) or json
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text, help = "Validation output style (text|json)")]
        format: OutputFormat,

        /// Skip tier alignment (validation includes alignment by default)
        #[arg(
            long = "skip-alignment",
            help = "Disable dependent tier alignment checks (alignment is on by default)"
        )]
        skip_alignment: bool,

        /// Force fresh validation, clearing and updating cache
        #[arg(
            long,
            help = "Force fresh validation (clears and updates cache for specified path)"
        )]
        force: bool,

        /// Number of parallel jobs (default: number of CPUs)
        #[arg(short, long)]
        jobs: Option<usize>,

        /// Suppress success output (errors still print)
        #[arg(long, help = "Quiet mode (only emit errors, rely on exit codes)")]
        quiet: bool,

        /// Stop after this many errors (across all files)
        #[arg(long)]
        max_errors: Option<usize>,

        /// Run roundtrip test (serialize → re-parse → compare) after validation.
        /// Tests serialization idempotency. Developer tool for parser/serializer testing.
        #[arg(long, help = "Test serialization idempotency (developer tool)")]
        roundtrip: bool,

        /// Parser backend for CHAT parsing.
        /// tree-sitter (default) supports incremental reparsing.
        /// re2c is a DFA-based parser that is faster for batch validation.
        #[arg(long, value_enum, default_value_t)]
        parser: ParserBackend,

        /// Audit mode: stream errors to JSONL file without caching (for bulk corpus validation).
        /// Reads from cache to skip clean files (fast), but doesn't write new errors to cache (avoids OOM).
        /// Generates summary statistics at the end.
        #[arg(
            long,
            help = "Stream errors to JSONL file (bulk audit mode)",
            value_name = "OUTPUT_FILE"
        )]
        audit: Option<PathBuf>,

        /// Enable strict cross-utterance linker validation (E351-E355).
        ///
        /// Checks that self-completion (+,) and other-completion (++)
        /// linkers are paired with the correct preceding terminators
        /// (+/. and +... respectively). Disabled by default because
        /// many existing corpora do not follow these strict conventions.
        #[arg(
            long = "strict-linkers",
            help = "Enable strict linker pairing validation (E351-E355)"
        )]
        strict_linkers: bool,

        /// Suppress error codes or named groups. Suppressed errors are not
        /// reported and do not cause a non-zero exit code.
        ///
        /// Named groups:
        ///   "xphon" — E725/E726/E727/E728: %xphosyl/%xphoaln/%xmodsyl
        ///             cross-tier alignment. **Suppressed by default
        ///             since 2026-04-21**; pass --check-xphon to
        ///             include these checks.
        ///
        /// Can mix groups and codes: --suppress xphon,E316
        #[arg(
            long,
            value_delimiter = ',',
            help = "Suppress error codes or groups (e.g., --suppress E726,E727)"
        )]
        suppress: Vec<String>,

        /// Opt in to `%xphon*` cross-tier alignment checks
        /// (E725/E726/E727/E728). These are skipped by default because
        /// PhonTalk-produced CHAT routinely trips them on
        /// data-quality issues outside our control; requiring
        /// --suppress xphon on every invocation had become noise.
        /// Pass this flag to re-enable them for a specific run.
        #[arg(
            long = "check-xphon",
            help = "Run %xphon* cross-tier alignment checks (skipped by default)"
        )]
        check_xphon: bool,
    },

    /// Normalize CHAT file to canonical format
    Normalize {
        /// Input CHAT file path
        input: PathBuf,

        /// Output CHAT file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Validate (includes alignment) before normalization
        #[arg(long, help = "Validate and check alignment before writing output")]
        validate: bool,

        /// Skip alignment when validating
        #[arg(
            long = "skip-alignment",
            help = "Skip alignment checks when --validate is supplied"
        )]
        skip_alignment: bool,
    },

    /// Convert CHAT file to JSON
    #[command(long_about = "Convert CHAT transcript(s) to JSON.\n\n\
        Output conforms to the TalkBank CHAT JSON Schema:\n\
        https://talkbank.org/schemas/v0.1/chat-file.json\n\n\
        Single file: prints JSON to stdout or writes to --output.\n\
        Directory: requires --output-dir. Walks recursively, preserving structure.\n\
        Incremental by default: skips files whose JSON is already up-to-date (mtime check).\n\
        Use --force to rebuild all. Use --prune to remove orphaned .json files.")]
    ToJson {
        /// Input CHAT file or directory path
        input: PathBuf,

        /// Output JSON file path (single-file mode; prints to stdout if omitted)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory (directory mode; preserves relative structure)
        #[arg(long)]
        output_dir: Option<PathBuf>,

        /// Compact (minified) JSON output instead of pretty-printed
        #[arg(long)]
        compact: bool,

        /// Force full rebuild (ignore mtime, reconvert all files)
        #[arg(long)]
        force: bool,

        /// Remove .json files with no matching .cha source (directory mode)
        #[arg(long)]
        prune: bool,

        /// Number of parallel workers for directory mode
        #[arg(short, long)]
        jobs: Option<usize>,

        /// [Deprecated] Validation is now on by default. This flag is ignored.
        #[arg(long, hide = true)]
        validate: bool,

        /// [Deprecated] Alignment is now on by default. Use --skip-alignment to disable.
        #[arg(short, long, hide = true)]
        alignment: bool,

        /// Skip tier alignment checks
        #[arg(
            long = "skip-alignment",
            help = "Disable tier alignment validation during conversion"
        )]
        skip_alignment: bool,

        /// Skip data model validation (parse only, always produce JSON)
        #[arg(
            long = "skip-validation",
            help = "Skip validation of the CHAT data model (parse only, no alignment)"
        )]
        skip_validation: bool,

        /// Skip validation against the CHAT JSON Schema
        #[arg(
            long,
            help = "Skip validation against the CHAT JSON Schema \
            (https://talkbank.org/schemas/v0.1/chat-file.json). \
            Useful for faster output when you trust the data model."
        )]
        skip_schema_validation: bool,
    },

    /// Convert JSON file to CHAT
    #[command(long_about = "Convert a JSON file back to CHAT format.\n\n\
        The input should conform to the TalkBank CHAT JSON Schema:\n\
        https://talkbank.org/schemas/v0.1/chat-file.json\n\n\
        Use `chatter schema` to print the full schema.")]
    FromJson {
        /// Input JSON file path
        input: PathBuf,

        /// Output CHAT file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Export CHAT file to TalkBank XML
    #[command(long_about = "Export one CHAT transcript to TalkBank XML.\n\n\
        This uses the Rust XML writer in talkbank-transform and validates the \
        input transcript before emitting XML.\n\n\
        XML ingest is not implemented, so there is no `from-xml` command.")]
    ToXml {
        /// Input CHAT file path
        input: PathBuf,

        /// Output XML file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Disable tier alignment validation during export
        #[arg(long = "skip-alignment")]
        skip_alignment: bool,
    },

    /// Verify that an utseg run preserved the gated invariants
    /// (file count, main-tier bullets, `@Media` linkage). %wor drops
    /// on split utterances are reported informationally.
    ///
    /// Compares two corpus directories — the pre-utseg input and the
    /// post-utseg output. Exit 0 on pass, 1 on regression.
    ValidateUtseg {
        /// Pre-utseg corpus directory (input).
        input: PathBuf,
        /// Post-utseg corpus directory (output).
        output: PathBuf,
        /// Suppress the per-file report; emit only the verdict line.
        #[arg(long)]
        quiet: bool,
    },

    /// Show alignment visualization for debugging
    ShowAlignment {
        /// Input CHAT file path
        input: PathBuf,

        /// Show alignment for specific tier types (mor, gra, pho, sin)
        /// If not specified, shows all available alignments
        #[arg(short, long, value_enum)]
        tier: Option<AlignmentTier>,

        /// Compact output (one line per alignment)
        #[arg(short, long)]
        compact: bool,
    },

    /// Watch CHAT file(s) for changes and continuously validate
    Watch {
        /// Path to CHAT file or directory to watch
        path: PathBuf,

        /// Skip tier alignment checks
        #[arg(long)]
        skip_alignment: bool,

        /// Clear terminal before each validation run
        #[arg(short, long)]
        clear: bool,
    },

    /// Lint CHAT file(s) and optionally auto-fix issues
    Lint {
        /// Path to CHAT file or directory
        path: PathBuf,

        /// Automatically apply fixes
        #[arg(long)]
        fix: bool,

        /// Show what would be fixed without modifying files
        #[arg(long, requires = "fix")]
        dry_run: bool,

        /// Skip tier alignment checks
        #[arg(long)]
        skip_alignment: bool,
    },

    /// Show cleaned text for each word in utterances (debugging aid)
    Clean {
        /// Input CHAT file path
        path: PathBuf,

        /// Only show words where raw text differs from cleaned text
        #[arg(long)]
        diff_only: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Create a new minimal valid CHAT file
    NewFile {
        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Speaker code (default: CHI)
        #[arg(short, long, default_value = "CHI")]
        speaker: String,

        /// ISO 639-3 language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Participant role (default: Target_Child)
        #[arg(short, long, default_value = "Target_Child")]
        role: String,

        /// Corpus identifier (default: corpus)
        #[arg(short, long, default_value = "corpus")]
        corpus: String,

        /// Initial utterance content (optional)
        #[arg(short, long)]
        utterance: Option<String>,
    },

    /// Cache management operations
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },

    /// Print the CHAT JSON Schema
    #[command(long_about = "Print the CHAT JSON Schema to stdout.\n\n\
        The schema describes the structure of CHAT transcripts serialized to JSON \
        by `chatter to-json`. It is auto-generated from the Rust data model \
        and conforms to JSON Schema 2020-12.\n\n\
        Canonical URL: https://talkbank.org/schemas/v0.1/chat-file.json")]
    Schema {
        /// Print only the canonical schema URL instead of the full schema
        #[arg(
            long,
            help = "Print only the canonical URL (https://talkbank.org/schemas/v0.1/chat-file.json)"
        )]
        url: bool,
    },

    /// CLAN analysis and transform commands
    #[command(
        about = "CLAN analysis and transform commands for CHAT transcripts",
        long_about = "Run CLAN analysis commands (freq, mlu, mlt, etc.) and transform commands \
            (flo, lowcase, etc.) on CHAT files.\n\n\
            Analysis commands compute statistics and produce text/JSON/CSV output.\n\
            Transform commands modify CHAT files in place or to a new file."
    )]
    Clan {
        #[command(subcommand)]
        command: ClanCommands,
    },

    /// Developer/debugging tools for CHAT analysis
    #[command(about = "Developer tools for inspecting and debugging CHAT files")]
    Debug {
        #[command(subcommand)]
        command: DebugCommands,
    },
}

/// Debug subcommands under `chatter debug`.
#[derive(Subcommand)]
pub enum DebugCommands {
    /// Rewrite whole-utterance runs of `@s` into utterance precodes (`[- LANG]`).
    ///
    /// Walks `.cha` files under the given paths, detects the same E255
    /// whole-utterance language-switch pattern that `chatter validate` rejects,
    /// and rewrites qualifying utterances in place. Files that need no changes
    /// are left untouched.
    FixS {
        /// Path to CHAT file(s) or directory trees to rewrite in place.
        path: Vec<PathBuf>,
    },

    /// Analyze CA overlap markers (⌈⌉⌊⌋): pairing, temporal consistency, orphans
    OverlapAudit {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Output format
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Write JSON lines database to this file (one JSON object per file).
        /// Enables persistent overlap data for downstream analysis.
        #[arg(long, value_name = "PATH")]
        database: Option<PathBuf>,
    },

    /// Audit linker and special terminator usage across a corpus
    ///
    /// Analyzes cross-utterance pairing correctness for linkers (+<, ++, +^,
    /// +", +,, +≋, +≈) and special terminators (+..., +/., +//., +"/.etc.).
    /// Reports frequency tables, pairing violations, orphaned terminators,
    /// and +< overlap block patterns.
    LinkerAudit {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Write per-anomaly JSON lines to this file. Each line is a JSON
        /// object with file, line, anomaly type, context, and suggested fix.
        #[arg(long, value_name = "PATH")]
        anomalies: Option<PathBuf>,
    },

    /// Filter CHAT files by @Languages / body content across a corpus tree.
    ///
    /// Internal corpus-inspection tool. Walks the given paths, parses the
    /// @Languages header of each .cha via the tree-sitter header fragment
    /// parser, and counts occurrences of an optional body substring. Emits
    /// the filtered list as paths / JSON Lines / CSV.
    ///
    /// Example: pick files with ≥20 @s tokens in bilingual transcripts,
    /// three per language pair, sorted by density:
    ///
    ///   chatter debug find ~/0tb/data --min-languages 2 --has-token @s \
    ///       --min-token-count 20 --max-per-pair 3 \
    ///       --sort token-count-desc --format jsonl
    Find(crate::commands::find::FindArgs),

    /// Sanitize a CHAT file for protected-corpus debugging — strip
    /// contributor lexical content while preserving structure.
    ///
    /// Replaces all word content, free-text dependent tiers, and
    /// free-text headers with placeholders / `[redacted]`, while
    /// preserving timing bullets, %wor offsets, speaker codes, POS
    /// tags, language markers, and structural counts. Intended for use
    /// before loading a protected `.cha` into LLM-assisted debugging
    /// tools. See `talkbank/docs/protected-corpus-debugging-workflow.md`.
    Sanitize {
        /// Path to a single .cha file to sanitize.
        input: PathBuf,

        /// Output path. If omitted, writes to stdout.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

/// Cache maintenance subcommands under `talkbank cache`.
#[derive(Subcommand)]
pub enum CacheCommands {
    /// Display cache statistics
    Stats {
        /// Output JSON format
        #[arg(long)]
        json: bool,
    },

    /// Clear cache entries
    Clear {
        /// Clear all cache entries
        #[arg(long, conflicts_with = "prefix")]
        all: bool,

        /// Clear entries matching this path prefix
        #[arg(long, conflicts_with = "all")]
        prefix: Option<PathBuf>,

        /// Show what would be cleared without actually clearing
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::{CommandFactory, Parser};

    use super::{Cli, Commands, DebugCommands};

    fn run_with_large_stack(test: impl FnOnce() + Send + 'static) {
        let join_result = std::thread::Builder::new()
            .name("core-args-test".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(test)
            .expect("spawn core args test thread")
            .join();
        match join_result {
            Ok(()) => {}
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[test]
    fn to_xml_parses_output_flag() {
        run_with_large_stack(|| {
            let parsed =
                Cli::parse_from(["chatter", "to-xml", "sample.cha", "--output", "sample.xml"]);

            let Commands::ToXml {
                input,
                output,
                skip_alignment,
            } = parsed.command
            else {
                panic!("expected to-xml command");
            };

            assert_eq!(input, PathBuf::from("sample.cha"));
            assert_eq!(output, Some(PathBuf::from("sample.xml")));
            assert!(!skip_alignment);
        });
    }

    #[test]
    fn help_shows_to_xml_and_not_from_xml() {
        run_with_large_stack(|| {
            let mut command = Cli::command();
            let mut help = Vec::new();
            command.write_long_help(&mut help).expect("render help");
            let rendered = String::from_utf8(help).expect("utf8 help");

            assert!(rendered.contains("to-xml"));
            assert!(!rendered.contains("from-xml"));
        });
    }

    #[test]
    fn debug_fix_s_parses_path_arguments() {
        run_with_large_stack(|| {
            let parsed = Cli::parse_from(["chatter", "debug", "fix-s", "sample.cha"]);

            let Commands::Debug {
                command: DebugCommands::FixS { path },
            } = parsed.command
            else {
                panic!("expected debug fix-s command");
            };

            assert_eq!(path, vec![PathBuf::from("sample.cha")]);
        });
    }
}
