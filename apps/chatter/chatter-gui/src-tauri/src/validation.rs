//! Desktop validation orchestration for a single selected target.
//!
//! Chatter's desktop contract is one target at a time:
//! - one `.cha` file
//! - or one directory

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use talkbank_model::{ErrorCollector, ParseValidateOptions, Severity};
use talkbank_transform::UnifiedCache;
use talkbank_transform::parse_and_validate_streaming;
use talkbank_transform::validation_runner::{
    ErrorEvent, FileCompleteEvent, FileStatus, ValidationConfig, ValidationEvent,
    ValidationStatsSnapshot, validate_directory_streaming,
};

use crate::events::{FrontendEvent, to_frontend_event};

/// Start validation for a single desktop target and stream frontend events.
pub fn validate_target_streaming(
    target: PathBuf,
) -> Result<(Receiver<FrontendEvent>, Sender<()>), String> {
    if !target.exists() {
        return Err(format!("Path does not exist: {}", target.display()));
    }

    if target.is_dir() {
        Ok(spawn_directory_validation(target))
    } else if target.is_file() {
        if !is_chat_file(&target) {
            return Err(format!(
                "Chatter validates one .cha file or one folder at a time: {}",
                target.display()
            ));
        }
        Ok(spawn_single_file_validation(target))
    } else {
        Err(format!(
            "Path is not a file or directory: {}",
            target.display()
        ))
    }
}

fn spawn_directory_validation(target: PathBuf) -> (Receiver<FrontendEvent>, Sender<()>) {
    let config = ValidationConfig::default();
    let (validation_rx, cancel_tx) =
        validate_directory_streaming::<UnifiedCache>(&target, &config, None);

    (bridge_validation_events(validation_rx, target), cancel_tx)
}

fn bridge_validation_events(
    validation_rx: Receiver<ValidationEvent>,
    root: PathBuf,
) -> Receiver<FrontendEvent> {
    let (frontend_tx, frontend_rx) = unbounded();

    std::thread::spawn(move || {
        while let Ok(event) = validation_rx.recv() {
            if !emit_frontend_event(&frontend_tx, &root, event) {
                break;
            }
        }
    });

    frontend_rx
}

fn spawn_single_file_validation(target: PathBuf) -> (Receiver<FrontendEvent>, Sender<()>) {
    let (frontend_tx, frontend_rx) = unbounded();
    let (cancel_tx, cancel_rx) = bounded::<()>(1);

    std::thread::spawn(move || {
        let root = target
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        if !emit_frontend_event(&frontend_tx, &root, ValidationEvent::Discovering) {
            return;
        }

        if cancelled(&cancel_rx) {
            let _ = emit_frontend_event(
                &frontend_tx,
                &root,
                ValidationEvent::Finished(cancelled_stats()),
            );
            return;
        }

        if !emit_frontend_event(
            &frontend_tx,
            &root,
            ValidationEvent::Started { total_files: 1 },
        ) {
            return;
        }

        if cancelled(&cancel_rx) {
            let _ = emit_frontend_event(
                &frontend_tx,
                &root,
                ValidationEvent::Finished(cancelled_stats()),
            );
            return;
        }

        let source = match fs::read_to_string(&target) {
            Ok(source) => Arc::<str>::from(source),
            Err(error) => {
                let status = FileStatus::ReadError {
                    message: error.to_string(),
                };
                let stats = finished_stats(&status);
                let _ = emit_frontend_event(
                    &frontend_tx,
                    &root,
                    ValidationEvent::FileComplete(FileCompleteEvent {
                        path: target.clone(),
                        status,
                    }),
                );
                let _ = emit_frontend_event(&frontend_tx, &root, ValidationEvent::Finished(stats));
                return;
            }
        };

        if cancelled(&cancel_rx) {
            let _ = emit_frontend_event(
                &frontend_tx,
                &root,
                ValidationEvent::Finished(cancelled_stats()),
            );
            return;
        }

        let options = ParseValidateOptions::default().with_alignment();
        let collector = ErrorCollector::new();

        let status = match parse_and_validate_streaming(source.as_ref(), options, &collector) {
            Ok(_) => {
                let errors = collector.into_vec();
                let hard_error_count = errors
                    .iter()
                    .filter(|error| matches!(error.severity, Severity::Error))
                    .count();

                if cancelled(&cancel_rx) {
                    let _ = emit_frontend_event(
                        &frontend_tx,
                        &root,
                        ValidationEvent::Finished(cancelled_stats()),
                    );
                    return;
                }

                if !errors.is_empty()
                    && !emit_frontend_event(
                        &frontend_tx,
                        &root,
                        ValidationEvent::Errors(ErrorEvent {
                            path: target.clone(),
                            errors,
                            source: source.clone(),
                        }),
                    )
                {
                    return;
                }

                if hard_error_count == 0 {
                    FileStatus::Valid { cache_hit: false }
                } else {
                    FileStatus::Invalid {
                        error_count: hard_error_count,
                        cache_hit: false,
                    }
                }
            }
            Err(error) => FileStatus::ParseError {
                message: error.to_string(),
            },
        };

        let stats = finished_stats(&status);

        if !emit_frontend_event(
            &frontend_tx,
            &root,
            ValidationEvent::FileComplete(FileCompleteEvent {
                path: target,
                status,
            }),
        ) {
            return;
        }

        let _ = emit_frontend_event(&frontend_tx, &root, ValidationEvent::Finished(stats));
    });

    (frontend_rx, cancel_tx)
}

fn emit_frontend_event(
    frontend_tx: &Sender<FrontendEvent>,
    root: &Path,
    event: ValidationEvent,
) -> bool {
    match to_frontend_event(event, root) {
        Some(frontend_event) => frontend_tx.send(frontend_event).is_ok(),
        None => true,
    }
}

fn cancelled(cancel_rx: &Receiver<()>) -> bool {
    match cancel_rx.try_recv() {
        Ok(()) => true,
        Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => false,
    }
}

fn cancelled_stats() -> ValidationStatsSnapshot {
    ValidationStatsSnapshot {
        total_files: 1,
        valid_files: 0,
        invalid_files: 0,
        cache_hits: 0,
        cache_misses: 0,
        parse_errors: 0,
        roundtrip_passed: 0,
        roundtrip_failed: 0,
        cancelled: true,
    }
}

fn finished_stats(status: &FileStatus) -> ValidationStatsSnapshot {
    match status {
        FileStatus::Valid { cache_hit } => ValidationStatsSnapshot {
            total_files: 1,
            valid_files: 1,
            invalid_files: 0,
            cache_hits: usize::from(*cache_hit),
            cache_misses: usize::from(!cache_hit),
            parse_errors: 0,
            roundtrip_passed: 0,
            roundtrip_failed: 0,
            cancelled: false,
        },
        FileStatus::Invalid { cache_hit, .. } => ValidationStatsSnapshot {
            total_files: 1,
            valid_files: 0,
            invalid_files: 1,
            cache_hits: usize::from(*cache_hit),
            cache_misses: usize::from(!cache_hit),
            parse_errors: 0,
            roundtrip_passed: 0,
            roundtrip_failed: 0,
            cancelled: false,
        },
        FileStatus::RoundtripFailed { cache_hit, .. } => ValidationStatsSnapshot {
            total_files: 1,
            valid_files: 0,
            invalid_files: 1,
            cache_hits: usize::from(*cache_hit),
            cache_misses: usize::from(!cache_hit),
            parse_errors: 0,
            roundtrip_passed: 0,
            roundtrip_failed: 1,
            cancelled: false,
        },
        FileStatus::ParseError { .. } => ValidationStatsSnapshot {
            total_files: 1,
            valid_files: 0,
            invalid_files: 0,
            cache_hits: 0,
            cache_misses: 1,
            parse_errors: 1,
            roundtrip_passed: 0,
            roundtrip_failed: 0,
            cancelled: false,
        },
        FileStatus::ReadError { .. } => ValidationStatsSnapshot {
            total_files: 1,
            valid_files: 0,
            invalid_files: 1,
            cache_hits: 0,
            cache_misses: 1,
            parse_errors: 0,
            roundtrip_passed: 0,
            roundtrip_failed: 0,
            cancelled: false,
        },
    }
}

fn is_chat_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cha"))
}
