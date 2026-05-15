//! Error classification and user-facing message translation.
//!
//! Raw system errors (e.g. "Broken pipe (os error 32)") must never surface
//! to end users. This module translates classified failures into messages
//! that help users understand what happened and what to do about it.

use crate::error::ServerError;
use crate::scheduling::FailureCategory;
use crate::worker::error::WorkerError;

/// Truncate a string to keep only the last `max_chars` characters.
///
/// Python tracebacks have the actual error at the END, so keeping the tail
/// is more useful than keeping the head.
fn truncate_tail(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    // Find a char boundary near the truncation point.
    let start = s.len() - max_chars;
    match s[start..].find('\n') {
        Some(offset) => &s[start + offset + 1..],
        None => &s[start..],
    }
}

/// Classify worker errors into control-plane failure categories.
pub(crate) fn classify_worker_error(error: &WorkerError) -> FailureCategory {
    match error {
        WorkerError::ReadyTimeout { .. } => FailureCategory::WorkerTimeout,
        WorkerError::HealthCheckFailed(_) => FailureCategory::WorkerTimeout,
        WorkerError::ProcessExited { .. } => FailureCategory::WorkerCrash,
        WorkerError::Protocol(message) if message.contains("timeout") => {
            FailureCategory::WorkerTimeout
        }
        WorkerError::Protocol(_) => FailureCategory::WorkerProtocol,
        WorkerError::Io(_) => FailureCategory::WorkerCrash,
        WorkerError::WorkerResponse(_) => FailureCategory::ProviderTransient,
        // Bootstrap-class worker errors are deterministic across retries:
        // a missing model file, a failed catalog download, or an
        // unsupported language will produce the same failure on every
        // attempt. The orchestrator must NOT retry these — historically,
        // 3-attempt retries of a deterministic Stanza catalog miss
        // produced multi-GB log explosions because each attempt dumped a
        // full Python traceback before the worker exited.
        WorkerError::Bootstrap(_) => FailureCategory::WorkerBootstrap,
        WorkerError::MemoryGuard(_) => FailureCategory::MemoryPressure,
        WorkerError::SpawnFailed(_)
        | WorkerError::ReadyParseFailed(_)
        | WorkerError::NoWorker { .. } => FailureCategory::System,
    }
}

/// Classify server-side orchestration errors into control-plane failure categories.
pub(crate) fn classify_server_error(error: &ServerError) -> FailureCategory {
    match error {
        ServerError::Worker(worker_error) => classify_worker_error(worker_error),
        ServerError::Validation(_) => FailureCategory::Validation,
        ServerError::MemoryPressure(_) => FailureCategory::MemoryPressure,
        ServerError::Io(_) => FailureCategory::System,
        ServerError::Database(_) | ServerError::Migration(_) | ServerError::Persistence(_) => {
            FailureCategory::System
        }
        ServerError::JobNotFound(_)
        | ServerError::JobConflict { .. }
        | ServerError::JobNotTerminal(_)
        | ServerError::FileNotFound(_)
        | ServerError::FileNotReady(_)
        | ServerError::UnknownCommand(_) => FailureCategory::System,
        // EmptyFaAudioSegment is an internal skip signal consumed before reaching
        // file-level error classification.  Treat as validation if it ever leaks.
        ServerError::EmptyFaAudioSegment { .. } => FailureCategory::Validation,
        // JobNotInLocalStore signals that an activity was dispatched to the
        // wrong server (shared-queue misconfiguration). It's a control-plane
        // topology bug, not a per-file validation or worker failure — the
        // closest fit is `System`, and the error message itself directs the
        // operator to check task-queue configuration.
        ServerError::JobNotInLocalStore(_) => FailureCategory::System,
    }
}

/// Whether a classified worker failure should be retried automatically.
pub(crate) fn is_retryable_worker_failure(category: FailureCategory) -> bool {
    matches!(
        category,
        FailureCategory::WorkerCrash
            | FailureCategory::WorkerTimeout
            | FailureCategory::ProviderTransient
    )
}

/// Translate a classified failure into a user-facing error message.
///
/// The returned message is what end users see in the dashboard. It must be
/// actionable and free of system internals (no "Broken pipe", no "os error
/// 32", no stack traces). The raw technical error is preserved in server
/// logs via `tracing` for developer debugging.
///
/// `command_label` is the human-readable command name (e.g. "Alignment",
/// "Morphosyntax"). `filename` is the file that failed.
pub(crate) fn user_facing_error(
    category: FailureCategory,
    command_label: &str,
    filename: &str,
    raw_error: &str,
) -> String {
    match category {
        FailureCategory::WorkerCrash => {
            // Include the raw error (which now contains worker stderr via
            // ProcessExited's Display impl) so users see the actual Python
            // traceback or OOM message, not a generic "contact administrator."
            let detail = truncate_tail(raw_error, 500);
            format!(
                "{command_label} failed for {filename}: the processing engine crashed.\n{detail}"
            )
        }
        FailureCategory::WorkerTimeout => format!(
            "{command_label} timed out for {filename}: the processing engine did not \
             respond in time. The file may be too large or the server may be overloaded. \
             Try restarting the job or processing fewer files at once."
        ),
        FailureCategory::WorkerProtocol => format!(
            "{command_label} failed for {filename}: communication error with the \
             processing engine. Try restarting the job."
        ),
        FailureCategory::WorkerBootstrap => {
            // Bootstrap-class errors are user-actionable: network failure,
            // disk full, missing auth token, etc. Surface the worker's typed
            // message verbatim — that's exactly the actionable hint the user
            // needs. Do NOT prepend "an internal error" framing; the worker
            // already produced the user-facing wording.
            let detail = truncate_tail(raw_error, 1000);
            format!("{command_label} failed for {filename}: {detail}")
        }
        FailureCategory::ProviderTransient => format!(
            "{command_label} failed for {filename}: the external service returned a \
             temporary error. Try restarting the job."
        ),
        FailureCategory::ProviderTerminal => format!(
            "{command_label} failed for {filename}: the external service rejected the \
             request. Check that your API keys are valid and the input is supported."
        ),
        FailureCategory::MemoryPressure => format!(
            "{command_label} was deferred for {filename}: the server does not have \
             enough free memory. Try again later or process fewer files at once."
        ),
        FailureCategory::InputMissing => format!(
            "{command_label} failed for {filename}: a required input file could not be \
             found. Check that all referenced media files exist."
        ),
        // Validation, ParseError, and System categories typically already have
        // well-formed messages from the validation/parse layer, so we pass
        // them through with light framing.
        FailureCategory::Validation | FailureCategory::ParseError => {
            // These messages are already user-facing (validation error codes, etc.)
            format!("{command_label} failed for {filename}: {raw_error}")
        }
        FailureCategory::Cancelled => format!("{command_label} was cancelled for {filename}."),
        FailureCategory::System => format!(
            "{command_label} failed for {filename} due to an internal error. \
             Try restarting the job. If the problem persists, please contact \
             your administrator."
        ),
    }
}
