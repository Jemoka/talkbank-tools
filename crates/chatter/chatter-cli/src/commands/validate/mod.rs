//! Validation commands for CHAT files.
//!
//! This module exposes the low-level `validate_file` entrypoint plus formatting helpers
//! and utilities (audit reporting, output formatting). It is the landing point for CLI `validate`
//! subcommands (single file, directory, TUI) and orchestrates caching, alignment toggles, and
//! structured outputs.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod audit_reporter;
pub(crate) mod cache;
mod file;
mod output;

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::ui::Theme;
use talkbank_transform::validation_runner::ParserKind;

use super::validate_parallel::{
    AlignmentValidationMode, CacheRefreshMode, RoundtripValidationMode, StreamingValidationOutput,
    ValidateDirectoryOptions, ValidationExecution, ValidationInterface, ValidationPresentation,
    ValidationRules, ValidationTraversalMode, validate_paths_parallel,
};

pub use file::{FileValidationOutcome, validate_file};

/// Typed options for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandRules {
    /// Alignment validation policy.
    pub alignment: AlignmentValidationMode,
    /// Roundtrip validation policy.
    pub roundtrip: RoundtripValidationMode,
    /// Parser backend selection.
    pub parser_kind: ParserKind,
    /// Enable strict cross-utterance linker validation (E351-E355).
    pub strict_linkers: bool,
}

/// Execution settings for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandExecution {
    /// Cache refresh policy for the target path.
    pub cache_refresh: CacheRefreshMode,
    /// Optional parallel worker count.
    pub jobs: Option<usize>,
    /// Optional global error cap for directory validation.
    pub max_errors: Option<usize>,
}

/// Output and interaction settings for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandPresentation {
    /// Output format for file or directory validation.
    pub format: OutputFormat,
    /// Whether to suppress success output.
    pub quiet: bool,
    /// Optional audit JSONL output path.
    pub audit_output: Option<PathBuf>,
    /// Interactive presentation surface to use.
    pub interface: ValidationInterface,
    /// Loaded theme for TUI validation.
    pub theme: Theme,
}

/// Typed options for the top-level `chatter validate` command.
#[derive(Clone, Debug)]
pub struct ValidateCommandOptions {
    /// Validation rules and parser choices.
    pub rules: ValidateCommandRules,
    /// Cache, worker-count, and failure-limit settings.
    pub execution: ValidateCommandExecution,
    /// Output, audit, and TUI settings.
    pub presentation: ValidateCommandPresentation,
    /// Raw `--suppress` list as received from the CLI. Named groups
    /// (like `xphon`) are still unexpanded at this point; final
    /// resolution happens in `resolve_suppress`, which folds in the
    /// default `xphon` suppression when `check_xphon` is false.
    pub suppress: Vec<String>,
    /// Opt-in for `%xphon*` cross-tier alignment checks
    /// (E725/E726/E727/E728). When false (the default), those codes
    /// are added to the effective suppress list so the routine
    /// validation surface stays clean on PhonTalk-exported data. See
    /// `resolve_suppress` for the merge rules.
    pub check_xphon: bool,
}

/// Expand named suppress groups into concrete error codes.
///
/// Named groups provide user-friendly shorthand for sets of related error codes.
/// Unknown names are treated as literal error codes (e.g., "E726").
fn expand_suppress_groups(raw: Vec<String>) -> Vec<String> {
    let mut codes = Vec::new();
    for item in raw {
        match item.to_lowercase().as_str() {
            // %xmodsyl / %xphosyl / %xphoaln cross-tier alignment
            // (PhonTalk-generated). E725: %modsyl vs %mod, E726:
            // %phosyl vs %pho, E727: %phoaln vs %mod, E728: %phoaln
            // vs %pho. Suppressed by default — see
            // `resolve_suppress` below — because PhonTalk output
            // regularly ships with these divergences that we cannot
            // fix upstream.
            "xphon" => codes.extend(["E725", "E726", "E727", "E728"].map(String::from)),
            _ => codes.push(item.to_uppercase()),
        }
    }
    codes
}

/// Combine the user's raw `--suppress` list with the default policy for
/// the `xphon` validation group, then expand named groups into concrete
/// error codes.
///
/// Policy: the `xphon` group
/// (E725–E728) is suppressed by default. Phon's CHAT-export tooling
/// ships data that trips these cross-tier alignment checks; running
/// them unconditionally creates unhelpful noise for every routine
/// validation. Users who want those checks opt in with
/// `--check-xphon`, which omits the automatic suppression.
///
/// Explicit user entries still win: if the user passes
/// `--suppress E725` while also asking for `--check-xphon`, E725 is
/// still suppressed (the user's instruction is more specific than the
/// group opt-in). Likewise, a user who passes `--suppress xphon`
/// keeps that exact suppression regardless of `--check-xphon`, since
/// `apply_xphon_default` only adds the group when neither condition
/// otherwise implies it.
fn resolve_suppress(user_raw: Vec<String>, check_xphon: bool) -> Vec<String> {
    let mut raw = user_raw;
    if !check_xphon {
        // Only add the default group if the user hasn't already
        // mentioned it, so we don't bloat the list with redundant
        // entries (cosmetic, but keeps diagnostic output clean).
        let already_listed = raw.iter().any(|item| item.to_lowercase() == "xphon");
        if !already_listed {
            raw.push("xphon".to_string());
        }
    }
    expand_suppress_groups(raw)
}

#[cfg(test)]
mod tests {
    use super::{expand_suppress_groups, resolve_suppress};

    #[test]
    fn xphon_suppressed_by_default_when_check_xphon_false() {
        let effective = resolve_suppress(vec![], false);
        for code in ["E725", "E726", "E727", "E728"] {
            assert!(
                effective.contains(&code.to_string()),
                "default suppress should include {code}"
            );
        }
    }

    #[test]
    fn xphon_active_when_check_xphon_true() {
        let effective = resolve_suppress(vec![], true);
        for code in ["E725", "E726", "E727", "E728"] {
            assert!(
                !effective.contains(&code.to_string()),
                "opt-in mode must not suppress {code}"
            );
        }
    }

    #[test]
    fn explicit_user_suppress_still_applies_alongside_default() {
        let effective = resolve_suppress(vec!["E316".to_string()], false);
        assert!(effective.contains(&"E316".to_string()));
        assert!(effective.contains(&"E725".to_string()));
    }

    #[test]
    fn redundant_xphon_entry_not_doubled() {
        let effective = resolve_suppress(vec!["xphon".to_string()], false);
        // Expect each xphon code present exactly once.
        for code in ["E725", "E726", "E727", "E728"] {
            let count = effective.iter().filter(|c| *c == code).count();
            assert_eq!(count, 1, "code {code} should appear exactly once");
        }
    }

    #[test]
    fn explicit_single_xphon_code_still_suppressed_even_with_check_xphon() {
        // User asks to check xphon as a group, but also explicitly
        // silences E725 on top. The explicit entry wins.
        let effective = resolve_suppress(vec!["E725".to_string()], true);
        assert!(effective.contains(&"E725".to_string()));
        assert!(!effective.contains(&"E726".to_string()));
    }

    #[test]
    fn xphon_expands_to_all_phon_cross_tier_codes() {
        let result = expand_suppress_groups(vec!["xphon".to_string()]);
        // E725: %modsyl vs %mod
        // E726: %phosyl vs %pho
        // E727: %phoaln vs %mod
        // E728: %phoaln vs %pho
        assert!(
            result.contains(&"E725".to_string()),
            "missing E725 (modsyl/mod)"
        );
        assert!(
            result.contains(&"E726".to_string()),
            "missing E726 (phosyl/pho)"
        );
        assert!(
            result.contains(&"E727".to_string()),
            "missing E727 (phoaln/mod)"
        );
        assert!(
            result.contains(&"E728".to_string()),
            "missing E728 (phoaln/pho)"
        );
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn literal_codes_pass_through_uppercased() {
        let result = expand_suppress_groups(vec!["e316".to_string()]);
        assert_eq!(result, vec!["E316"]);
    }

    #[test]
    fn mixed_groups_and_codes() {
        let result = expand_suppress_groups(vec!["xphon".to_string(), "E316".to_string()]);
        assert_eq!(result.len(), 5);
        assert!(result.contains(&"E725".to_string()));
        assert!(result.contains(&"E316".to_string()));
    }
}

/// Execute one top-level `chatter validate` invocation.
///
/// Accepts one or more paths. Each path can be a file or directory.
/// Multiple files are validated individually. A single directory uses
/// the parallel directory validation pipeline.
pub fn run_validate_command(paths: Vec<PathBuf>, options: ValidateCommandOptions) {
    let ValidateCommandOptions {
        rules,
        execution,
        presentation,
        suppress: raw_suppress,
        check_xphon,
    } = options;
    let suppress = resolve_suppress(raw_suppress, check_xphon);
    let ValidateCommandRules {
        alignment,
        roundtrip,
        parser_kind,
        strict_linkers,
    } = rules;
    let ValidateCommandExecution {
        cache_refresh,
        jobs,
        max_errors,
    } = execution;
    let ValidateCommandPresentation {
        format,
        quiet,
        audit_output,
        interface,
        theme,
    } = presentation;

    // ARCHITECTURAL NOTE (2026-05-03): every CLI input — single file,
    // multiple files, single directory, multiple directories, or any
    // mix — funnels through ONE pipeline (`validate_paths_parallel` →
    // `validate_files_streaming`). This replaces a previous fork where
    // multi-file inputs went through a per-file `validate_file` loop
    // (no progress bar, noisy "✓ valid" lines for each file, separate
    // per-file TUI) while directory inputs went through the parallel
    // streaming pipeline. The fork was flagged as wrong UX: multi-file
    // input behaved differently from directory input despite both
    // resolving to the same logical set of files, leaving duplicate
    // code paths reinventing the streaming pipeline.
    //
    // The unified shape means CLI args drive ONLY which .cha files are
    // collected; everything downstream (renderer, progress, TUI,
    // suppression, summary, exit code) is identical regardless of
    // input shape.

    // Walk every input path into a flat .cha file list. Files contribute
    // themselves directly; directories contribute their recursive .cha
    // descendants.
    let mut files: Vec<PathBuf> = Vec::new();
    for p in &paths {
        if p.is_file() {
            files.push(p.clone());
        } else if p.is_dir() {
            collect_cha_files_recursive(p, &mut files);
        } else {
            eprintln!("Error: {:?} is not a file or directory", p);
            std::process::exit(1);
        }
    }

    if files.is_empty() {
        eprintln!("Error: no .cha files found in {:?}", paths);
        std::process::exit(1);
    }

    // Sort for deterministic processing order (matches directory walk
    // behavior, which sorts collected files before dispatch).
    files.sort();

    // Cosmetic summary label: use the first input path verbatim. For
    // a single-directory invocation this preserves the old behavior of
    // printing the directory name in the summary; for a multi-file or
    // mixed-input invocation it's just the first arg the user typed.
    let summary_label = paths.first().cloned().unwrap_or_else(|| PathBuf::from("."));

    let stats = validate_paths_parallel(
        files,
        summary_label,
        ValidateDirectoryOptions {
            rules: ValidationRules {
                alignment,
                roundtrip,
                parser_kind,
                strict_linkers,
            },
            traversal: ValidationTraversalMode::Recursive,
            execution: ValidationExecution {
                cache_refresh,
                jobs,
                max_errors,
            },
            presentation: match audit_output {
                Some(output_path) => ValidationPresentation::Audit { output_path },
                None => ValidationPresentation::Streaming(StreamingValidationOutput {
                    format,
                    quiet,
                    interface,
                    theme,
                }),
            },
            suppress,
        },
    );

    if stats.invalid_files > 0 || stats.parse_errors > 0 {
        std::process::exit(1);
    }
}

/// Walk `dir` and append every `.cha` file (recursively) to `files`.
/// Mirrors the directory-walk behavior of
/// `validation_runner::collect_cha_files` but lives on the CLI side
/// because the CLI is the layer that mixes file-and-directory args.
fn collect_cha_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: failed to read {:?}: {}", dir, e);
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cha_files_recursive(&path, files);
        } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            files.push(path);
        }
    }
}
