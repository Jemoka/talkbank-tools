//! Scheduling, retry, and work-unit domain types.
//!
//! These types describe the control-plane concepts that sit above individual
//! worker IPC messages:
//!
//! - what kind of unit is being scheduled
//! - how failures are classified
//! - whether a failure is retryable, deferrable, or terminal
//! - what retry policy should apply
//! - what happened on a concrete attempt
//!
//! The goal is to keep these concepts explicit and shared before fleet mode is
//! reintroduced, so the single-node server and future multi-node control plane
//! speak the same language.

use serde::{Deserialize, Serialize};

use crate::api::{JobId, NodeId, UnixTimestamp};
use crate::worker::WorkerPid;
pub use batchalign_types::scheduling::{AttemptId, WorkUnitId};

// `DurationMs` is defined in domain.rs alongside the other core numeric
// newtypes and re-exported here so that `crate::scheduling::DurationMs`
// paths continue to resolve unchanged.
pub use super::domain::DurationMs;

/// A schedulable unit of work within the control plane.
///
/// Today most work is effectively per-file, but the runner already has
/// different execution styles:
///
/// - per-file `process` dispatch
/// - per-file infer dispatch
/// - per-file forced alignment orchestration
/// - batched text inference
///
/// This enum makes those distinctions explicit so retry, leasing, and fleet
/// routing can target a concrete unit instead of relying on control-flow shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitKind {
    /// A pre-dispatch file validation or normalization unit.
    ///
    /// This is used when the control plane can reject a file before it chooses
    /// the concrete execution path (`file_process`, `file_infer`, or FA).
    FileSetup,
    /// A full per-file `process` request handled by a Python worker.
    FileProcess,
    /// A per-file infer-path orchestration handled by the Rust server.
    FileInfer,
    /// A per-file forced-alignment orchestration.
    FileForcedAlignment,
    /// A cross-file batched text inference unit.
    BatchInfer,
}

impl std::fmt::Display for WorkUnitKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileSetup => write!(f, "file_setup"),
            Self::FileProcess => write!(f, "file_process"),
            Self::FileInfer => write!(f, "file_infer"),
            Self::FileForcedAlignment => write!(f, "file_forced_alignment"),
            Self::BatchInfer => write!(f, "batch_infer"),
        }
    }
}

impl std::str::FromStr for WorkUnitKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file_setup" => Ok(Self::FileSetup),
            "file_process" => Ok(Self::FileProcess),
            "file_infer" => Ok(Self::FileInfer),
            "file_forced_alignment" => Ok(Self::FileForcedAlignment),
            "batch_infer" => Ok(Self::BatchInfer),
            other => Err(format!("unknown WorkUnitKind: {other}")),
        }
    }
}

/// Broad classification for failures seen by the control plane.
///
/// Retry behavior should be defined against these categories rather than raw
/// error strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    /// Input failed schema or command-level validation.
    Validation,
    /// CHAT parse or semantic validation failed.
    ParseError,
    /// Required input file or media could not be found or read.
    InputMissing,
    /// Worker process died unexpectedly.
    WorkerCrash,
    /// Worker or provider exceeded an expected time budget.
    WorkerTimeout,
    /// Worker IPC or protocol framing failed.
    WorkerProtocol,
    /// Worker reported a deterministic bootstrap-class failure (model load,
    /// catalog download, missing language pack, package import) that will
    /// recur identically across retries. Non-retryable; the user-facing
    /// message is the verbatim error from the worker (network unreachable,
    /// disk full, authentication required, etc. — all actionable). The
    /// historical reason this category exists: deterministic bootstrap
    /// failures classified as transient crashes produced multi-GB log
    /// explosions when retried 3× with a full traceback per attempt.
    WorkerBootstrap,
    /// Provider/backend returned a failure that may succeed on retry.
    ProviderTransient,
    /// Provider/backend returned a failure that should not be retried.
    ProviderTerminal,
    /// Scheduling was blocked by memory pressure.
    MemoryPressure,
    /// Work was cancelled intentionally.
    Cancelled,
    /// Catch-all infrastructure or system error.
    System,
}

impl std::fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation => write!(f, "validation"),
            Self::ParseError => write!(f, "parse_error"),
            Self::InputMissing => write!(f, "input_missing"),
            Self::WorkerCrash => write!(f, "worker_crash"),
            Self::WorkerTimeout => write!(f, "worker_timeout"),
            Self::WorkerProtocol => write!(f, "worker_protocol"),
            Self::WorkerBootstrap => write!(f, "worker_bootstrap"),
            Self::ProviderTransient => write!(f, "provider_transient"),
            Self::ProviderTerminal => write!(f, "provider_terminal"),
            Self::MemoryPressure => write!(f, "memory_pressure"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for FailureCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "validation" => Ok(Self::Validation),
            "parse_error" => Ok(Self::ParseError),
            "input_missing" => Ok(Self::InputMissing),
            "worker_crash" => Ok(Self::WorkerCrash),
            "worker_timeout" => Ok(Self::WorkerTimeout),
            "worker_protocol" => Ok(Self::WorkerProtocol),
            "worker_bootstrap" => Ok(Self::WorkerBootstrap),
            "provider_transient" => Ok(Self::ProviderTransient),
            "provider_terminal" => Ok(Self::ProviderTerminal),
            "memory_pressure" => Ok(Self::MemoryPressure),
            "cancelled" => Ok(Self::Cancelled),
            "system" => Ok(Self::System),
            other => Err(format!("unknown FailureCategory: {other}")),
        }
    }
}

/// Outcome of one concrete attempt to execute a work unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    /// The work unit finished successfully.
    Succeeded,
    /// The work unit failed and the failure is terminal.
    Failed,
    /// The attempt failed but the work unit should be retried later.
    RetryableFailure,
    /// The work unit was deferred without being treated as a failure.
    Deferred,
    /// The work unit was cancelled.
    Cancelled,
}

impl std::fmt::Display for AttemptOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::RetryableFailure => write!(f, "retryable_failure"),
            Self::Deferred => write!(f, "deferred"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for AttemptOutcome {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "retryable_failure" => Ok(Self::RetryableFailure),
            "deferred" => Ok(Self::Deferred),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown AttemptOutcome: {other}")),
        }
    }
}

/// Scheduler decision after an attempt completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RetryDisposition {
    /// Mark the work as terminally successful.
    Succeed,
    /// Mark the work as terminally failed.
    TerminalFailure,
    /// Retry the work after backoff.
    Retry,
    /// Leave the work queued for later without incrementing terminal failure.
    Defer,
}

impl std::fmt::Display for RetryDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Succeed => write!(f, "succeed"),
            Self::TerminalFailure => write!(f, "terminal_failure"),
            Self::Retry => write!(f, "retry"),
            Self::Defer => write!(f, "defer"),
        }
    }
}

impl std::str::FromStr for RetryDisposition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "succeed" => Ok(Self::Succeed),
            "terminal_failure" => Ok(Self::TerminalFailure),
            "retry" => Ok(Self::Retry),
            "defer" => Ok(Self::Defer),
            other => Err(format!("unknown RetryDisposition: {other}")),
        }
    }
}

/// Retry/backoff policy attached to a class of work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct RetryPolicy {
    /// Maximum total attempts, including the first attempt.
    pub max_attempts: u32,
    /// Initial backoff in milliseconds.
    pub initial_backoff_ms: DurationMs,
    /// Maximum backoff in milliseconds.
    pub max_backoff_ms: DurationMs,
    /// Exponential multiplier applied after each retry.
    pub backoff_multiplier: u32,
}

impl RetryPolicy {
    /// Conservative default suitable for transient worker/runtime failures.
    pub const fn conservative() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: DurationMs(1_000),
            max_backoff_ms: DurationMs(60_000),
            backoff_multiplier: 2,
        }
    }

    /// Compute the backoff delay for a 1-based retry number.
    pub fn backoff_for_retry(&self, retry_number: u32) -> DurationMs {
        if retry_number <= 1 {
            return DurationMs(self.initial_backoff_ms.0.min(self.max_backoff_ms.0));
        }

        let mut backoff = self.initial_backoff_ms.0;
        for _ in 1..retry_number {
            backoff = backoff.saturating_mul(self.backoff_multiplier as u64);
            if backoff >= self.max_backoff_ms.0 {
                return self.max_backoff_ms;
            }
        }
        DurationMs(backoff.min(self.max_backoff_ms.0))
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::conservative()
    }
}

/// Durable record of one attempt to execute a work unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AttemptRecord {
    /// Stable attempt identifier.
    pub attempt_id: AttemptId,
    /// Parent job identifier.
    pub job_id: JobId,
    /// Opaque work-unit identifier within the job.
    pub work_unit_id: WorkUnitId,
    /// Kind of work unit that was executed.
    pub work_unit_kind: WorkUnitKind,
    /// 1-based attempt number for this work unit.
    pub attempt_number: u32,
    /// Start timestamp, unix seconds with fractional precision.
    pub started_at: UnixTimestamp,
    /// Finish timestamp, unix seconds with fractional precision.
    pub finished_at: Option<UnixTimestamp>,
    /// Final outcome of the attempt.
    pub outcome: AttemptOutcome,
    /// Broad failure classification when the attempt did not succeed.
    pub failure_category: Option<FailureCategory>,
    /// Scheduler decision for what should happen next.
    pub disposition: RetryDisposition,
    /// Identifier of the node that executed the attempt.
    pub worker_node_id: Option<NodeId>,
    /// Worker process identifier, when execution happened in a local worker.
    pub worker_pid: Option<WorkerPid>,
}

/// Lease metadata for a claimed schedulable unit.
///
/// In single-node mode this is coarse-grained and attached at the job level:
/// the local dispatcher claims a queued job for a node, records the lease, and
/// later clears it when the runner exits. Fleet mode will extend the same shape
/// with real cross-node renewal and expiry handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct LeaseRecord {
    /// Identifier of the node that currently owns the lease.
    pub leased_by_node: NodeId,
    /// Unix timestamp when the lease was created or most recently renewed.
    pub heartbeat_at: UnixTimestamp,
    /// Unix timestamp when the lease should be considered expired if not renewed.
    pub expires_at: UnixTimestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_category_roundtrip() {
        for category in [
            FailureCategory::Validation,
            FailureCategory::ParseError,
            FailureCategory::InputMissing,
            FailureCategory::WorkerCrash,
            FailureCategory::WorkerTimeout,
            FailureCategory::WorkerProtocol,
            FailureCategory::ProviderTransient,
            FailureCategory::ProviderTerminal,
            FailureCategory::MemoryPressure,
            FailureCategory::Cancelled,
            FailureCategory::System,
        ] {
            let json = serde_json::to_string(&category).unwrap();
            let back: FailureCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(category, back);
            assert_eq!(category.to_string(), json.trim_matches('"'));
        }
    }

    #[test]
    fn scheduling_enums_display_and_parse() {
        for kind in [
            WorkUnitKind::FileSetup,
            WorkUnitKind::FileProcess,
            WorkUnitKind::FileInfer,
            WorkUnitKind::FileForcedAlignment,
            WorkUnitKind::BatchInfer,
        ] {
            let raw = kind.to_string();
            assert_eq!(raw.parse::<WorkUnitKind>().unwrap(), kind);
        }

        for outcome in [
            AttemptOutcome::Succeeded,
            AttemptOutcome::Failed,
            AttemptOutcome::RetryableFailure,
            AttemptOutcome::Deferred,
            AttemptOutcome::Cancelled,
        ] {
            let raw = outcome.to_string();
            assert_eq!(raw.parse::<AttemptOutcome>().unwrap(), outcome);
        }

        for disposition in [
            RetryDisposition::Succeed,
            RetryDisposition::TerminalFailure,
            RetryDisposition::Retry,
            RetryDisposition::Defer,
        ] {
            let raw = disposition.to_string();
            assert_eq!(raw.parse::<RetryDisposition>().unwrap(), disposition);
        }
    }

    #[test]
    fn retry_policy_default_is_conservative() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.initial_backoff_ms, 1_000u64);
        assert_eq!(policy.max_backoff_ms, 60_000u64);
        assert_eq!(policy.backoff_multiplier, 2);
        assert_eq!(policy.backoff_for_retry(1), 1_000u64);
        assert_eq!(policy.backoff_for_retry(2), 2_000u64);
        assert_eq!(policy.backoff_for_retry(3), 4_000u64);
    }
}
