//! Bridge between `talkbank_transform::ValidationEvent` and Tauri frontend events.
//!
//! All structs use `#[serde(rename_all = "camelCase")]` so the JSON matches
//! the TypeScript types while Rust stays idiomatic snake_case.

use serde::Serialize;
use std::path::Path;
use talkbank_model::{ParseError, enhance_errors_with_source};
use talkbank_transform::validation_runner::{FileStatus, ValidationEvent, ValidationStatsSnapshot};
use talkbank_transform::{
    render_error_with_miette_with_source, render_error_with_miette_with_source_colored,
};

/// A parse diagnostic paired with its pre-rendered miette HTML.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendDiagnostic {
    pub error: ParseError,
    pub rendered_html: String,
    /// Plain text rendering (no ANSI codes) for clipboard copy.
    pub rendered_text: String,
}

/// Serializable event sent to the frontend via `app_handle.emit()`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrontendEvent {
    Discovering,

    #[serde(rename_all = "camelCase")]
    Started {
        total_files: usize,
    },

    Errors {
        file: String,
        diagnostics: Vec<FrontendDiagnostic>,
        source: String,
    },

    #[serde(rename_all = "camelCase")]
    FileComplete {
        file: String,
        status: FrontendFileStatus,
    },

    Finished {
        stats: FrontendStats,
    },
}

/// Serializable version of `FileStatus` for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrontendFileStatus {
    Valid {
        cache_hit: bool,
    },

    #[serde(rename_all = "camelCase")]
    Invalid {
        error_count: usize,
        cache_hit: bool,
    },

    #[serde(rename_all = "camelCase")]
    RoundtripFailed {
        cache_hit: bool,
        reason: String,
    },

    #[serde(rename_all = "camelCase")]
    ParseError {
        message: String,
    },

    #[serde(rename_all = "camelCase")]
    ReadError {
        message: String,
    },
}

/// Serializable version of `ValidationStatsSnapshot` for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendStats {
    pub total_files: usize,
    pub valid_files: usize,
    pub invalid_files: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub parse_errors: usize,
    pub roundtrip_passed: usize,
    pub roundtrip_failed: usize,
    pub cancelled: bool,
}

/// Convert a workspace `ValidationEvent` into a `FrontendEvent`.
pub fn to_frontend_event(event: ValidationEvent, root: &Path) -> Option<FrontendEvent> {
    match event {
        ValidationEvent::Discovering => Some(FrontendEvent::Discovering),

        ValidationEvent::Started { total_files } => Some(FrontendEvent::Started { total_files }),

        ValidationEvent::Errors(e) => {
            let file_path = path_string(&e.path, root);
            let source = e.source.to_string();

            // Pre-render each error using miette (same output as CLI)
            let mut enhanced = e.errors.clone();
            enhance_errors_with_source(&mut enhanced, &source);

            let diagnostics = e
                .errors
                .into_iter()
                .zip(enhanced)
                .map(|(error, enhanced_error)| {
                    let raw = render_error_with_miette_with_source_colored(
                        &enhanced_error,
                        &file_path,
                        &source,
                    );
                    let rendered_text =
                        render_error_with_miette_with_source(&enhanced_error, &file_path, &source);
                    FrontendDiagnostic {
                        error,
                        rendered_html: ansi_to_html(&raw),
                        rendered_text,
                    }
                })
                .collect();

            Some(FrontendEvent::Errors {
                file: file_path,
                diagnostics,
                source,
            })
        }

        ValidationEvent::FileComplete(e) => Some(FrontendEvent::FileComplete {
            file: path_string(&e.path, root),
            status: convert_status(e.status),
        }),

        // Roundtrip events are not shown in the desktop UI (Phase 1)
        ValidationEvent::RoundtripComplete(_) => None,

        ValidationEvent::Finished(stats) => Some(FrontendEvent::Finished {
            stats: convert_stats(stats),
        }),
    }
}

fn convert_status(status: FileStatus) -> FrontendFileStatus {
    match status {
        FileStatus::Valid { cache_hit } => FrontendFileStatus::Valid { cache_hit },
        FileStatus::Invalid {
            error_count,
            cache_hit,
        } => FrontendFileStatus::Invalid {
            error_count,
            cache_hit,
        },
        FileStatus::RoundtripFailed { cache_hit, reason } => {
            FrontendFileStatus::RoundtripFailed { cache_hit, reason }
        }
        FileStatus::ParseError { message } => FrontendFileStatus::ParseError { message },
        FileStatus::ReadError { message } => FrontendFileStatus::ReadError { message },
    }
}

fn convert_stats(s: ValidationStatsSnapshot) -> FrontendStats {
    FrontendStats {
        total_files: s.total_files,
        valid_files: s.valid_files,
        invalid_files: s.invalid_files,
        cache_hits: s.cache_hits,
        cache_misses: s.cache_misses,
        parse_errors: s.parse_errors,
        roundtrip_passed: s.roundtrip_passed,
        roundtrip_failed: s.roundtrip_failed,
        cancelled: s.cancelled,
    }
}

/// Convert a path to a string, using the absolute path.
fn path_string(path: &Path, _root: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Convert ANSI escape codes to HTML `<span>` elements using the `ansi-to-html` crate.
fn ansi_to_html(input: &str) -> String {
    ansi_to_html::convert(input).unwrap_or_else(|_| html_escape(input))
}

/// Fallback HTML escaping if ANSI conversion fails.
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
