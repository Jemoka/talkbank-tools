//! Standard streamed validation runtime with text, JSON, and TUI frontends.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::commands::validate::cache::initialize_validation_cache;
use crate::commands::validate_parallel::renderer::create_renderer;
use crate::commands::validate_parallel::shared::empty_stats;
use crate::commands::validate_parallel::{
    ValidateDirectoryOptions, ValidationPresentation, ValidationTraversalMode,
};
use crate::ui::{TuiAction, run_validation_tui_streaming};
use talkbank_transform::validation_runner::{
    CacheMode, DirectoryMode, ValidationConfig, ValidationEvent, ValidationStatsSnapshot,
    validate_files_streaming,
};

/// Run the standard validation flow on a pre-collected list of CHAT
/// files. Both `chatter validate dir/` and
/// `chatter validate a.cha b.cha c.cha` resolve to this single path
/// after the CLI has walked input args into a flat file list — the
/// fix for the prior divergence between per-file and per-directory
/// code paths.
///
/// `summary_label` is cosmetic for the summary line; the actual
/// validation operates entirely on `files`.
pub fn run_validation_runtime(
    files: Vec<PathBuf>,
    summary_label: PathBuf,
    options: ValidateDirectoryOptions,
) -> ValidationStatsSnapshot {
    let ValidateDirectoryOptions {
        rules,
        traversal,
        execution,
        presentation,
        suppress,
    } = options;
    // Routing invariant: `cli/dispatch.rs` extracts the audit
    // presentation variant before forwarding to the runtime
    // entrypoint, so reaching this `else` branch is impossible by
    // construction.
    #[allow(clippy::unreachable)]
    let ValidationPresentation::Streaming(output) = presentation else {
        unreachable!("audit presentation is handled before the runtime entrypoint");
    };

    let suppress_set: std::collections::HashSet<String> = suppress.into_iter().collect();

    let config = ValidationConfig {
        check_alignment: rules.alignment.enabled(),
        jobs: execution.jobs,
        cache: CacheMode::Enabled,
        directory: match traversal {
            ValidationTraversalMode::Recursive => DirectoryMode::Recursive,
            ValidationTraversalMode::SingleFile => DirectoryMode::SingleFile,
        },
        roundtrip: rules.roundtrip.enabled(),
        parser_kind: rules.parser_kind,
        strict_linkers: rules.strict_linkers,
    };

    let cache = initialize_validation_cache(&summary_label, execution.cache_refresh);

    if output.interface.uses_tui() {
        return run_tui_loop(
            files,
            &summary_label,
            &config,
            cache,
            output.theme,
            &suppress_set,
        );
    }

    let (events_rx, cancel_tx) = validate_files_streaming(files, &config, cache.clone());
    let (filtered_rx, files_fully_suppressed) = filter_suppressed_events(events_rx, &suppress_set);
    install_ctrlc_handler(&cancel_tx);

    let json_mode = matches!(output.format, crate::cli::OutputFormat::Json);
    let mut renderer = create_renderer(json_mode, output.quiet);
    let mut final_stats = None;
    let mut error_count = 0usize;
    let mut files_completed = 0usize;

    for event in filtered_rx {
        match event {
            ValidationEvent::Discovering => renderer.handle_discovering(),
            ValidationEvent::Started { total_files } => renderer.handle_started(total_files),
            ValidationEvent::Errors(error_event) => {
                error_count = error_count.saturating_add(renderer.handle_errors(&error_event));
                cancel_if_error_limit_reached(&cancel_tx, execution.max_errors, error_count);
            }
            ValidationEvent::RoundtripComplete(rt_event) => {
                error_count =
                    error_count.saturating_add(renderer.handle_roundtrip_complete(&rt_event));
                cancel_if_error_limit_reached(&cancel_tx, execution.max_errors, error_count);
            }
            ValidationEvent::FileComplete(file_event) => {
                files_completed += 1;
                renderer.handle_file_complete(&file_event, files_completed);
            }
            ValidationEvent::Finished(snapshot) => {
                final_stats = Some(snapshot);
            }
        }
    }

    let stats = match final_stats {
        Some(stats) => stats,
        None => {
            eprintln!("Error: No validation stats received");
            std::process::exit(1);
        }
    };

    // Adjust stats before rendering: files whose errors were entirely
    // suppressed should not count as invalid in the summary or exit code.
    let mut stats = stats;
    let suppressed = files_fully_suppressed.load(Ordering::Relaxed);
    if suppressed > 0 {
        stats.invalid_files = stats.invalid_files.saturating_sub(suppressed);
    }

    renderer.handle_finished(&stats, files_completed, execution.max_errors, error_count);
    renderer.print_summary(&summary_label, &stats, rules.roundtrip.enabled());

    if suppressed > 0 {
        eprintln!(
            "Suppressed: {suppressed} file(s) had only suppressed errors (not counted as invalid)"
        );
    }

    stats
}

/// Drive the interactive TUI, supporting reruns until the user exits.
///
/// Re-streaming on Rerun re-uses the same file list so the rerun
/// honors the same input the user originally asked for.
fn run_tui_loop(
    files: Vec<PathBuf>,
    summary_label: &Path,
    config: &ValidationConfig,
    cache: Option<Arc<talkbank_transform::CachePool>>,
    theme: crate::ui::Theme,
    suppress_set: &std::collections::HashSet<String>,
) -> ValidationStatsSnapshot {
    let _ = summary_label; // reserved for future "rerunning <label>..." messaging
    loop {
        let (events_rx, cancel_tx) = validate_files_streaming(files.clone(), config, cache.clone());
        let (filtered_rx, _suppressed) = filter_suppressed_events(events_rx, suppress_set);
        match run_validation_tui_streaming(filtered_rx, cancel_tx, theme.clone()) {
            Ok(TuiAction::Quit) => return empty_stats(false),
            Ok(TuiAction::ForceQuit) => std::process::exit(130),
            Ok(TuiAction::Rerun) => {
                eprintln!("Re-running validation...");
            }
            Err(error) => {
                eprintln!("TUI error: {}", error);
                return empty_stats(true);
            }
        }
    }
}

/// Filter suppressed error codes from the validation event stream.
///
/// Returns a new receiver that has suppressed errors removed, plus an atomic
/// counter of files whose errors were entirely suppressed (for stats adjustment).
/// If `suppress_set` is empty, returns the original receiver unchanged.
fn filter_suppressed_events(
    events_rx: crossbeam_channel::Receiver<ValidationEvent>,
    suppress_set: &std::collections::HashSet<String>,
) -> (
    crossbeam_channel::Receiver<ValidationEvent>,
    Arc<AtomicUsize>,
) {
    let suppressed_count = Arc::new(AtomicUsize::new(0));
    if suppress_set.is_empty() {
        return (events_rx, suppressed_count);
    }

    let (filtered_tx, filtered_rx) = crossbeam_channel::unbounded();
    let suppress_set = suppress_set.clone();
    let count = Arc::clone(&suppressed_count);
    std::thread::spawn(move || {
        // Track files whose errors were entirely suppressed, so we can
        // rewrite their FileComplete status from Invalid → Valid.
        let mut fully_suppressed_files: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        for event in events_rx {
            match event {
                ValidationEvent::Errors(mut error_event) => {
                    let pre_count = error_event.errors.len();
                    error_event
                        .errors
                        .retain(|e| !suppress_set.contains(e.code.as_str()));
                    if error_event.errors.is_empty() && pre_count > 0 {
                        count.fetch_add(1, Ordering::Relaxed);
                        fully_suppressed_files.insert(error_event.path.clone());
                    }
                    if !error_event.errors.is_empty() {
                        let _ = filtered_tx.send(ValidationEvent::Errors(error_event));
                    }
                }
                ValidationEvent::FileComplete(mut file_event) => {
                    // If this file's errors were entirely suppressed, report it as Valid
                    if fully_suppressed_files.remove(&file_event.path) {
                        file_event.status =
                            talkbank_transform::validation_runner::FileStatus::Valid {
                                cache_hit: false,
                            };
                    }
                    let _ = filtered_tx.send(ValidationEvent::FileComplete(file_event));
                }
                ValidationEvent::Finished(mut snapshot) => {
                    // Adjust the final stats to reflect suppression
                    let suppressed = count.load(Ordering::Relaxed);
                    snapshot.invalid_files = snapshot.invalid_files.saturating_sub(suppressed);
                    let _ = filtered_tx.send(ValidationEvent::Finished(snapshot));
                }
                other => {
                    let _ = filtered_tx.send(other);
                }
            }
        }
    });
    (filtered_rx, suppressed_count)
}

/// Install the Ctrl+C handler used by non-interactive validation modes.
fn install_ctrlc_handler(cancel_tx: &crossbeam_channel::Sender<()>) {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_count = Arc::new(AtomicUsize::new(0));
    let cancelled_clone = Arc::clone(&cancelled);
    let cancel_count_clone = Arc::clone(&cancel_count);
    let cancel_tx_clone = cancel_tx.clone();

    if let Err(error) = ctrlc::set_handler(move || {
        let count = cancel_count_clone.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            let was_cancelled = cancelled_clone.swap(true, Ordering::SeqCst);
            cancel_tx_clone.send(()).ok();
            if !was_cancelled {
                eprintln!("\nCancelling validation... (press Ctrl+C again to force quit)");
            }
        } else {
            eprintln!("\nForce quitting.");
            std::process::exit(130);
        }
    }) {
        eprintln!("Error setting Ctrl+C handler: {}", error);
    }
}

/// Cancel the run when the configured error limit has been reached.
fn cancel_if_error_limit_reached(
    cancel_tx: &crossbeam_channel::Sender<()>,
    max_errors: Option<usize>,
    error_count: usize,
) {
    if let Some(limit) = max_errors
        && error_count >= limit
    {
        cancel_tx.send(()).ok();
    }
}
