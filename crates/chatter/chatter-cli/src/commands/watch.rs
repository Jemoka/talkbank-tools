//! Watch mode - continuously validate CHAT files as they change.
//!
//! Uses the `notify` crate to monitor file system events and re-validate
//! files when they are modified.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crossbeam_channel::{Sender, select, unbounded};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::cli::OutputFormat;
use crate::commands::{self, AlignmentValidationMode, CacheRefreshMode, ValidationInterface};
use crate::ui::Theme;

/// Watch CHAT files for changes and continuously validate, re-using the same validation/audit rules as the CLI.
///
/// The manual encourages tooling to keep transcripts in sync with their Golden copy, so this mode monitors file events,
/// debounces flapping editors, and reruns the same validation/alignment pipeline tied to the Main Tier/Dependent Tier
/// sections. The mode also suppresses `%wor` alignment violations (per the Alignment chapter) because temporary
/// flushes while typing should not pollute the console.
pub fn watch_files(
    path: &Path,
    check_alignment: bool,
    recursive: bool,
    clear_screen: bool,
) -> Result<(), WatchError> {
    if !path.exists() {
        return Err(WatchError::MissingPath {
            path: path.to_path_buf(),
        });
    }

    println!("👀 Watching {} for changes...", path.display());
    if recursive {
        println!("   (recursive mode)");
    }
    println!("   Press Ctrl+C to stop\n");

    // Run initial validation
    if path.is_file() {
        validate_with_header(path, check_alignment, clear_screen);
    }

    // Set up file watcher (debounced events ensure we do not over-drain CPU on editors that fire multi events)
    let (event_tx, event_rx) = unbounded();
    let mut watcher = create_watcher(event_tx)?;

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    watcher
        .watch(path, mode)
        .map_err(|source| WatchError::Watch {
            path: path.to_path_buf(),
            source,
        })?;

    // Set up Ctrl+C handler
    let (ctrl_c_tx, ctrl_c_rx) = unbounded();
    ctrlc::set_handler(move || {
        let _ = ctrl_c_tx.send(());
    })
    .map_err(|source| WatchError::CtrlC { source })?;

    // Debounce: wait 500ms after last event before validating
    let debounce_duration = Duration::from_millis(500);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        // Check for events or ctrl-c
        select! {
            recv(event_rx) -> msg => {
                if let Ok(file_path) = msg {
                    // Mark this file as pending with current time
                    pending.insert(file_path, Instant::now());
                }
            }
            recv(ctrl_c_rx) -> _ => {
                println!("\n👋 Stopping watch mode...");
                break;
            }
            default(Duration::from_millis(100)) => {
                // Check if any pending files are ready (debounce expired)
                let now = Instant::now();
                let ready: Vec<PathBuf> = pending
                    .iter()
                    .filter(|(_, last_event)| now.duration_since(**last_event) >= debounce_duration)
                    .map(|(path, _)| path.clone())
                    .collect();

                for file_path in ready {
                    pending.remove(&file_path);
                    validate_with_header(file_path.as_path(), check_alignment, clear_screen);
                }
            }
        }
    }

    Ok(())
}

/// Errors returned while setting up or running filesystem watch mode.
///
/// The watch mode is long-running, so each variant includes enough context to correlate with
/// the CLI’s monitoring/uptime guidance in the File Format appendix (e.g., missing paths or
/// watcher limitations reported verbatim to users).
#[derive(Debug, Error)]
pub enum WatchError {
    #[error("Path does not exist: {path}")]
    MissingPath { path: PathBuf },
    #[error("Failed to create file watcher")]
    CreateWatcher { source: notify::Error },
    #[error("Failed to watch path: {path}")]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
    #[error("Failed to set Ctrl+C handler")]
    CtrlC { source: ctrlc::Error },
}

/// Builds watcher for downstream use.
///
/// Returns a `notify::RecommendedWatcher` configured only to observe `.cha` create/modify events so
/// edit-time validation stays within the CHAT file format constraints.
fn create_watcher(tx: Sender<PathBuf>) -> Result<RecommendedWatcher, WatchError> {
    notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            // Only care about modify and create events
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                for path in event.paths {
                    // Only process .cha files
                    if path.extension().and_then(|s| s.to_str()) == Some("cha") {
                        let _ = tx.send(path);
                    }
                }
            }
        }
    })
    .map_err(|source| WatchError::CreateWatcher { source })
}

/// Helper that clears the screen, prints a header, and invokes `validate_file`.
///
/// Watch mode always suppresses `%wor` alignment failures (they are expensive for interactive use) and keeps the
/// same Main Tier/Dependent Tier validation rules described in the manual’s CLI section.
fn validate_with_header(path: &Path, check_alignment: bool, clear_screen: bool) {
    if clear_screen {
        // ANSI escape: clear screen and move cursor to top-left
        print!("\x1B[2J\x1B[1;1H");
    }

    println!("📝 Validating: {}", path.display());
    println!("{}", "─".repeat(60));

    // Watch mode shows results to the user inline; the per-file outcome
    // does not influence anything outside the loop, so it is intentionally
    // discarded.
    let _ = commands::validate_file(
        &path.to_path_buf(),
        OutputFormat::Text,
        AlignmentValidationMode::from_enabled(check_alignment),
        CacheRefreshMode::ReuseExisting,
        false,
        ValidationInterface::Plain,
        Theme::default(),
        &[],
        false, // strict_linkers disabled in watch mode
    );

    println!();
}
