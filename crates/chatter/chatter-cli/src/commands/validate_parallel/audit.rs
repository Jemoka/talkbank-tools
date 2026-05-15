//! Audit-mode validation that streams JSONL output without cache writes.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::commands::CacheRefreshMode;
use crate::commands::validate::audit_reporter::AuditReporter;
use crate::commands::validate::cache::{get_cached_validation, initialize_validation_cache};
use talkbank_model::{ErrorCollector, ParseValidateOptions};
use talkbank_transform::parse_and_validate_streaming;
use talkbank_transform::validation_runner::ValidationStatsSnapshot;

/// Run validation in audit mode and stream error details to `audit_output`.
pub fn run_audit_mode(
    path: &Path,
    audit_output: &Path,
    check_alignment: bool,
    cache_refresh: CacheRefreshMode,
) -> ValidationStatsSnapshot {
    println!("Running validation in audit mode...");
    println!("Output file: {}", audit_output.display());
    println!();

    let audit_reporter = match AuditReporter::new(audit_output) {
        Ok(reporter) => reporter,
        Err(error) => {
            eprintln!("Error creating audit output file: {}", error);
            std::process::exit(1);
        }
    };
    let audit_handle = audit_reporter.reporter();

    let cache = initialize_validation_cache(path, cache_refresh);
    let files = discover_chat_files(path);

    println!("Found {} files to validate", files.len());
    println!();

    let options = if check_alignment {
        ParseValidateOptions::default().with_alignment()
    } else {
        ParseValidateOptions::default().with_validation()
    };

    let cache_hits = AtomicUsize::new(0);
    let cache_misses = AtomicUsize::new(0);
    let progress_counter = AtomicUsize::new(0);
    let total_files = files.len();
    let num_workers = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(4);

    eprintln!("Using {} worker threads", num_workers);

    let (work_tx, work_rx) = crossbeam_channel::bounded::<&Path>(num_workers * 2);

    std::thread::scope(|scope| {
        for _ in 0..num_workers {
            let work_rx = work_rx.clone();
            let audit_handle = audit_handle.clone();
            let cache = &cache;
            let cache_hits = &cache_hits;
            let cache_misses = &cache_misses;
            let progress_counter = &progress_counter;
            let options = &options;

            scope.spawn(move || {
                for file_path in work_rx {
                    let skip_file = if let Some(cache) = cache.as_ref() {
                        match get_cached_validation(Some(cache), file_path, check_alignment) {
                            Some(true) => {
                                cache_hits.fetch_add(1, Ordering::Relaxed);
                                true
                            }
                            _ => {
                                cache_misses.fetch_add(1, Ordering::Relaxed);
                                false
                            }
                        }
                    } else {
                        false
                    };

                    let done = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if done.is_multiple_of(500) {
                        eprintln!("Progress: {}/{} files...", done, total_files);
                    }

                    if skip_file {
                        continue;
                    }

                    let content = match fs::read_to_string(file_path) {
                        Ok(content) => content,
                        Err(error) => {
                            eprintln!("Error reading {:?}: {}", file_path, error);
                            continue;
                        }
                    };

                    let temp_sink = ErrorCollector::new();
                    match parse_and_validate_streaming(&content, options.clone(), &temp_sink) {
                        Ok(_) => {
                            let errors = temp_sink.into_vec();
                            audit_handle.report_file_results(&file_path.to_string_lossy(), errors);
                        }
                        Err(error) => {
                            eprintln!("Parse error in {:?}: {}", file_path, error);
                            audit_handle.mark_file_done(true);
                        }
                    }
                }
            });
        }

        drop(work_rx);

        for file_path in &files {
            if work_tx.send(file_path.as_path()).is_err() {
                break;
            }
        }

        drop(work_tx);
    });

    let cache_hits = cache_hits.load(Ordering::Relaxed);
    let cache_misses = cache_misses.load(Ordering::Relaxed);

    let stats = match audit_reporter.finish() {
        Ok(stats) => stats,
        Err(error) => {
            eprintln!("Error finalizing audit output: {}", error);
            std::process::exit(1);
        }
    };
    stats.print_summary();

    println!("Cache hits: {}", cache_hits);
    println!("Cache misses: {}", cache_misses);
    if cache_hits + cache_misses > 0 {
        println!(
            "Hit rate: {:.1}%",
            100.0 * cache_hits as f64 / (cache_hits + cache_misses) as f64
        );
    }
    println!();
    println!("Detailed errors written to: {}", audit_output.display());

    ValidationStatsSnapshot {
        total_files: stats.total_files,
        valid_files: stats.total_files - stats.files_with_errors,
        invalid_files: stats.files_with_errors,
        cache_hits,
        cache_misses,
        parse_errors: 0,
        roundtrip_passed: 0,
        roundtrip_failed: 0,
        cancelled: false,
    }
}

/// Discover all `.cha` files rooted at `path`, or return `path` itself when it is a file.
fn discover_chat_files(path: &Path) -> Vec<std::path::PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("cha"))
                .unwrap_or(false)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}
