//! Test module for runner in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

// Roundtrip test runner - some functions are used conditionally
#![allow(dead_code, clippy::ptr_arg, clippy::too_many_arguments)]

use super::types::{FailureReason, FileStatus};
use std::fs;
use std::path::PathBuf;

use crate::test_utils::diagnostics::print_pipeline_error;
use crate::test_utils::roundtrip::{parse_for_roundtrip, parse_for_roundtrip_with_parser};
use talkbank_model::model::{ChatFile, SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::PipelineError;
use talkbank_transform::to_json_pretty_validated;

/// Parser selection for roundtrip corpus testing.
///
/// Only tree-sitter is supported (Chumsky/direct parser has been removed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundtripParserKind {
    TreeSitter,
}

impl RoundtripParserKind {
    /// Returns the cache label for this parser kind.
    pub fn cache_label(self) -> &'static str {
        match self {
            RoundtripParserKind::TreeSitter => "tree-sitter",
        }
    }
}

/// Parser wrapper for roundtrip testing.
pub struct RoundtripParser(pub TreeSitterParser);

impl RoundtripParser {
    /// Parse a file for roundtrip testing.
    pub fn parse_for_roundtrip(
        &self,
        content: &str,
        check_alignment: bool,
    ) -> Result<ChatFile, PipelineError> {
        parse_for_roundtrip_with_parser(&self.0, content, check_alignment)
    }
}

/// Run roundtrip test on a single file and return FileStatus
pub fn test_roundtrip_file(
    file_path: &PathBuf,
    corpus_dir: &PathBuf,
    emit_artifacts: bool,
    check_alignment: bool,
) -> FileStatus {
    match test_roundtrip_file_internal(file_path, corpus_dir, emit_artifacts, |content| {
        parse_for_roundtrip(content, check_alignment)
    }) {
        Ok(()) => FileStatus::Passed { cache_hit: false },
        Err(reason) => FileStatus::Failed {
            reason,
            cache_hit: false,
        },
    }
}

/// Run roundtrip test on a single file using a shared parser
pub fn test_roundtrip_file_with_parser(
    file_path: &PathBuf,
    corpus_dir: &PathBuf,
    parser: &RoundtripParser,
    emit_artifacts: bool,
    check_alignment: bool,
) -> FileStatus {
    let parse_with_parser = |content: &str| parser.parse_for_roundtrip(content, check_alignment);
    match test_roundtrip_file_internal(file_path, corpus_dir, emit_artifacts, parse_with_parser) {
        Ok(()) => FileStatus::Passed { cache_hit: false },
        Err(reason) => FileStatus::Failed {
            reason,
            cache_hit: false,
        },
    }
}

/// Internal roundtrip test logic
/// Returns Ok(()) on success or FailureReason on failure
fn test_roundtrip_file_internal<F>(
    file_path: &PathBuf,
    _corpus_dir: &PathBuf,
    emit_artifacts: bool,
    parse_file: F,
) -> Result<(), FailureReason>
where
    F: Fn(&str) -> Result<ChatFile, PipelineError>,
{
    let file_name = match file_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => {
            return Err(FailureReason::ReadError(format!(
                "Missing filename for path {}",
                file_path.display()
            )));
        }
    };

    let original_content = fs::read_to_string(file_path)
        .map_err(|e| FailureReason::ReadError(format!("Failed to read {}: {}", file_name, e)))?;

    // Use shared roundtrip parsing logic - DO NOT DUPLICATE
    let chat_file = parse_file(&original_content).map_err(|e| {
        print_pipeline_error(Some(file_path), &original_content, &e);
        FailureReason::ParseError(format!("{}: {}", file_name, e))
    })?;

    let mut diff_base_path: Option<PathBuf> = None;
    if emit_artifacts {
        // Serialize to JSON with schema validation
        let json_string = to_json_pretty_validated(&chat_file).map_err(|e| {
            FailureReason::ParseError(format!(
                "{} - JSON serialization/validation failed: {}",
                file_name, e
            ))
        })?;

        // Convert absolute path to relative for output directory structure
        let json_relative_path = if file_path.is_absolute() {
            // For absolute paths, use the path components without the root
            file_path
                .components()
                .filter(|c| !matches!(c, std::path::Component::RootDir))
                .collect::<PathBuf>()
                .with_extension("json")
        } else {
            file_path.with_extension("json")
        };

        // Write output files to ~/.cache/talkbank-chat/ (non-fatal failures)
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("talkbank-chat")
            .join("roundtrip-artifacts");
        let json_output_dir = cache_dir.join("json");
        let diff_output_dir = cache_dir.join("diffs");

        // Create JSON file with full absolute path hierarchy mirrored
        let json_file_path = json_output_dir.join(&json_relative_path);
        if let Some(parent) = json_file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&json_file_path, &json_string);

        // Diff files mirror full path structure in separate directory tree
        let diff_path = diff_output_dir.join(&json_relative_path).with_extension("");
        if let Some(parent) = diff_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        diff_base_path = Some(diff_path);
    }

    let serialized = chat_file.to_chat_string();

    // Re-parse the serialized CHAT to get the second model
    let reparsed_file = parse_file(&serialized).map_err(|e| {
        print_pipeline_error(Some(file_path), &serialized, &e);
        FailureReason::ParseError(format!(
            "{}: Failed to re-parse serialized CHAT: {}",
            file_name, e
        ))
    })?;

    // Semantic comparison - ignores span fields and computed metadata
    if !chat_file.semantic_eq(&reparsed_file) {
        if let Some(diff_base_path) = &diff_base_path {
            let serialized_raw_file = diff_base_path.with_extension("cha.serialized-raw");
            let json_reparsed_file = diff_base_path.with_extension("json-reparsed");
            let semantic_diff_file = diff_base_path.with_extension("semantic-diff.txt");

            let _ = fs::write(&serialized_raw_file, &serialized);
            if let Ok(reparsed_json) = serde_json::to_string_pretty(&reparsed_file) {
                let _ = fs::write(&json_reparsed_file, &reparsed_json);
            }
            let diff_report =
                crate::test_utils::semantic_diff::analyze_semantic_diff(&chat_file, &reparsed_file);
            // Write comparison report showing both original and serialized context
            let _ = fs::write(
                &semantic_diff_file,
                diff_report.render_comparison(&file_name, &original_content, &serialized),
            );
        }
        let diff_report =
            crate::test_utils::semantic_diff::analyze_semantic_diff(&chat_file, &reparsed_file);
        // Use short comparison form for inline error message
        let summary =
            diff_report.render_comparison_short(&file_name, &original_content, &serialized);

        return Err(FailureReason::SemanticMismatch(summary));
    }

    Ok(())
}

/// Run streaming roundtrip tests with caching support
pub fn run_roundtrip_streaming(
    directory: &std::path::Path,
    use_cache: bool,
    emit_artifacts: bool,
    check_alignment: bool,
    parser_kind: RoundtripParserKind,
    cache: Option<std::sync::Arc<talkbank_transform::UnifiedCache>>,
) -> (
    crossbeam_channel::Receiver<super::types::RoundtripEvent>,
    crossbeam_channel::Sender<()>,
) {
    use crossbeam_channel::bounded;
    use std::thread;

    let (event_tx, event_rx) = bounded(100);
    let (cancel_tx, cancel_rx) = bounded(1);

    let dir = directory.to_path_buf();
    let corpus_dir = directory.to_path_buf();

    thread::spawn(move || {
        run_roundtrip_internal(
            dir,
            corpus_dir,
            use_cache,
            emit_artifacts,
            check_alignment,
            parser_kind,
            cache,
            event_tx,
            cancel_rx,
        );
    });

    (event_rx, cancel_tx)
}

/// Internal streaming roundtrip execution
fn run_roundtrip_internal(
    directory: std::path::PathBuf,
    corpus_dir: std::path::PathBuf,
    use_cache: bool,
    emit_artifacts: bool,
    check_alignment: bool,
    parser_kind: RoundtripParserKind,
    cache: Option<std::sync::Arc<talkbank_transform::UnifiedCache>>,
    event_tx: crossbeam_channel::Sender<super::types::RoundtripEvent>,
    cancel_rx: crossbeam_channel::Receiver<()>,
) {
    use super::discovery;
    use super::types::RoundtripStats;
    use super::worker::{WorkerConfig, worker_loop};
    use crossbeam_channel::bounded;
    use std::thread;

    // Discover all .cha files
    let mut files = Vec::new();
    discovery::find_cha_files(&directory, &mut files);
    files.sort();

    let total_files = files.len();

    // Send start event
    let _ = event_tx.send(super::types::RoundtripEvent::Started { total_files });

    // Create bounded work queue
    let (work_tx, work_rx) = bounded(total_files);
    let num_workers = num_cpus::get();
    let (result_tx, result_rx) = bounded(num_workers.saturating_mul(2).max(1));
    let mut stats = RoundtripStats::for_run(total_files);

    // Spawn worker threads
    let mut workers = Vec::new();

    for _ in 0..num_workers {
        let work_rx = work_rx.clone();
        let result_tx = result_tx.clone();
        let cancel_rx = cancel_rx.clone();
        let cache_ref = cache.clone();
        let corpus_dir_ref = corpus_dir.clone();

        let worker = thread::spawn(move || {
            worker_loop(
                work_rx,
                result_tx,
                cancel_rx,
                cache_ref,
                WorkerConfig {
                    use_cache,
                    emit_artifacts,
                    check_alignment,
                    parser_kind,
                },
                corpus_dir_ref,
            );
        });

        workers.push(worker);
    }
    drop(result_tx);

    // Send work items
    for file in files {
        if work_tx.send(file).is_err() {
            break;
        }
    }
    drop(work_tx);

    // Forward worker results and accumulate summary stats in one coordinator thread.
    for event in result_rx {
        if let super::types::RoundtripEvent::FileComplete { path, status } = event {
            stats.record_file_status(&status);
            let _ = event_tx.send(super::types::RoundtripEvent::FileComplete { path, status });
        }
    }

    // Wait for all workers to finish.
    for worker in workers {
        let _ = worker.join();
    }

    // Send final stats.
    let _ = event_tx.send(super::types::RoundtripEvent::Finished(stats));
}
