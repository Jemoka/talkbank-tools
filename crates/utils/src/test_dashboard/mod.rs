//! Entry-point orchestration for the corpus test dashboard binary.

pub mod app;
pub mod manifest;
pub mod runner;
pub mod ui;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, channel},
};
use std::thread;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use talkbank_transform::{CorpusManifest, UnifiedCache};

use crate::test_dashboard::app::{
    AppState, Args, DashboardEvent, WorkerLoopContext, home_dir_or_exit, restore_terminal,
    setup_terminal,
};
use crate::test_dashboard::manifest::DashboardManifest;
use crate::test_dashboard::runner::worker_loop;
use crate::test_dashboard::ui::render_dashboard;

/// Run the dashboard binary end to end.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut terminal = setup_terminal()?;

    let manifest_path = home_dir_or_exit().join(".cache/talkbank-tools/corpus-manifest.json");
    if !manifest_path.exists() {
        restore_terminal(&mut terminal)?;
        eprintln!("Manifest not found: {}", manifest_path.display());
        eprintln!("Run: cargo run --release --bin build-corpus-manifest");
        std::process::exit(1);
    }

    let manifest = match CorpusManifest::load(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            restore_terminal(&mut terminal)?;
            eprintln!("Failed to load manifest: {}", error);
            std::process::exit(1);
        }
    };

    let cache = if args.no_cache {
        match UnifiedCache::in_memory() {
            Ok(cache) => cache,
            Err(error) => {
                restore_terminal(&mut terminal)?;
                eprintln!("Failed to create in-memory cache: {}", error);
                std::process::exit(1);
            }
        }
    } else {
        match UnifiedCache::new() {
            Ok(cache) => cache,
            Err(error) => match UnifiedCache::in_memory() {
                Ok(cache) => {
                    eprintln!("Warning: Falling back to in-memory cache: {}", error);
                    cache
                }
                Err(error) => {
                    restore_terminal(&mut terminal)?;
                    eprintln!("Failed to create cache: {}", error);
                    std::process::exit(1);
                }
            },
        }
    };

    let manifest_store = DashboardManifest::new(manifest, manifest_path.clone());

    let mut state = AppState::new();
    state.initialize_from_manifest(manifest_store.manifest());

    let corpus_paths = {
        let mut list: Vec<(String, String, usize, usize)> = manifest_store
            .manifest()
            .corpora
            .iter()
            .filter(|(_, entry)| entry.not_tested > 0)
            .map(|(key, value)| {
                (
                    key.clone(),
                    value.name.clone(),
                    value.file_count,
                    value.not_tested,
                )
            })
            .collect();
        list.sort_by_key(|entry| std::cmp::Reverse(entry.2));
        list
    };

    let should_stop = Arc::new(AtomicBool::new(false));
    let should_pause = Arc::new(AtomicBool::new(false));
    let should_skip_corpus = Arc::new(AtomicBool::new(false));
    let (event_tx, event_rx) = channel();

    let worker_thread = thread::spawn({
        let should_stop = Arc::clone(&should_stop);
        let should_pause = Arc::clone(&should_pause);
        let should_skip_corpus = Arc::clone(&should_skip_corpus);
        let auto_mode = args.auto;

        move || {
            worker_loop(WorkerLoopContext {
                manifest_store,
                cache,
                corpus_paths,
                event_tx,
                should_stop,
                should_pause,
                should_skip_corpus,
                auto_mode,
            })
        }
    });

    loop {
        drain_dashboard_events(&event_rx, &mut state);
        state.tick_elapsed();

        terminal.draw(|frame| render_dashboard(frame, &state))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    should_stop.store(true, Ordering::Relaxed);
                    break;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    should_stop.store(true, Ordering::Relaxed);
                    break;
                }
                KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char(' ') => {
                    let paused = should_pause.load(Ordering::Relaxed);
                    should_pause.store(!paused, Ordering::Relaxed);
                    state.is_paused = !paused;
                    state.status_message = if !paused {
                        "Paused by user".to_string()
                    } else {
                        "Resumed".to_string()
                    };
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    should_skip_corpus.store(true, Ordering::Relaxed);
                    state.status_message = "Skipping current corpus...".to_string();
                }
                _ => {}
            }
        }

        if should_stop.load(Ordering::Relaxed) {
            break;
        }
    }

    let _ = worker_thread.join();
    drain_dashboard_events(&event_rx, &mut state);

    restore_terminal(&mut terminal)?;

    println!("\nFinal Results:");
    println!(
        "  Total tested: {} files ({:.1}% of {})",
        state.total_passed + state.total_failed,
        state.overall_progress_pct(),
        state.total_files
    );
    println!("  ✓ Passed: {} files", state.total_passed);
    println!("  ✗ Failed: {} files", state.total_failed);
    println!("  Cache hit rate: {:.1}%", state.cache_hit_rate());
    println!("\nManifest saved to: {}", manifest_path.display());

    Ok(())
}

/// Drain pending worker events into the UI-owned state snapshot.
fn drain_dashboard_events(event_rx: &Receiver<DashboardEvent>, state: &mut AppState) {
    while let Ok(event) = event_rx.try_recv() {
        state.apply_event(event);
    }
}
