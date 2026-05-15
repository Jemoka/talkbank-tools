//! Shared clap argument groups for CLAN analysis commands.
//!
//! This module keeps the CLI-facing flag surface small and typed. In
//! particular, `--range` now parses directly into the library-owned
//! [`clan_core::framework::UtteranceRange`] model so the CLI stops carrying
//! raw `start-end` strings past argument parsing.

use clap::{Args, ValueEnum};
use clan_core::framework::{IdFilter, UtteranceRange, parse_id_filter, parse_utterance_range};

/// Shared filtering and output arguments for CLAN analysis commands.
#[derive(Args, Debug, Clone)]
pub struct CommonAnalysisArgs {
    /// Filter by speaker code(s) — only process these speakers (can be repeated)
    #[arg(short, long)]
    pub speaker: Vec<String>,

    /// Exclude speaker code(s) — skip these speakers (can be repeated)
    #[arg(short = 'X', long)]
    pub exclude_speaker: Vec<String>,

    /// Only process utterances within gem segments matching these labels (can be repeated)
    #[arg(short, long)]
    pub gem: Vec<String>,

    /// Skip utterances within gem segments matching these labels (can be repeated)
    #[arg(long)]
    pub exclude_gem: Vec<String>,

    /// Only process utterances containing these words — case-insensitive substring (can be repeated)
    #[arg(short = 'w', long)]
    pub include_word: Vec<String>,

    /// Skip utterances containing these words — case-insensitive substring (can be repeated)
    #[arg(short = 'W', long)]
    pub exclude_word: Vec<String>,

    /// Restrict to a 1-based utterance range within each file (e.g., "25-125")
    #[arg(long, value_parser = parse_utterance_range)]
    pub range: Option<UtteranceRange>,

    /// Filter by `@ID` header pattern, pipe-separated in @ID column order
    /// (`lang|corpus|speaker|age|sex|group|ses|role|education|custom`).
    ///
    /// Each field is `*` / empty (wildcard) or a literal match.
    /// Trailing wildcards may be omitted: `eng|*|CHI` ≡ `eng|*|CHI|`
    /// ≡ `eng|*|CHI|*`. A file is included only if at least one `@ID`
    /// matches; within matching files, utterances from non-matching
    /// speakers are dropped. Replaces legacy CLAN `+t@ID="…"`.
    #[arg(long, value_parser = parse_id_filter)]
    pub id_filter: Option<IdFilter>,

    /// Output results per file instead of aggregated across all files
    #[arg(long)]
    pub per_file: bool,

    /// Include retraced words in counting (CLAN +r6 equivalent)
    #[arg(long)]
    pub include_retracings: bool,

    /// Output format: text (default), json, or csv
    #[arg(short, long, value_enum, default_value_t = ClanOutputFormat::Text)]
    pub format: ClanOutputFormat,
}

/// Output format for CLAN analysis commands.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ClanOutputFormat {
    /// Human-readable text
    Text,
    /// Structured JSON
    Json,
    /// CSV for spreadsheets
    Csv,
    /// CLAN-compatible output (character-for-character match with legacy CLAN)
    Clan,
}
