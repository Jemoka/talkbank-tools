//! Worker-thread execution logic for validation and optional roundtrip.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::cache::{CacheOutcome, ValidationCache};
use super::config::{CacheMode, ParserKind, ValidationConfig};
use super::roundtrip;
use super::types::{
    ErrorEvent, FileCompleteEvent, FileStatus, RoundtripEvent, ValidationEvent, ValidationStats,
};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use talkbank_model::{ChatFile, ChatParser, ErrorSink, ParseOutcome};
use talkbank_model::{ParseError, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Dispatch enum wrapping both parser backends.
///
/// Avoids trait objects (which would require `ErrorSink` monomorphization)
/// while keeping the worker code parser-agnostic.
pub(super) enum ParserDispatch {
    /// Tree-sitter parser (default, supports incremental reparsing).
    TreeSitter(TreeSitterParser),
    /// Re2c DFA parser (faster batch validation).
    Re2c(Re2cParser),
}

impl ParserDispatch {
    /// Create the appropriate parser for the given `ParserKind`.
    pub(super) fn new(kind: ParserKind) -> Result<Self, String> {
        match kind {
            ParserKind::TreeSitter => TreeSitterParser::new()
                .map(ParserDispatch::TreeSitter)
                .map_err(|e| format!("{e}")),
            ParserKind::Re2c => Ok(ParserDispatch::Re2c(Re2cParser::new())),
        }
    }

    /// Parse a complete CHAT file with streaming error reporting.
    ///
    /// Both backends always return a ChatFile (best-effort recovery).
    pub(super) fn parse_chat_file_streaming(
        &self,
        input: &str,
        errors: &impl ErrorSink,
    ) -> ChatFile {
        match self {
            Self::TreeSitter(p) => p.parse_chat_file_streaming(input, errors),
            Self::Re2c(p) => match p.parse_chat_file(input, 0, errors) {
                ParseOutcome::Parsed(cf) => cf,
                ParseOutcome::Rejected => ChatFile::new(vec![]),
            },
        }
    }
}

/// Main loop executed by each validation worker thread.
pub(super) fn worker_loop<C>(
    work_rx: Receiver<PathBuf>,
    event_tx: Sender<ValidationEvent>,
    cancel_rx: Receiver<()>,
    cache: Option<Arc<C>>,
    config: ValidationConfig,
    stats: Arc<ValidationStats>,
) where
    C: ValidationCache + Send + Sync,
{
    let parser = match ParserDispatch::new(config.parser_kind) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Error creating parser ({:?})", config.parser_kind);
            return;
        }
    };

    loop {
        // Check for cancellation
        match cancel_rx.try_recv() {
            Ok(()) => break,
            // Sender dropped is not a cancellation request.
            Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => {}
        }

        // Get next file from work queue
        match work_rx.recv() {
            Ok(file_path) => {
                // Attempt to serve from cache before touching the filesystem.
                // CacheOutcome::Valid = cached valid; Invalid = cached invalid (re-validate for errors).
                if config.cache == CacheMode::Enabled
                    && let Some(CacheOutcome::Valid) = cache
                        .as_ref()
                        .and_then(|cache_ref| cache_ref.get(&file_path, config.check_alignment))
                {
                    tracing::debug!(file = ?file_path, "Cache hit (valid) - skipping reparse");

                    // If roundtrip is requested, check roundtrip cache too
                    if config.roundtrip {
                        let roundtrip_cached = cache.as_ref().and_then(|c| {
                            c.get_roundtrip(
                                &file_path,
                                config.check_alignment,
                                config.parser_kind.cache_label(),
                            )
                        });
                        if let Some(rt_outcome) = roundtrip_cached {
                            let rt_passed = rt_outcome == CacheOutcome::Valid;
                            let status = if rt_passed {
                                FileStatus::Valid { cache_hit: true }
                            } else {
                                FileStatus::RoundtripFailed {
                                    cache_hit: true,
                                    reason: "Roundtrip failed (cached)".to_string(),
                                }
                            };

                            // Emit roundtrip event for the cached result
                            let _ =
                                event_tx.send(ValidationEvent::RoundtripComplete(RoundtripEvent {
                                    path: file_path.clone(),
                                    passed: rt_passed,
                                    failure_reason: if rt_passed {
                                        None
                                    } else {
                                        Some("Roundtrip failed (cached)".to_string())
                                    },
                                    diff: None,
                                }));

                            update_stats(&stats, &status);
                            if event_tx
                                .send(ValidationEvent::FileComplete(FileCompleteEvent {
                                    path: file_path,
                                    status,
                                }))
                                .is_err()
                            {
                                break;
                            }
                            continue;
                        }
                        // else: roundtrip not cached, fall through to full processing
                    } else {
                        // No roundtrip needed, just use validation cache hit
                        let status = FileStatus::Valid { cache_hit: true };
                        update_stats(&stats, &status);
                        if event_tx
                            .send(ValidationEvent::FileComplete(FileCompleteEvent {
                                path: file_path,
                                status,
                            }))
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                }

                // Cache miss or invalid file — need to parse
                tracing::debug!(file = ?file_path, "Cache miss - parsing file");

                // Read file content
                let content = match fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        let status = FileStatus::ReadError {
                            message: e.to_string(),
                        };

                        if let Err(_send_error) =
                            event_tx.send(ValidationEvent::FileComplete(FileCompleteEvent {
                                path: file_path.clone(),
                                status: status.clone(),
                            }))
                        {
                            tracing::warn!(file = ?file_path, "Failed to send FileComplete event: receiver dropped");
                        }

                        update_stats(&stats, &status);
                        continue;
                    }
                };

                let source = Arc::<str>::from(content);

                let (errors, chat_file) = validate_single_file_streaming(
                    &file_path,
                    source.clone(),
                    config.check_alignment,
                    config.strict_linkers,
                    &parser,
                    &event_tx,
                );

                let error_count = errors
                    .iter()
                    .filter(|e| matches!(e.severity, Severity::Error))
                    .count();

                let is_valid = error_count == 0;

                // Cache the validation result.
                // Only cache as Valid when there are NO diagnostics at all
                // (including warnings). Warnings must be shown on every run
                // until the user fixes them, so warnings-only files must not
                // be cached as Valid (that would silently hide them).
                let validation_outcome = if errors.is_empty() {
                    CacheOutcome::Valid
                } else {
                    CacheOutcome::Invalid
                };
                if config.cache == CacheMode::Enabled
                    && let Some(cache_ref) = cache.as_ref()
                    && let Err(e) =
                        cache_ref.set(&file_path, config.check_alignment, validation_outcome)
                {
                    tracing::warn!(file = ?file_path, error = %e, "Failed to cache validation result");
                }

                let status = if is_valid {
                    // Validation passed. Run roundtrip if configured.
                    if config.roundtrip {
                        if let Some(ref cf) = chat_file {
                            run_roundtrip_and_emit(
                                cf, &parser, &file_path, &config, &cache, &event_tx, &stats,
                            )
                        } else {
                            FileStatus::Valid { cache_hit: false }
                        }
                    } else {
                        FileStatus::Valid { cache_hit: false }
                    }
                } else {
                    FileStatus::Invalid {
                        error_count,
                        cache_hit: false,
                    }
                };

                update_stats(&stats, &status);

                // Stream file completion
                if event_tx
                    .send(ValidationEvent::FileComplete(FileCompleteEvent {
                        path: file_path,
                        status,
                    }))
                    .is_err()
                {
                    break; // Receiver dropped (cancelled)
                }
            }
            Err(_) => break, // Work channel closed, no more files
        }
    }
}

/// Run roundtrip test and emit events. Returns the resulting FileStatus.
fn run_roundtrip_and_emit<C>(
    chat_file: &ChatFile,
    parser: &ParserDispatch,
    file_path: &Path,
    config: &ValidationConfig,
    cache: &Option<Arc<C>>,
    event_tx: &Sender<ValidationEvent>,
    stats: &Arc<ValidationStats>,
) -> FileStatus
where
    C: ValidationCache + Send + Sync,
{
    // Check roundtrip cache first
    if config.cache == CacheMode::Enabled
        && let Some(rt_outcome) = cache.as_ref().and_then(|c| {
            c.get_roundtrip(
                file_path,
                config.check_alignment,
                config.parser_kind.cache_label(),
            )
        })
    {
        let rt_passed = rt_outcome == CacheOutcome::Valid;
        // Emit roundtrip event
        let _ = event_tx.send(ValidationEvent::RoundtripComplete(RoundtripEvent {
            path: file_path.to_path_buf(),
            passed: rt_passed,
            failure_reason: if rt_passed {
                None
            } else {
                Some("Roundtrip failed (cached)".to_string())
            },
            diff: None,
        }));

        if rt_passed {
            stats.record_roundtrip_passed();
            return FileStatus::Valid { cache_hit: true };
        } else {
            stats.record_roundtrip_failed();
            return FileStatus::RoundtripFailed {
                cache_hit: true,
                reason: "Roundtrip failed (cached)".to_string(),
            };
        }
    }

    let result = roundtrip::run_roundtrip(chat_file, parser);

    // Cache the roundtrip result
    let roundtrip_outcome = if result.passed {
        CacheOutcome::Valid
    } else {
        CacheOutcome::Invalid
    };
    if config.cache == CacheMode::Enabled
        && let Some(cache_ref) = cache.as_ref()
        && let Err(e) = cache_ref.set_roundtrip(
            file_path,
            config.check_alignment,
            config.parser_kind.cache_label(),
            roundtrip_outcome,
        )
    {
        tracing::warn!(file = ?file_path, error = %e, "Failed to cache roundtrip result");
    }

    // Emit roundtrip event
    let _ = event_tx.send(ValidationEvent::RoundtripComplete(RoundtripEvent {
        path: file_path.to_path_buf(),
        passed: result.passed,
        failure_reason: result.failure_reason.clone(),
        diff: result.diff.clone(),
    }));

    if result.passed {
        stats.record_roundtrip_passed();
        FileStatus::Valid { cache_hit: false }
    } else {
        stats.record_roundtrip_failed();
        FileStatus::RoundtripFailed {
            cache_hit: false,
            reason: result
                .failure_reason
                .unwrap_or_else(|| "Roundtrip failed".to_string()),
        }
    }
}

/// Updates stats.
pub(super) fn update_stats(stats: &Arc<ValidationStats>, status: &FileStatus) {
    match status {
        FileStatus::Valid { cache_hit } => {
            stats.record_valid_file();
            if *cache_hit {
                stats.record_cache_hit();
            } else {
                stats.record_cache_miss();
            }
        }
        FileStatus::Invalid { cache_hit, .. } => {
            stats.record_invalid_file();
            if *cache_hit {
                stats.record_cache_hit();
            } else {
                stats.record_cache_miss();
            }
        }
        FileStatus::RoundtripFailed { cache_hit, .. } => {
            // Roundtrip failures count as invalid files
            stats.record_invalid_file();
            if *cache_hit {
                stats.record_cache_hit();
            } else {
                stats.record_cache_miss();
            }
        }
        FileStatus::ParseError { .. } => {
            stats.record_parse_error();
            stats.record_cache_miss();
        }
        FileStatus::ReadError { .. } => {
            stats.record_invalid_file();
            stats.record_cache_miss();
        }
    }
}

/// Validate a single file with streaming error output.
///
/// Returns the collected errors AND the parsed ChatFile (if parsing succeeded).
/// The ChatFile is needed for roundtrip testing.
fn validate_single_file_streaming(
    file_path: &Path,
    content: Arc<str>,
    check_alignment: bool,
    strict_linkers: bool,
    parser: &ParserDispatch,
    event_tx: &Sender<ValidationEvent>,
) -> (Vec<ParseError>, Option<ChatFile>) {
    // Collect all errors during validation (no streaming)
    let collector = talkbank_model::ErrorCollector::new();

    // Parse with error collection
    let mut chat_file = parser.parse_chat_file_streaming(content.as_ref(), &collector);

    // Validate with error collection, optionally enabling strict linker checks.
    if strict_linkers {
        let config = talkbank_model::ValidationConfig::new().with_strict_linkers();
        chat_file.validate_with_config(config, &collector, None);
    } else if check_alignment {
        chat_file.validate_with_alignment(&collector, None);
    } else {
        chat_file.validate(&collector, None);
    }

    let errors = collector.into_vec();

    // Send ONE batched error event if there are any errors
    if !errors.is_empty() {
        let _ = event_tx.send(ValidationEvent::Errors(ErrorEvent {
            path: file_path.to_path_buf(),
            errors: errors.clone(),
            source: content.clone(),
        }));
    }

    (errors, Some(chat_file))
}
