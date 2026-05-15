//! Orchestration for directory-scale streaming validation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::cache::ValidationCache;
use super::config::ValidationConfig;
use super::helpers::collect_cha_files;
use super::types::{ValidationEvent, ValidationStats};
use super::worker::worker_loop;
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use std::path::Path;
use std::sync::Arc;
use std::thread;

/// Run validation for all discovered files and stream progress/events.
///
/// Returns a tuple of:
/// - `Receiver<ValidationEvent>` - Events stream as validation progresses
/// - `Sender<()>` - Send to this channel to cancel validation
///
/// # Example
/// ```ignore
/// let (events, cancel) = validate_directory_streaming(&dir, &config, cache);
///
/// for event in events {
///     match event {
///         ValidationEvent::Errors(e) => print_errors(&e),
///         ValidationEvent::FileComplete(f) => update_progress(&f),
///         ValidationEvent::Finished(stats) => print_summary(&stats),
///         _ => {}
///     }
/// }
/// ```
pub fn validate_directory_streaming<C>(
    directory: &Path,
    config: &ValidationConfig,
    cache: Option<Arc<C>>,
) -> (Receiver<ValidationEvent>, Sender<()>)
where
    C: ValidationCache + Send + Sync + 'static,
{
    // Use unbounded channel for events to prevent backpressure
    // Errors are cheap to store, and we want workers to never block on sending events
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (cancel_tx, cancel_rx) = bounded::<()>(1);

    let dir = directory.to_path_buf();
    let cfg = config.clone();

    thread::spawn(move || {
        // Send discovering event immediately so UI shows something is happening
        let _ = event_tx.send(ValidationEvent::Discovering);
        run_validation(dir, cfg, cache, event_tx, cancel_rx);
    });

    (event_rx, cancel_tx)
}

/// Validate a pre-collected list of .cha files using the same streaming
/// pipeline as [`validate_directory_streaming`].
///
/// This exists so the `chatter validate` CLI can route a list of file
/// paths (e.g. `chatter validate a.cha b.cha c.cha`) through the same
/// renderer/progress/TUI surface as a directory walk, instead of
/// reinventing per-file output. The CLI is responsible for resolving
/// arguments to a flat file list (walking any directories first); this
/// entry point trusts the list verbatim — no filtering, no extension
/// check.
///
/// Cancellation, event semantics, and worker behavior are identical
/// to the directory variant.
pub fn validate_files_streaming<C>(
    files: Vec<std::path::PathBuf>,
    config: &ValidationConfig,
    cache: Option<Arc<C>>,
) -> (Receiver<ValidationEvent>, Sender<()>)
where
    C: ValidationCache + Send + Sync + 'static,
{
    let (event_tx, event_rx) = unbounded::<ValidationEvent>();
    let (cancel_tx, cancel_rx) = bounded::<()>(1);

    let cfg = config.clone();

    thread::spawn(move || {
        // Send discovering event immediately so the renderer transitions
        // out of "starting up" the same way it would for a directory walk.
        let _ = event_tx.send(ValidationEvent::Discovering);
        run_validation_on_files(files, cfg, cache, event_tx, cancel_rx);
    });

    (event_rx, cancel_tx)
}

/// Internal runner implementation used by the directory streaming
/// entrypoint. Discovers `.cha` files under `directory` and forwards
/// the collected list to [`run_validation_on_files`]. Kept thin so the
/// directory-walk and explicit-file-list paths share all worker /
/// event-stream / stats logic.
pub(super) fn run_validation<C>(
    directory: std::path::PathBuf,
    config: ValidationConfig,
    cache: Option<Arc<C>>,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) where
    C: ValidationCache + Send + Sync + 'static,
{
    let mut files = Vec::new();
    collect_cha_files(
        &directory,
        config.directory == super::config::DirectoryMode::Recursive,
        &mut files,
    );
    files.sort();
    run_validation_on_files(files, config, cache, event_tx, cancel_rx);
}

/// Worker-pool body shared by the directory and explicit-file-list
/// streaming entrypoints. Takes a pre-collected list of files,
/// dispatches to N worker threads, streams the standard
/// `ValidationEvent` sequence (Started → Errors / FileComplete /
/// RoundtripComplete → Finished) on `event_tx`, and respects
/// `cancel_rx`.
pub(super) fn run_validation_on_files<C>(
    files: Vec<std::path::PathBuf>,
    config: ValidationConfig,
    cache: Option<Arc<C>>,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
) where
    C: ValidationCache + Send + Sync + 'static,
{
    let total_files = files.len();

    // Send start event
    if event_tx
        .send(ValidationEvent::Started { total_files })
        .is_err()
    {
        return; // Receiver dropped
    }

    if total_files == 0 {
        let stats = ValidationStats::new(0);
        event_tx
            .send(ValidationEvent::Finished(stats.snapshot()))
            .ok();
        return;
    }

    // Set up work queue
    let (work_tx, work_rx) = bounded::<std::path::PathBuf>(total_files);
    let stats = Arc::new(ValidationStats::new(total_files));

    // Determine number of workers. Treat `jobs=0` as `1` to preserve progress.
    let num_workers = match config.jobs {
        Some(0) => {
            tracing::warn!("validation jobs=0 requested; using 1 worker instead");
            1
        }
        Some(n) => n,
        None => num_cpus::get(),
    };

    // Spawn worker threads
    let workers: Vec<_> = (0..num_workers)
        .map(|_| {
            let rx = work_rx.clone();
            let tx = event_tx.clone();
            let cancel = cancel_rx.clone();
            let cache_ref = cache.clone();
            let cfg = config.clone();
            let stats = stats.clone();

            thread::spawn(move || {
                worker_loop(rx, tx, cancel, cache_ref, cfg, stats);
            })
        })
        .collect();

    // Send all work to the queue
    for file in files {
        // Check for early cancellation
        match cancel_rx.try_recv() {
            Ok(()) => break,
            // Sender dropped is not an explicit cancellation request.
            Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => {}
        }

        if work_tx.send(file).is_err() {
            break; // Workers died
        }
    }
    drop(work_tx); // Signal no more work

    // Wait for all workers to complete
    let mut had_panic = false;
    for (worker_id, worker) in workers.into_iter().enumerate() {
        match worker.join() {
            Ok(()) => {
                // Worker completed successfully
            }
            Err(panic_payload) => {
                tracing::error!(
                    worker_id = worker_id,
                    "Worker panicked: {:?}",
                    panic_payload
                );
                had_panic = true;
            }
        }
    }

    if had_panic {
        tracing::error!("One or more validation workers panicked - results may be incomplete");
        // Note: Don't fail the entire validation, just log the issue
        // The stats will show what was actually processed
    }

    // Send final stats
    // Check if cancelled
    if cancel_rx.try_recv().is_ok() {
        stats.mark_cancelled();
    }

    let final_stats = stats.snapshot();

    // Log cache statistics for debugging
    let hit_rate = if final_stats.total_files > 0 {
        (final_stats.cache_hits as f64 / final_stats.total_files as f64) * 100.0
    } else {
        0.0
    };
    tracing::info!(
        cache_hits = final_stats.cache_hits,
        cache_misses = final_stats.cache_misses,
        total_files = final_stats.total_files,
        hit_rate_percent = hit_rate,
        valid_files = final_stats.valid_files,
        invalid_files = final_stats.invalid_files,
        "Validation complete"
    );

    if let Err(e) = event_tx.send(ValidationEvent::Finished(final_stats)) {
        tracing::warn!(stats = ?e.0, "Failed to send Finished event: receiver dropped");
    }
}
