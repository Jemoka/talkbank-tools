//! CLAN subcommand definitions for the `chatter` CLI.
//!
//! Analysis commands deliberately keep their flag surface thin and typed so the
//! CLI can normalize into library-owned analysis models before delegating
//! defaults and validation back to `clan-core`.

use clap::{ArgGroup, Command, Subcommand};
use std::path::PathBuf;
use clan_core::commands::chains::ChainsConfig;
use clan_core::commands::codes::CodesConfig;
use clan_core::commands::corelex::CorelexConfig;
use clan_core::commands::dss::DssConfig;
use clan_core::commands::ipsyn::IpsynConfig;
use clan_core::commands::keymap::KeymapConfig;
use clan_core::commands::maxwd::MaxwdConfig;
use clan_core::commands::rely::RelyConfig;
use clan_core::commands::trnfix::TrnfixConfig;

use super::clan_common::CommonAnalysisArgs;

/// Flat enum of all CLAN analysis and transform commands.
#[derive(Subcommand)]
pub enum ClanCommands {
    // -- Analysis commands --
    /// Word/morpheme frequency counts with type-token ratio
    Freq {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Count morphemes from %mor tier instead of words from main tier
        #[arg(long)]
        mor: bool,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Mean length of utterance (morphemes or words)
    Mlu {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Count words from main tier instead of morphemes from %mor
        #[arg(long)]
        words: bool,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Mean length of turn (utterances and words per turn)
    Mlt {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Word length distribution
    Wdlen {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Word size (character length) histogram from %mor stems
    Wdsize {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Use main tier words instead of %mor stems
        #[arg(long)]
        main_tier: bool,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Longest words per speaker
    Maxwd {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Maximum number of words to display
        #[arg(short = 'n', long, default_value_t = MaxwdConfig::default().limit)]
        limit: clan_core::framework::WordLimit,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Word frequency grouped by part of speech from %mor tier
    Freqpos {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Time duration statistics from bullet timing marks
    Timedur {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Keyword-in-context search (matching utterances)
    Kwal {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Keywords to search for (case-insensitive substring match)
        #[arg(short, long, required = true)]
        keyword: Vec<String>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// List gem segments (@Bg/@Eg bracketed regions)
    Gemlist {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Boolean keyword search (AND/OR combinations)
    Combo {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Search expression(s): use + for AND, comma for OR (can be repeated)
        #[arg(short = 'S', long = "search", required = true)]
        search: Vec<String>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Word co-occurrence counting (pairs of words in same utterance)
    Cooccur {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Word distribution analysis (dispersion across utterances)
    Dist {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Child/parent interaction profile (imitation, overlap analysis)
    Chip {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Phonological frequency from %pho tier (phone character counts)
    Phonfreq {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Model/replica comparison from %mod and %pho tiers
    Modrep {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Vocabulary diversity (D statistic) via bootstrap sampling
    Vocd {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Report repeated utterances with frequency counts
    Uniq {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Sort output by descending frequency (CLAN -o flag)
        #[arg(long)]
        sort: bool,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Frequency table of codes from %cod tier
    Codes {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Maximum depth of code parsing (0 = all levels)
        #[arg(long, default_value_t = CodesConfig::default().max_depth)]
        max_depth: clan_core::framework::CodeDepth,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Compare two tiers word-by-word and report mismatches
    Trnfix {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// First tier to compare (default: mor)
        #[arg(long, default_value_t = TrnfixConfig::default().tier1)]
        tier1: clan_core::framework::TierKind,

        /// Second tier to compare (default: trn)
        #[arg(long, default_value_t = TrnfixConfig::default().tier2)]
        tier2: clan_core::framework::TierKind,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Morphosyntactic structure scoring (MLU-S, TNW, WPS, CPS)
    Sugar {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Cross-tabulation of morphological categories from %mor tier
    Mortable {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Path to language script file (.cut)
        #[arg(short = 'f', long)]
        script: PathBuf,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Clause chain analysis via code markers (consecutive code occurrences)
    Chains {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Tier label to read codes from (default: cod)
        #[arg(long, default_value_t = ChainsConfig::default().tier)]
        tier: clan_core::framework::TierKind,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Syntactic complexity ratio from %gra dependency tier
    Complexity {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Core vocabulary analysis (words above frequency threshold)
    Corelex {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Minimum frequency threshold for core words (defaults to the library corelex threshold)
        #[arg(long, default_value_t = CorelexConfig::default().min_frequency)]
        threshold: clan_core::framework::FrequencyThreshold,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Contingency tables for coded data (keyword-following-code frequencies)
    Keymap {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Keywords to track (can be repeated)
        #[arg(short, long, required = true)]
        keyword: Vec<String>,

        /// Tier label to read codes from (default: cod)
        #[arg(long, default_value_t = KeymapConfig::default().tier)]
        tier: clan_core::framework::TierKind,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Compare utterances to a template script (accuracy metrics)
    Script {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Path to template/script CHAT file
        #[arg(short, long)]
        template: PathBuf,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Inter-rater agreement (Cohen's kappa) between two coded files
    Rely {
        /// First coded CHAT file
        file1: PathBuf,

        /// Second coded CHAT file
        file2: PathBuf,

        /// Tier label to compare (default: cod)
        #[arg(long, default_value_t = RelyConfig::default().tier)]
        tier: clan_core::framework::TierKind,

        /// Output format: text (default), json, or csv
        #[arg(short, long, value_enum, default_value_t = super::clan_common::ClanOutputFormat::Text)]
        format: super::clan_common::ClanOutputFormat,
    },

    /// Fluency calculation (disfluency metrics: SLD, TD)
    Flucalc {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Developmental Sentence Scoring
    Dss {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Path to DSS rules file (.scr)
        #[arg(long)]
        rules: Option<PathBuf>,

        /// Maximum utterances to score (default: 50)
        #[arg(long, default_value_t = DssConfig::default().max_utterances)]
        max_utterances: clan_core::framework::UtteranceLimit,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Index of Productive Syntax
    Ipsyn {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Path to IPSYN rules file
        #[arg(long)]
        rules: Option<PathBuf>,

        /// Maximum utterances to analyze (default: 100)
        #[arg(long, default_value_t = IpsynConfig::default().max_utterances)]
        max_utterances: clan_core::framework::UtteranceLimit,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Language sample evaluation (morphosyntactic analysis)
    Eval {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Combined child language evaluation (DSS + VOCD + IPSYN + EVAL)
    Kideval {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Path to DSS rules file
        #[arg(long)]
        dss_rules: Option<PathBuf>,

        /// Path to IPSYN rules file
        #[arg(long)]
        ipsyn_rules: Option<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    // -- Transform commands --
    /// Simplified fluent output (adds %flo tier, strips headers)
    Flo {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Lowercase all words on main tiers
    Lowcase {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// String replacement using a changes file
    Chstring {
        /// Path to input CHAT file
        path: PathBuf,

        /// Path to changes file (alternating find/replace lines)
        #[arg(short, long)]
        changes: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compute ages from @Birth and @Date headers
    Dates {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add missing terminators (default: period)
    Delim {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Fix timing bullet consistency
    Fixbullets {
        /// Path to input CHAT file
        path: PathBuf,

        /// Global millisecond offset to apply to parsed bullet timings
        #[arg(long)]
        offset: Option<i64>,

        /// Include only selected tier kinds (for example `cod`, `%com`, `*`)
        #[arg(long = "tier")]
        tier: Vec<String>,

        /// Exclude selected tier kinds (for example `mor`, `%cod`, `*`)
        #[arg(long = "exclude-tier")]
        exclude_tier: Vec<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add %ret tier copying main tier content verbatim
    Retrace {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Mark utterances with revisions using [+ rep] postcodes
    Repeat {
        /// Path to input CHAT file
        path: PathBuf,

        /// Target speaker code (required)
        #[arg(short, long)]
        speaker: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Combine multiple dependent tiers of the same type into one
    Combtier {
        /// Path to input CHAT file
        path: PathBuf,

        /// Tier label to combine (e.g., "com" for %com)
        #[arg(short, long)]
        tier: String,

        /// Separator between combined contents
        #[arg(long, default_value = " ")]
        separator: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Normalize compound word formatting (dashes to plus notation)
    Compound {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Reorder dependent tiers to canonical order
    Tierorder {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add or remove line numbers on tier lines
    Lines {
        /// Path to input CHAT file
        path: PathBuf,

        /// Remove existing line numbers instead of adding them
        #[arg(short, long)]
        remove: bool,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Fix common CHAT formatting errors (bracket spacing, ellipsis, etc.)
    Dataclean {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract quoted text to separate utterances
    Quotes {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Apply orthographic conversion using a dictionary file
    Ort {
        /// Path to input CHAT file
        path: PathBuf,

        /// Path to orthographic conversion dictionary
        #[arg(short, long)]
        dictionary: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Apply pattern-matching rules to %mor tier post-processing
    Postmortem {
        /// Path to input CHAT file
        path: PathBuf,

        /// Path to rules file (from_pattern => to_replacement)
        #[arg(short, long)]
        rules: PathBuf,

        /// Target tier label (default: mor)
        #[arg(long, default_value = "mor")]
        target_tier: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate %mod tier from pronunciation lexicon
    Makemod {
        /// Path to input CHAT file
        path: PathBuf,

        /// Path to CMU-format pronunciation lexicon file
        #[arg(short, long)]
        lexicon: PathBuf,

        /// Show all alternative pronunciations
        #[arg(long)]
        all_alternatives: bool,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Remove selected dependent tiers while preserving the rest of the file
    Trim {
        /// Path to input CHAT file
        path: PathBuf,

        /// Keep only the selected dependent tier label(s), e.g. "mor" or "*"
        #[arg(long = "tier")]
        tier: Vec<String>,

        /// Remove the selected dependent tier label(s), e.g. "mor" or "*"
        #[arg(long = "exclude-tier")]
        exclude_tier: Vec<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Rename speaker codes throughout a CHAT file
    Roles {
        /// Path to input CHAT file
        path: PathBuf,

        /// Rename mapping as OLD=NEW (can be repeated)
        #[arg(short, long, required = true)]
        rename: Vec<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    // -- Converter commands --
    /// Convert CHAT file to plain text
    Chat2text {
        /// Path to CHAT file
        path: PathBuf,

        /// Include speaker codes in output
        #[arg(long)]
        include_speaker: bool,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert SRT subtitles to CHAT format
    Srt2chat {
        /// Path to SRT file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert CHAT file to SRT subtitle format
    Chat2srt {
        /// Path to CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert CHAT file to WebVTT subtitle format
    Chat2vtt {
        /// Path to CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert plain text to CHAT format
    Text2chat {
        /// Path to text file
        path: PathBuf,

        /// Speaker code (default: SPK)
        #[arg(short, long, default_value = "SPK")]
        speaker: String,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert LIPP phonetic profile to CHAT format
    Lipp2chat {
        /// Path to LIPP file
        path: PathBuf,

        /// Speaker code (default: CHI)
        #[arg(short, long, default_value = "CHI")]
        speaker: String,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert ELAN EAF file to CHAT format
    Elan2chat {
        /// Path to ELAN EAF file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert Praat TextGrid to CHAT format
    Praat2chat {
        /// Path to Praat TextGrid file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert CHAT file to Praat TextGrid format
    Chat2praat {
        /// Path to CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert LENA ITS device output to CHAT format
    Lena2chat {
        /// Path to LENA ITS file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert PLAY annotation to CHAT format
    Play2chat {
        /// Path to PLAY file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert LAB timing labels to CHAT format
    Lab2chat {
        /// Path to LAB file
        path: PathBuf,

        /// Speaker code (default: SPK)
        #[arg(short, long, default_value = "SPK")]
        speaker: String,

        /// Language code (default: eng)
        #[arg(short = 'L', long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert RTF file to CHAT format
    Rtf2chat {
        /// Path to RTF file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert SALT transcription to CHAT format
    Salt2chat {
        /// Path to SALT file
        path: PathBuf,

        /// Language code (default: eng)
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract gem segments (@Bg/@Eg bounded regions)
    Gem {
        /// Path to input CHAT file
        path: PathBuf,

        /// Gem labels to extract (if empty, extract all)
        #[arg(short, long)]
        gem: Vec<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert CHAT file to ELAN EAF format
    Chat2elan {
        /// Path to CHAT file
        path: PathBuf,

        /// Media file extension (e.g., wav, mp4)
        #[arg(short, long)]
        media_extension: Option<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Language sample evaluation with dialect support
    #[command(
        name = "eval-d",
        about = "Language sample evaluation with dialect support (EVAL variant)"
    )]
    EvalD {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },

    /// Morphological analysis — deliberately not implemented
    Mor {},

    /// POS disambiguation — deliberately not implemented
    Post {},

    /// Grammar relation parsing — deliberately not implemented
    Megrasp {},

    /// List POST database contents — deliberately not implemented
    Postlist {},

    /// Modify POST database rules — deliberately not implemented
    Postmodrules {},

    /// Train POST model — deliberately not implemented
    Posttrain {},

    // -- Compatibility aliases (CLAN command names) --
    /// Validate CHAT file(s) with CLAN CHECK-compatible output and flags
    #[command(about = "Validate CHAT file(s) (CLAN 'check' command)")]
    Check {
        /// Path to CHAT file(s) or directory (required unless --list-errors)
        paths: Vec<PathBuf>,

        /// Check bullet consistency (0=full, 1=missing only)
        #[arg(long)]
        bullets: Option<u8>,

        /// Only report this error number (can repeat)
        #[arg(long = "error", short = 'e')]
        include_errors: Vec<u16>,

        /// Exclude this error number (can repeat)
        #[arg(long = "exclude-error")]
        exclude_errors: Vec<u16>,

        /// List all error numbers and their messages
        #[arg(long)]
        list_errors: bool,

        /// Check for "CHI Target_Child" in @Participants (+g2)
        #[arg(long)]
        check_target: bool,

        /// Check for missing @ID tiers (+g4, on by default)
        #[arg(long)]
        check_id: Option<bool>,

        /// Check for unused speakers (+g5)
        #[arg(long)]
        check_unused: bool,

        /// Validate UD features on %mor tier (+u)
        #[arg(long)]
        check_ud: bool,
    },

    /// Normalize CHAT file — CLAN compatibility alias for `chatter normalize` (tier reordering + line wrapping)
    #[command(
        about = "Normalize CHAT file (CLAN 'fixit' equivalent — same as `chatter normalize`)"
    )]
    Fixit {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Align CA overlap markers (⌈/⌊) by column position
    #[command(about = "Align CA overlap markers by column position")]
    Indent {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Normalize CHAT file — CLAN compatibility alias for `chatter normalize` (join continuation lines)
    #[command(
        about = "Normalize CHAT file (CLAN 'longtier' equivalent — same as `chatter normalize`)"
    )]
    Longtier {
        /// Path to input CHAT file
        path: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Word frequency within gem segments — CLAN compatibility alias for `freq --gem`
    #[command(
        about = "Word frequency within gem segments (CLAN 'gemfreq' equivalent — same as `freq --gem`)",
        group(ArgGroup::new("gemfreq-required-gem").args(["gem"]).required(true))
    )]
    Gemfreq {
        /// Path to CHAT file(s) or directory
        path: Vec<PathBuf>,

        /// Count morphemes from %mor tier instead of words from main tier
        #[arg(long)]
        mor: bool,

        #[command(flatten)]
        common: CommonAnalysisArgs,
    },
}

/// Command category for grouping in help output.
///
/// Each CLAN subcommand belongs to exactly one category. The mapping is
/// maintained here alongside the enum definition so they stay in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanCommandCategory {
    /// Compute statistics, counts, and metrics from CHAT data.
    Analysis,
    /// Modify CHAT files: add/remove tiers, fix formatting, etc.
    Transform,
    /// Convert between CHAT and other formats (SRT, EAF, Praat, etc.).
    Converter,
    /// Alternate names for existing commands (CLAN compatibility).
    CompatibilityAlias,
    /// Commands that require CLAN directly (morphological analysis, etc.).
    NotAvailable,
}

impl ClanCommandCategory {
    /// Heading text displayed in grouped help output.
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Analysis => "Analysis Commands",
            Self::Transform => "Transform Commands",
            Self::Converter => "Format Converters",
            Self::CompatibilityAlias => "Compatibility Aliases",
            Self::NotAvailable => "Not Available (use CLAN directly)",
        }
    }
}

/// Map from subcommand name to its category.
///
/// Returns `None` for the built-in `help` subcommand that clap adds.
pub fn command_category(name: &str) -> Option<ClanCommandCategory> {
    use ClanCommandCategory::*;
    Some(match name {
        // -- Analysis commands --
        "freq" | "mlu" | "mlt" | "wdlen" | "wdsize" | "maxwd" | "freqpos" | "timedur" | "kwal"
        | "gemlist" | "combo" | "cooccur" | "dist" | "chip" | "phonfreq" | "modrep" | "vocd"
        | "uniq" | "codes" | "trnfix" | "sugar" | "mortable" | "chains" | "complexity"
        | "corelex" | "keymap" | "script" | "rely" | "flucalc" | "dss" | "ipsyn" | "eval"
        | "kideval" | "eval-d" => Analysis,
        // -- Transform commands --
        "flo" | "lowcase" | "chstring" | "dates" | "delim" | "fixbullets" | "retrace"
        | "repeat" | "combtier" | "compound" | "tierorder" | "lines" | "dataclean" | "quotes"
        | "ort" | "postmortem" | "makemod" | "trim" | "roles" => Transform,
        // -- Converter commands --
        "chat2text" | "srt2chat" | "chat2srt" | "chat2vtt" | "text2chat" | "lipp2chat"
        | "elan2chat" | "praat2chat" | "chat2praat" | "lena2chat" | "play2chat" | "lab2chat"
        | "rtf2chat" | "salt2chat" | "gem" | "chat2elan" => Converter,
        // -- Compatibility aliases --
        "check" | "fixit" | "indent" | "longtier" | "gemfreq" => CompatibilityAlias,
        // -- Not available --
        "mor" | "post" | "megrasp" | "postlist" | "postmodrules" | "posttrain" => NotAvailable,
        // clap's built-in help subcommand
        "help" => return None,
        _ => return None,
    })
}

/// All categories in display order.
const CATEGORY_ORDER: &[ClanCommandCategory] = &[
    ClanCommandCategory::Analysis,
    ClanCommandCategory::Transform,
    ClanCommandCategory::Converter,
    ClanCommandCategory::CompatibilityAlias,
    ClanCommandCategory::NotAvailable,
];

/// Apply category grouping to the `clan` subcommand's help output.
///
/// Clap 4 does not support grouping subcommands under different headings via
/// derive attributes (`help_heading` on subcommand variants controls argument
/// headings, not subcommand listing headings). This function works around that
/// limitation by replacing the `clan` subcommand's `override_help` with a
/// custom-rendered grouped listing.
///
/// Call this on the root `Command` returned by `Cli::command()` before parsing.
pub fn apply_clan_help_grouping(root: Command) -> Command {
    root.mut_subcommand("clan", |clan_cmd| {
        // Build the grouped help text from the actual subcommands registered
        // by clap derive, so names and descriptions stay in sync automatically.
        let grouped_help = build_grouped_help(&clan_cmd);
        clan_cmd.override_help(grouped_help)
    })
}

/// Build a help string with subcommands organized under category headings.
fn build_grouped_help(cmd: &Command) -> String {
    use std::fmt::Write;

    let mut out = String::new();

    // Preamble: about text
    if let Some(long_about) = cmd.get_long_about() {
        let _ = writeln!(out, "{long_about}");
    } else if let Some(about) = cmd.get_about() {
        let _ = writeln!(out, "{about}");
    }

    // Usage line (bin_name is not set until build(), so construct manually)
    let _ = writeln!(out, "\nUsage: chatter clan [OPTIONS] <COMMAND>");

    // Collect subcommands into a map by name for lookup
    let subcmds: std::collections::BTreeMap<&str, &Command> = cmd
        .get_subcommands()
        .map(|sc| (sc.get_name(), sc))
        .collect();

    // Compute the longest subcommand name for alignment
    let longest = subcmds.keys().map(|name| name.len()).max().unwrap_or(0);

    // Render each category
    for &category in CATEGORY_ORDER {
        let heading = category.heading();
        let commands_in_category: Vec<&&Command> = subcmds
            .values()
            .filter(|sc| command_category(sc.get_name()) == Some(category))
            .collect();

        if commands_in_category.is_empty() {
            continue;
        }

        let _ = writeln!(out, "\n{heading}:");
        for sc in commands_in_category {
            let name = sc.get_name();
            let about = sc.get_about().map(|a| a.to_string()).unwrap_or_default();
            let _ = writeln!(out, "  {name:<longest$}  {about}");
        }
    }

    // The built-in help subcommand
    let _ = writeln!(
        out,
        "\nOptions:\n  -h, --help  Print help (see more with '--help')"
    );

    out
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn run_with_large_stack(test: impl FnOnce() + Send + 'static) {
        let join_result = std::thread::Builder::new()
            .name("clan-args-test".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(test)
            .expect("spawn clan args test thread")
            .join();
        match join_result {
            Ok(()) => {}
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: ClanCommands,
    }

    #[test]
    fn corelex_default_matches_library_config() {
        run_with_large_stack(|| {
            let parsed = TestCli::parse_from(["test-cli", "corelex", "sample.cha"]);

            let ClanCommands::Corelex { threshold, .. } = parsed.command else {
                panic!("expected corelex command");
            };

            assert_eq!(threshold, CorelexConfig::default().min_frequency);
        });
    }

    #[test]
    fn rely_default_tier_matches_library_config() {
        run_with_large_stack(|| {
            let parsed = TestCli::parse_from(["test-cli", "rely", "left.cha", "right.cha"]);

            let ClanCommands::Rely { tier, .. } = parsed.command else {
                panic!("expected rely command");
            };

            assert_eq!(tier, RelyConfig::default().tier);
        });
    }

    #[test]
    fn gemfreq_uses_common_gem_filter() {
        run_with_large_stack(|| {
            let parsed =
                TestCli::parse_from(["test-cli", "gemfreq", "--gem", "episode", "sample.cha"]);

            let ClanCommands::Gemfreq { common, .. } = parsed.command else {
                panic!("expected gemfreq command");
            };

            assert_eq!(common.gem, vec!["episode"]);
        });
    }

    #[test]
    fn gemfreq_requires_gem_filter() {
        run_with_large_stack(|| {
            let error = match TestCli::try_parse_from(["test-cli", "gemfreq", "sample.cha"]) {
                Ok(_) => panic!("gemfreq should require --gem"),
                Err(error) => error,
            };
            let rendered = error.to_string();

            assert!(
                rendered.contains("--gem"),
                "expected missing --gem error, got `{rendered}`"
            );
        });
    }

    #[test]
    fn check_list_errors_allows_omitting_path() {
        run_with_large_stack(|| {
            let parsed = TestCli::parse_from(["test-cli", "check", "--list-errors"]);

            let ClanCommands::Check {
                paths, list_errors, ..
            } = parsed.command
            else {
                panic!("expected check command");
            };

            assert!(list_errors);
            assert!(paths.is_empty());
        });
    }
}
