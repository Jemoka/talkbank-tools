//! Roundtrip test for arbitrary corpus directories with dual execution modes
//!
//! Default: Custom streaming runner with bounded memory usage and caching
//!
//! ## Usage
//!
//! Custom streaming mode (default):
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir ~/corpus
//! ```
//!
//! Disable caching:
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir ~/corpus --no-cache
//! ```
//!
//! Emit JSON/diff artifacts:
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir ~/corpus --emit-artifacts
//! ```
//!
//! Skip alignment validation:
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir ~/corpus --no-alignment
//! ```
//!
//! Use the direct parser:
//! ```bash
//! cargo test --release --test roundtrip_corpus -- --corpus-dir ~/corpus --direct
//! ```

#[path = "test_utils/mod.rs"]
mod test_utils;

#[path = "roundtrip_corpus/mod.rs"]
mod roundtrip_corpus;

use clap::Parser;
use roundtrip_corpus::{discovery, runner, types};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use talkbank_cache::UnifiedCache;

/// Type representing Args.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing the corpus to test
    #[arg(long, short = 'd')]
    corpus_dir: Option<PathBuf>,

    /// Disable caching of test results
    #[arg(long)]
    no_cache: bool,

    /// Write JSON and diff artifacts (off by default for speed)
    #[arg(long)]
    emit_artifacts: bool,

    /// Skip alignment validation for faster batch runs
    #[arg(long)]
    no_alignment: bool,
}

/// Entry point for this binary target.
fn main() {
    let args = Args::parse();

    let corpus_dir = match args.corpus_dir {
        Some(dir) => dir,
        None => {
            // Default to the reference corpus
            let default_corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/corpus/reference");
            if default_corpus.exists() {
                eprintln!(
                    "Using default reference corpus: {}",
                    default_corpus.display()
                );
                default_corpus
            } else {
                eprintln!(
                    "Error: Reference corpus not found at {}",
                    default_corpus.display()
                );
                eprintln!("Usage: cargo test --test roundtrip_corpus -- --corpus-dir ~/my-corpus");
                std::process::exit(1);
            }
        }
    };

    if !corpus_dir.exists() {
        eprintln!("Corpus directory does not exist: {}", corpus_dir.display());
        std::process::exit(1);
    }

    let mut files = Vec::new();
    discovery::find_cha_files(&corpus_dir, &mut files);
    files.sort();

    if files.is_empty() {
        eprintln!("No .cha files found in {}", corpus_dir.display());
        std::process::exit(1);
    }

    eprintln!(
        "Found {} .cha files in {}",
        files.len(),
        corpus_dir.display()
    );

    run_custom_mode(
        &corpus_dir,
        args.no_cache,
        args.emit_artifacts,
        !args.no_alignment,
    );
}

/// Run custom streaming mode with bounded memory and caching
fn run_custom_mode(corpus_dir: &Path, no_cache: bool, emit_artifacts: bool, check_alignment: bool) {
    use indicatif::{ProgressBar, ProgressStyle};

    let parser_kind = roundtrip_corpus::runner::RoundtripParserKind::TreeSitter;

    // Create cache (unless disabled)
    let cache = if !no_cache {
        match UnifiedCache::new() {
            Ok(c) => {
                eprintln!("Cache enabled: ~/.cache/talkbank-chat/talkbank-cache.db");
                Some(Arc::new(c))
            }
            Err(e) => {
                eprintln!("Warning: Failed to load cache: {}", e);
                None
            }
        }
    } else {
        eprintln!("Cache disabled (--no-cache)");
        None
    };

    // Run streaming roundtrip
    // Pass !no_cache (use_cache) to runner
    let (events_rx, _cancel_tx) = runner::run_roundtrip_streaming(
        corpus_dir,
        !no_cache,
        emit_artifacts,
        check_alignment,
        parser_kind,
        cache,
    );

    let mut stats = types::RoundtripStats {
        total_files: 0,
        passed: 0,
        failed: 0,
        cache_hits: 0,
        cache_misses: 0,
        cancelled: false,
    };
    let mut progress_bar: Option<ProgressBar> = None;
    let mut failed_files: Vec<(String, String)> = Vec::new();

    for event in events_rx {
        match event {
            types::RoundtripEvent::Started { total_files } => {
                let pb = ProgressBar::new(total_files as u64);
                let base_style = ProgressStyle::default_bar();
                let styled = match base_style.template(
                    "{msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) ETA: {eta_precise}",
                ) {
                    Ok(style) => style,
                    Err(err) => {
                        eprintln!("Warning: Failed to set progress template: {err}");
                        ProgressStyle::default_bar()
                    }
                };
                pb.set_style(styled.progress_chars("=>-"));
                pb.set_message("Testing");
                progress_bar = Some(pb);
            }
            types::RoundtripEvent::FileComplete { path, status } => {
                if let Some(pb) = &progress_bar {
                    pb.inc(1);

                    match &status {
                        types::FileStatus::Passed { cache_hit: true } => {
                            pb.set_message("Testing (cached)");
                        }
                        types::FileStatus::Failed { reason, .. } => {
                            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                            let reason_text = reason.to_string();
                            pb.println(format!("✗ {} - {}", file_name, reason_text));
                            failed_files.push((file_name.to_string(), reason_text));
                        }
                        _ => {}
                    }
                }
            }
            types::RoundtripEvent::Finished(final_stats) => {
                stats = final_stats;
                if let Some(pb) = progress_bar.take() {
                    pb.finish_with_message("Complete");
                }
            }
        }
    }

    // Print summary
    let cache_hit_rate = if stats.total_files > 0 {
        (stats.cache_hits as f64 / stats.total_files as f64) * 100.0
    } else {
        0.0
    };

    eprintln!();
    eprintln!(
        "Results: ✓ {} passed, ✗ {} failed, ⚡ {} cache hits ({:.1}% hit rate)",
        stats.passed, stats.failed, stats.cache_hits, cache_hit_rate
    );

    if stats.failed > 0 {
        eprintln!("\nFailed files:");
        for (file, reason) in failed_files {
            eprintln!("  - {}: {}", file, reason);
        }
        if emit_artifacts {
            eprintln!("\nDiff files written to: ~/talkbank-roundtrip-diffs/");
        }
        std::process::exit(1);
    }
}
