//! `chatter debug find` — filter CHAT files by header and body content.
//!
//! Purpose: produce a curated list of CHAT files from a corpus tree
//! matching configurable header and body predicates. Used internally by
//! the TalkBank team for corpus auditing and evaluation-set curation
//! (notably the L2 morphotag breadth-first evaluation).
//!
//! Architecture:
//!
//! - [`scanner`] owns the single-file scan — header fragment parsing
//!   plus body substring counting — and the per-file [`ChatFileScan`]
//!   record.
//! - [`filter`] owns the predicates (language set, token threshold).
//! - [`output`] owns serialization to paths / JSONL / CSV.
//! - This module ([`mod.rs`]) wires the Clap arguments, reuses
//!   [`DiscoveredChatFiles`](clan_core::framework::input::DiscoveredChatFiles)
//!   for directory walking, and orchestrates the scan → filter → cap →
//!   sort → write pipeline.

pub mod filter;
pub mod output;
pub mod scanner;

#[cfg(test)]
mod integration_tests;

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::Args;
use clan_core::framework::DiscoveredChatFiles;
use talkbank_model::LanguageCodes;

use filter::{FindFilter, LanguageCodeSet, LanguageSetMatch};
use output::{FindOutputFormat, FindSortOrder, write_results};
use scanner::{ChatFileScan, ChatHeaderScanner, TokenPattern};

/// Clap argument struct for `chatter debug find`.
#[derive(Args, Debug, Clone)]
pub struct FindArgs {
    /// Paths to CHAT file(s) or directory(ies) to search.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Exact `@Languages` match (order-insensitive), e.g. `spa,eng`.
    #[arg(long, value_delimiter = ',')]
    pub languages: Option<Vec<String>>,

    /// Require at least one of these language codes in `@Languages`.
    /// Repeatable: `--language eng --language spa`.
    #[arg(long = "language", action = clap::ArgAction::Append)]
    pub language: Vec<String>,

    /// Require `@Languages` to have at least this many codes.
    /// Useful as `--min-languages 2` to select bilingual files.
    #[arg(long)]
    pub min_languages: Option<u32>,

    /// Count occurrences of this substring in the body; combine with
    /// `--min-token-count` to filter. Example: `--has-token @s`.
    #[arg(long)]
    pub has_token: Option<String>,

    /// Require at least N occurrences of `--has-token`. Default `1`
    /// when `--has-token` is supplied; ignored otherwise.
    #[arg(long, default_value_t = 1)]
    pub min_token_count: u32,

    /// Cap the output at N files per unique `@Languages` set
    /// (order-insensitive). Used to balance language-pair diversity
    /// when scanning large corpora with skewed distributions.
    #[arg(long)]
    pub max_per_pair: Option<u32>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = FindOutputFormat::Paths)]
    pub format: FindOutputFormat,

    /// Sort order for reported files.
    #[arg(long, value_enum, default_value_t = FindSortOrder::Path)]
    pub sort: FindSortOrder,
}

/// Entry point invoked from the top-level dispatcher.
///
/// Streams progress to stderr; writes results to stdout in the format
/// selected by `--format`. Exits the process with a non-zero code on I/O
/// errors from the writer (downstream tools like `head` may close the
/// pipe early, which is handled gracefully).
pub fn run_find(args: FindArgs) {
    if let Err(e) = run_find_impl(args, &mut io::stdout().lock()) {
        eprintln!("chatter debug find: {}", e);
        std::process::exit(1);
    }
}

/// Library-level implementation that writes to an injected `Write`
/// target. Used by both the CLI entry point and the integration tests.
fn run_find_impl<W: Write>(args: FindArgs, writer: &mut W) -> Result<(), FindError> {
    // Discover every CHAT file under the supplied paths. We reuse the
    // existing walker so directory semantics match `chatter validate`.
    let discovered = DiscoveredChatFiles::from_paths(&args.paths);
    for skipped in discovered.skipped_paths() {
        let skipped: &std::path::Path = skipped.as_ref();
        eprintln!(
            "chatter debug find: skipped unresolved path: {}",
            skipped.display()
        );
    }

    // Build the filter and the optional body token pattern.
    let pattern = match &args.has_token {
        Some(raw) => Some(TokenPattern::new(raw.clone())?),
        None => None,
    };
    let filter = build_filter(&args)?;

    // Construct the scanner once — it owns a tree-sitter parser that is
    // reused across every file in the walk. `TreeSitterParser` is
    // `!Send`, so we scan sequentially (parallelization would require
    // one scanner per worker thread and currently isn't warranted).
    let mut scanner = ChatHeaderScanner::new()?;
    if let Some(pattern) = pattern {
        scanner = scanner.with_pattern(pattern);
    }

    // Scan → filter. I/O errors on individual files are reported to
    // stderr but do not abort the whole run; a single unreadable file
    // shouldn't invalidate a corpus-wide scan.
    let mut accepted: Vec<ChatFileScan> = Vec::new();
    for path in discovered.files() {
        match scanner.scan(path) {
            Ok(scan) => {
                if filter.accepts(&scan) {
                    accepted.push(scan);
                }
            }
            Err(e) => {
                eprintln!("chatter debug find: scan error: {}", e);
            }
        }
    }

    // Sort first so that a subsequent per-pair cap picks the top-N per
    // pair in the user's requested order (e.g. `token-count-desc + cap
    // 3` yields the three densest files per pair).
    sort_in_place(&mut accepted, args.sort);
    if let Some(cap) = args.max_per_pair {
        apply_per_pair_cap(&mut accepted, cap);
    }

    write_results(writer, args.format, &accepted)?;
    Ok(())
}

/// Converts raw CLI strings into the typed [`FindFilter`].
///
/// Kept as a free function so the integration tests can exercise the
/// conversion directly without spinning up a full argument parser.
fn build_filter(args: &FindArgs) -> Result<FindFilter, FindError> {
    let mut languages = LanguageSetMatch::default();

    if let Some(codes) = &args.languages {
        let set: LanguageCodeSet = codes
            .iter()
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .collect();
        if set.is_empty() {
            return Err(FindError::EmptyLanguagesList);
        }
        languages.exact = Some(set);
    }

    if !args.language.is_empty() {
        languages.contains_all = args.language.iter().map(|s| s.trim().to_string()).collect();
    }

    languages.min_count = args.min_languages;

    Ok(FindFilter {
        languages,
        min_token_count: if args.has_token.is_some() {
            args.min_token_count
        } else {
            0
        },
    })
}

fn sort_in_place(scans: &mut [ChatFileScan], order: FindSortOrder) {
    match order {
        FindSortOrder::Path => scans.sort_by(|a, b| a.path.cmp(&b.path)),
        FindSortOrder::TokenCountDesc => {
            // Ties broken by path so the ordering is deterministic.
            scans.sort_by(|a, b| {
                b.token_count
                    .get()
                    .cmp(&a.token_count.get())
                    .then_with(|| a.path.cmp(&b.path))
            });
        }
    }
}

/// Caps the number of entries retained per unique `@Languages` set.
///
/// The language set is normalized by sorting and deduplicating codes so
/// that `spa,eng` and `eng,spa` bucket together. Input order is
/// preserved within each bucket (caller responsible for pre-sorting).
fn apply_per_pair_cap(scans: &mut Vec<ChatFileScan>, cap: u32) {
    if cap == 0 {
        scans.clear();
        return;
    }
    let mut counts: BTreeMap<Vec<String>, u32> = BTreeMap::new();
    scans.retain(|scan| {
        let key = pair_key(&scan.languages);
        let entry = counts.entry(key).or_insert(0);
        if *entry < cap {
            *entry += 1;
            true
        } else {
            false
        }
    });
}

fn pair_key(codes: &LanguageCodes) -> Vec<String> {
    let set: LanguageCodeSet = codes.iter().map(|c| c.as_str().to_string()).collect();
    set.into_iter().collect()
}

/// Errors that can surface from `chatter debug find`.
#[derive(Debug, thiserror::Error)]
pub enum FindError {
    /// The tree-sitter parser failed to initialize.
    #[error("parser init: {0}")]
    ParserInit(#[from] talkbank_parser::ParserInitError),

    /// `--has-token` was supplied with an empty value.
    #[error("token pattern must be non-empty")]
    EmptyTokenPattern,

    /// `--languages` was supplied with an empty list (all entries blank).
    #[error("--languages requires at least one code")]
    EmptyLanguagesList,

    /// Writing output failed.
    #[error("output I/O: {0}")]
    Io(#[from] io::Error),
}

impl From<scanner::EmptyTokenPattern> for FindError {
    fn from(_: scanner::EmptyTokenPattern) -> Self {
        Self::EmptyTokenPattern
    }
}

impl From<scanner::ScanError> for FindError {
    fn from(value: scanner::ScanError) -> Self {
        match value {
            scanner::ScanError::Io { source, .. } => FindError::Io(source),
        }
    }
}

#[cfg(test)]
mod mod_tests {
    use super::*;

    fn args_with_languages(codes: &[&str]) -> FindArgs {
        FindArgs {
            paths: vec![],
            languages: Some(codes.iter().map(|s| (*s).to_string()).collect()),
            language: vec![],
            min_languages: None,
            has_token: None,
            min_token_count: 1,
            max_per_pair: None,
            format: FindOutputFormat::Paths,
            sort: FindSortOrder::Path,
        }
    }

    #[test]
    fn empty_languages_flag_is_rejected() {
        let args = args_with_languages(&[""]);
        let err = build_filter(&args).unwrap_err();
        assert!(matches!(err, FindError::EmptyLanguagesList));
    }

    #[test]
    fn languages_csv_trims_whitespace() {
        let args = args_with_languages(&["spa", " eng "]);
        let filter = build_filter(&args).expect("ok");
        let set = filter.languages.exact.expect("exact set present");
        let pair: Vec<_> = set.iter().map(|c| c.as_str().to_string()).collect();
        assert_eq!(pair, vec!["eng", "spa"]);
    }

    #[test]
    fn min_token_count_only_applies_with_has_token() {
        let mut args = args_with_languages(&["eng"]);
        args.languages = None;
        args.min_token_count = 50;
        args.has_token = None;
        let filter = build_filter(&args).expect("ok");
        assert_eq!(filter.min_token_count, 0);

        args.has_token = Some("@s".to_string());
        let filter = build_filter(&args).expect("ok");
        assert_eq!(filter.min_token_count, 50);
    }
}
