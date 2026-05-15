//! Worker-loop and corpus-test execution for the test dashboard.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::thread;
use std::time::Duration;

use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::corpus::manifest::{ErrorDetail, FailureReason};
use talkbank_transform::{UnifiedCache, parse_and_validate_with_parser};

use crate::test_dashboard::app::{
    DashboardEvent, FileProgressUpdate, FileTestOutcome, WorkerLoopContext,
};

/// Process corpora sequentially, update manifest/state, and checkpoint progress to disk.
pub fn worker_loop(ctx: WorkerLoopContext) {
    let WorkerLoopContext {
        manifest_store,
        cache,
        corpus_paths,
        event_tx,
        should_stop,
        should_pause,
        should_skip_corpus,
        auto_mode,
    } = ctx;
    let mut manifest_store = manifest_store;

    for (corpus_idx, (corpus_path_key, corpus_name, file_count, _not_tested)) in
        corpus_paths.iter().enumerate()
    {
        if should_stop.load(Ordering::Relaxed) {
            break;
        }

        let corpus_path = match manifest_store.corpus_path(corpus_path_key) {
            Ok(path) => path,
            Err(error) => {
                send_status(&event_tx, false, false, error);
                break;
            }
        };

        send_event(
            &event_tx,
            DashboardEvent::CorpusStarted {
                corpus_idx,
                corpus_name: corpus_name.clone(),
                file_count: *file_count,
            },
        );

        match test_corpus(
            &corpus_path,
            &cache,
            &event_tx,
            &should_pause,
            &should_skip_corpus,
        ) {
            Ok(results) => {
                let summary = match manifest_store.commit_results(corpus_path_key, &results) {
                    Ok(summary) => summary,
                    Err(error) => {
                        send_status(&event_tx, false, false, error);
                        break;
                    }
                };

                send_event(
                    &event_tx,
                    DashboardEvent::TotalsCommitted {
                        corpus_name: corpus_name.clone(),
                        newly_passed: summary.newly_passed,
                        newly_failed: summary.newly_failed,
                    },
                );
                if let Err(error) = manifest_store.save() {
                    send_status(&event_tx, false, false, error);
                    break;
                }

                if !auto_mode && corpus_idx < corpus_paths.len() - 1 {
                    send_status(
                        &event_tx,
                        false,
                        true,
                        "Waiting for user... (Press P to continue, S to skip, Q to quit)",
                    );
                    should_pause.store(true, Ordering::Relaxed);

                    while should_pause.load(Ordering::Relaxed) {
                        if should_stop.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            Err(error) => {
                send_status(
                    &event_tx,
                    false,
                    false,
                    format!("Error testing corpus: {}", error),
                );
            }
        }

        should_skip_corpus.store(false, Ordering::Relaxed);
    }

    if let Err(error) = manifest_store.save() {
        send_status(&event_tx, false, false, error);
    }
    send_event(&event_tx, DashboardEvent::Finished);
    should_stop.store(true, Ordering::Relaxed);
}

/// Test all `.cha` files under one corpus path while honoring pause and skip controls.
pub fn test_corpus(
    corpus_path: &Path,
    cache: &UnifiedCache,
    event_tx: &Sender<DashboardEvent>,
    should_pause: &Arc<AtomicBool>,
    should_skip_corpus: &Arc<AtomicBool>,
) -> Result<Vec<FileTestOutcome>, String> {
    let mut results = Vec::new();
    let parser =
        TreeSitterParser::new().map_err(|error| format!("Failed to create parser: {:?}", error))?;

    let mut files = Vec::new();
    find_cha_files(corpus_path, &mut files)?;
    files.sort();

    for (idx, file_path) in files.iter().enumerate() {
        while should_pause.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
        }

        if should_skip_corpus.load(Ordering::Relaxed) {
            break;
        }

        let cached_passed = cache.get_roundtrip(file_path, true, "tree-sitter");
        let result = if let Some(rt_passed) = cached_passed {
            FileTestOutcome::new(file_path.clone(), rt_passed, None, None)
        } else {
            match run_file_test(&parser, file_path) {
                Ok(()) => {
                    let _ = cache.set_roundtrip(file_path, true, "tree-sitter", true);
                    FileTestOutcome::new(file_path.clone(), true, None, None)
                }
                Err((reason, error_detail)) => {
                    let _ = cache.set_roundtrip(file_path, true, "tree-sitter", false);
                    FileTestOutcome::new(file_path.clone(), false, Some(reason), Some(error_detail))
                }
            }
        };

        send_event(
            event_tx,
            DashboardEvent::FileProgress(FileProgressUpdate {
                tested: idx + 1,
                cache_hit: cached_passed.is_some(),
                passed: result.passed,
                failure_message: result.failure_message(),
            }),
        );

        results.push(result);
    }

    Ok(results)
}

/// Execute parse, validate, serialize, and semantic reparse roundtrip for one CHAT file.
pub fn run_file_test(
    parser: &TreeSitterParser,
    file_path: &Path,
) -> Result<(), (FailureReason, ErrorDetail)> {
    let original_content = std::fs::read_to_string(file_path).map_err(|error| {
        (
            FailureReason::ReadError,
            ErrorDetail::new("ReadError", format!("Failed to read file: {}", error)),
        )
    })?;

    let options = talkbank_model::ParseValidateOptions::default().with_alignment();
    let chat_file = match parse_and_validate_with_parser(parser, &original_content, options.clone())
    {
        Ok(chat_file) => chat_file,
        Err(talkbank_transform::PipelineError::Validation(errors)) => {
            let error_msg = errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "Validation failed".to_string());
            let detail = ErrorDetail::new("ValidationError", error_msg);
            return Err((FailureReason::ValidationError, detail));
        }
        Err(talkbank_transform::PipelineError::Parse(errors)) => {
            let error_msg = errors
                .errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "Parse failed".to_string());
            let detail = ErrorDetail::new("ParseError", error_msg);
            return Err((FailureReason::ParseError, detail));
        }
        Err(error) => {
            let detail = ErrorDetail::new("ParseError", format!("{:?}", error));
            return Err((FailureReason::ParseError, detail));
        }
    };

    let mut serialized = String::new();
    chat_file.write_chat(&mut serialized).map_err(|error| {
        (
            FailureReason::ChatMismatch,
            ErrorDetail::new(
                "ChatMismatch",
                format!("CHAT serialization failed: {}", error),
            ),
        )
    })?;

    let reparsed = match parse_and_validate_with_parser(parser, &serialized, options) {
        Ok(chat_file) => chat_file,
        Err(error) => {
            return Err((
                FailureReason::ChatMismatch,
                ErrorDetail::new(
                    "ChatMismatch",
                    format!("Failed to re-parse serialized CHAT: {:?}", error),
                ),
            ));
        }
    };

    if !chat_file.semantic_eq(&reparsed) {
        return Err((
            FailureReason::ChatMismatch,
            ErrorDetail::new("ChatMismatch", "Semantic roundtrip mismatch"),
        ));
    }

    Ok(())
}

/// Recursively discover `.cha` files rooted at `dir`.
pub fn find_cha_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("Failed to read directory: {}", error))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read entry: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            find_cha_files(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("cha") {
            files.push(path);
        }
    }

    Ok(())
}

/// Send one best-effort dashboard event to the UI thread.
fn send_event(event_tx: &Sender<DashboardEvent>, event: DashboardEvent) {
    let _ = event_tx.send(event);
}

/// Send one best-effort status update to the UI thread.
fn send_status(
    event_tx: &Sender<DashboardEvent>,
    is_testing: bool,
    is_paused: bool,
    message: impl Into<String>,
) {
    send_event(
        event_tx,
        DashboardEvent::Status {
            is_testing,
            is_paused,
            message: message.into(),
        },
    );
}
