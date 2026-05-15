//! Worker-thread execution for bounded parallel roundtrip testing.
//!
//! Workers only own parsing, cache access, and per-file status production. They
//! do not mutate shared aggregate state. Instead, each worker sends completed
//! file results back to the coordinator, which owns summary statistics.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::UnifiedCache;

use super::runner::{RoundtripParser, RoundtripParserKind, test_roundtrip_file_with_parser};
use super::types::{FailureReason, FileStatus, RoundtripEvent};

/// Configuration passed to each worker thread.
pub(super) struct WorkerConfig {
    /// Whether roundtrip pass/fail results should be read from and written to cache.
    pub use_cache: bool,
    /// Whether JSON and diff artifacts should be emitted for failed files.
    pub emit_artifacts: bool,
    /// Whether alignment validation should be part of the roundtrip parse step.
    pub check_alignment: bool,
    /// Which parser implementation the worker should instantiate.
    pub parser_kind: RoundtripParserKind,
}

/// Process work items until the work queue closes or cancellation is requested.
pub(super) fn worker_loop(
    work_rx: Receiver<PathBuf>,
    result_tx: Sender<RoundtripEvent>,
    cancel_rx: Receiver<()>,
    cache: Option<Arc<UnifiedCache>>,
    config: WorkerConfig,
    corpus_dir: PathBuf,
) {
    let parser = match create_parser(config.parser_kind) {
        Some(parser) => parser,
        None => return,
    };

    loop {
        match cancel_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        match work_rx.recv() {
            Ok(file_path) => {
                let status = run_file_roundtrip(&file_path, &corpus_dir, &parser, &cache, &config);
                let _ = result_tx.send(RoundtripEvent::FileComplete {
                    path: file_path,
                    status,
                });
            }
            Err(_) => break,
        }
    }
}

/// Create the parser instance owned by one worker thread.
fn create_parser(_parser_kind: RoundtripParserKind) -> Option<RoundtripParser> {
    match TreeSitterParser::new() {
        Ok(parser) => Some(RoundtripParser(parser)),
        Err(error) => {
            eprintln!("Error creating parser: {:?}", error);
            None
        }
    }
}

/// Execute roundtrip testing for one file, consulting the cache when enabled.
fn run_file_roundtrip(
    file_path: &PathBuf,
    corpus_dir: &PathBuf,
    parser: &RoundtripParser,
    cache: &Option<Arc<UnifiedCache>>,
    config: &WorkerConfig,
) -> FileStatus {
    if config.use_cache
        && let Some(cached_status) = cached_status(file_path, cache, config)
    {
        return cached_status;
    }

    let status = test_roundtrip_file_with_parser(
        file_path,
        corpus_dir,
        parser,
        config.emit_artifacts,
        config.check_alignment,
    );

    if config.use_cache {
        write_cache_result(file_path, cache, config, &status);
    }

    status
}

/// Read a cached roundtrip result for one file when available.
fn cached_status(
    file_path: &Path,
    cache: &Option<Arc<UnifiedCache>>,
    config: &WorkerConfig,
) -> Option<FileStatus> {
    let roundtrip_passed = cache.as_ref().and_then(|cache_ref| {
        cache_ref.get_roundtrip(
            file_path,
            config.check_alignment,
            config.parser_kind.cache_label(),
        )
    })?;

    Some(if roundtrip_passed {
        FileStatus::Passed { cache_hit: true }
    } else {
        FileStatus::Failed {
            reason: FailureReason::ParseError(String::from("cached failure")),
            cache_hit: true,
        }
    })
}

/// Persist the pass/fail-only roundtrip result back into cache.
fn write_cache_result(
    file_path: &PathBuf,
    cache: &Option<Arc<UnifiedCache>>,
    config: &WorkerConfig,
    status: &FileStatus,
) {
    let passed = matches!(status, FileStatus::Passed { .. });
    if let Some(cache_ref) = cache.as_ref()
        && let Err(error) = cache_ref.set_roundtrip(
            file_path,
            config.check_alignment,
            config.parser_kind.cache_label(),
            passed,
        )
    {
        eprintln!(
            "Warning: Failed to cache result for {:?}: {}",
            file_path, error
        );
    }
}
