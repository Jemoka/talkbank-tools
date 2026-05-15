//! Single-file validation flow with error-sink wiring.
//!
//! Implements `chatter validate <file>` by composing the cache, parser, validator,
//! and output layers. Three output modes are supported: streaming terminal (text),
//! structured JSON, and interactive TUI with rerun capability.
//!
//! # Error-sink architecture
//!
//! In text mode, a [`TeeErrorSink`] mirrors each error to both a [`TerminalErrorSink`]
//! (for immediate display) and an [`ErrorCollector`] (for caching and exit-code logic).
//! JSON and TUI modes collect into an [`ErrorCollector`] alone and format after the parse
//! completes.

use std::fs;
use std::path::PathBuf;

use talkbank_model::{ErrorCollector, ErrorSink, ParseValidateOptions, TeeErrorSink};
use talkbank_transform::parse_and_validate_streaming;

use crate::cli::OutputFormat;
use crate::commands::{AlignmentValidationMode, CacheRefreshMode, ValidationInterface};
use crate::output::TerminalErrorSink;
use crate::ui::Theme;

use super::cache::{get_cached_validation, initialize_validation_cache, set_cached_validation};
use super::output::output_validation_result;

/// Outcome of validating one file via [`validate_file`].
///
/// Per the project no-boolean-blindness rule, this is an enum rather
/// than `bool` — `validate_file(...) == Valid` reads correctly
/// without the caller having to remember which polarity `true` means.
///
/// Used today only by `chatter watch`, which discards the outcome
/// (the per-file watch UI handles its own state). Multi-file CLI
/// invocations route through `validate_paths_parallel` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileValidationOutcome {
    /// File parsed and validated cleanly (or was a cache hit).
    Valid,
    /// File produced one or more validation errors. Errors were
    /// already streamed to the terminal or printed in JSON form
    /// inline; nothing further is deferred to the caller.
    Invalid,
}

/// Validate a single CHAT file with optional alignment and caching behavior.
///
/// This routine encapsulates the CLI behavior for the `validate` subcommand when the target
/// path is a single file. It manages the shared `UnifiedCache`, optionally purges entries when
/// `--force` is provided, reads the CHAT content, and builds `ParseValidateOptions` that align
/// with the Main Tier and Dependent Tier rules described in the CHAT manual. Errors are streamed
/// through the appropriate sinks (JSON, TUI, or terminal).
///
/// Returns [`FileValidationOutcome::Valid`] / [`FileValidationOutcome::Invalid`] so callers
/// processing multiple files can keep going instead of having this function `process::exit`
/// after the first invalid file. Inability-to-validate conditions (unreadable file, internal
/// parser error, TUI subsystem failure) still call `process::exit(1)` since the per-file run
/// genuinely cannot continue; the TUI `ForceQuit` action still calls `process::exit(130)`
/// because the user explicitly asked the whole command to stop.
pub fn validate_file(
    path: &PathBuf,
    format: OutputFormat,
    alignment: AlignmentValidationMode,
    cache_refresh: CacheRefreshMode,
    quiet: bool,
    interface: ValidationInterface,
    _theme: Theme,
    suppress: &[String],
    strict_linkers: bool,
) -> FileValidationOutcome {
    // `theme` is no longer used inside `validate_file` because the TUI
    // launch moved to the multi-file driver in `run_validate_command`
    // (so all files end up in one consolidated TUI). Kept in the
    // signature so the public API and the watch-mode caller don't break.
    let check_alignment = alignment.enabled();

    let cache = initialize_validation_cache(path, cache_refresh);

    // Try to get cached results.
    // On Some(true): cached valid — skip revalidation.
    // On Some(false) or None: revalidate.
    if get_cached_validation(cache.as_ref(), path, check_alignment) == Some(true) {
        // Cached success: output and return without revalidating.
        // (TUI mode included: a cached-valid file has no errors to
        // contribute to the consolidated TUI.)
        output_validation_result(path, &[], None, format, true, quiet);
        return FileValidationOutcome::Valid;
    }

    // Not in cache or cache disabled - validate file
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file {:?}: {}", path, e);
            std::process::exit(1);
        }
    };

    // Build pipeline options (alignment adds `%wor`/`%pho` checks while validation-only skips them).
    // Alignment is on by default; use --skip-alignment to disable
    let mut options = if check_alignment {
        ParseValidateOptions::default().with_alignment()
    } else {
        ParseValidateOptions::default().with_validation()
    };
    if strict_linkers {
        options = options.with_strict_linkers();
    }

    // Build suppression set for error filtering
    let suppress_set: std::collections::HashSet<String> =
        suppress.iter().map(|s| s.to_uppercase()).collect();

    // Use different error sinks based on output format and TUI mode
    // JSON/TUI needs structured values, interactive CLI streams to terminal plus collecting.
    let mut errors = if matches!(format, OutputFormat::Json) || interface.uses_tui() {
        // JSON mode or TUI mode: collect errors for structured output or TUI display
        let error_sink = ErrorCollector::new();

        match parse_and_validate_streaming(&content, options.clone(), &error_sink) {
            Ok(_) => error_sink.into_vec(),
            Err(e) => {
                if matches!(format, OutputFormat::Json) {
                    let json_output = serde_json::json!({
                        "file": path.to_string_lossy(),
                        "status": "error",
                        "error": format!("{}", e)
                    });
                    match serde_json::to_string_pretty(&json_output) {
                        Ok(serialized) => println!("{}", serialized),
                        Err(err) => {
                            eprintln!("Error serializing JSON output: {}", err);
                        }
                    }
                } else {
                    eprintln!("Error: {}", e);
                }
                std::process::exit(1);
            }
        }
    } else if !suppress_set.is_empty() {
        // Text mode with suppression: collect first, then filter, then display
        // (cannot stream-and-suppress because TerminalErrorSink prints immediately)
        let error_sink = ErrorCollector::new();

        match parse_and_validate_streaming(&content, options.clone(), &error_sink) {
            Ok(_) => error_sink.into_vec(),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Interactive mode (not TUI): stream errors immediately to terminal AND collect for caching/output
        let terminal_sink = TerminalErrorSink::new(path, &content);
        let collecting_sink = ErrorCollector::new();
        let tee_sink = TeeErrorSink::new(&terminal_sink, &collecting_sink);

        match parse_and_validate_streaming(&content, options.clone(), &tee_sink) {
            Ok(_) => collecting_sink.into_vec(),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Track whether the sink above streamed errors to stderr as it
    // went. Only the no-suppress / text-mode branch does so; the
    // JSON, TUI, and text+suppress branches all collected silently
    // and therefore owe the user a printed rendering below. Before
    // `resolve_suppress` defaulted `xphon` on, the suppress list was
    // typically empty in interactive runs, so the streamed branch
    // covered almost every case and this distinction wasn't visible
    // in test fixtures. It became visible once xphon suppression
    // fired by default (2026-04-21).
    let errors_streamed_already =
        matches!(format, OutputFormat::Text) && !interface.uses_tui() && suppress_set.is_empty();

    // Enhance errors with source context for proper miette display (TUI, JSON output, etc.)
    talkbank_model::enhance_errors_with_source(&mut errors, &content);

    // Filter out suppressed error codes
    if !suppress_set.is_empty() {
        errors.retain(|e| !suppress_set.contains(e.code.as_str()));
    }

    // Cache the results (pass/fail only)
    set_cached_validation(cache.as_ref(), path, check_alignment, errors.is_empty());

    // TUI mode in this single-file path is unused by the modern CLI
    // (multi-file invocations route through `validate_paths_parallel`,
    // which uses the streaming TUI). `chatter watch` is the only
    // remaining caller and it always passes `ValidationInterface::Plain`.
    // Keep a defensive branch that prints a plain-text summary so
    // any future TUI=true single-file caller still produces useful
    // output without launching a TUI from this code path.
    if interface.uses_tui() {
        if errors.is_empty() {
            println!("✓ No errors found in {}", path.display());
            return FileValidationOutcome::Valid;
        }
        eprintln!(
            "✗ Errors found in {} ({} error(s))",
            path.display(),
            errors.len()
        );
        return FileValidationOutcome::Invalid;
    }

    // Non-TUI output paths.
    let source_for_print = if matches!(format, OutputFormat::Text) {
        Some(content.as_str())
    } else {
        None
    };

    // Text mode normally streams errors live via `TerminalErrorSink`;
    // when that happened, re-printing through
    // `output_validation_result` would duplicate every diagnostic.
    // The suppressed branch collected silently, however, so in that
    // case we still owe the user a printed rendering.
    if matches!(format, OutputFormat::Text) && !errors.is_empty() {
        if !errors_streamed_already {
            let terminal_sink = TerminalErrorSink::new(path, &content);
            for error in &errors {
                terminal_sink.report(error.clone());
            }
        }
        if !quiet && crate::output::should_show_cascading_hint(&errors) {
            eprintln!("{}", crate::output::CASCADING_HINT);
        }
        return FileValidationOutcome::Invalid;
    }

    output_validation_result(path, &errors, source_for_print, format, false, quiet);
    if errors.is_empty() {
        FileValidationOutcome::Valid
    } else {
        FileValidationOutcome::Invalid
    }
}
