//! Per-job async task — port of `JobStore._run_job` from Python.
//!
//! Each job runs as a `tokio::spawn` task. It acquires a semaphore permit,
//! then processes files concurrently (bounded by `compute_job_workers`) via
//! the WorkerPool.
//!
//! ## Sub-module layout
//!
//! - `context` — shared type definitions: execution contexts, host bundles,
//!   orchestration traits, and internal request/error enums.
//! - `execution` — job lifecycle entry points (`job_task`, `run_direct_job`,
//!   `run_server_job_attempt`) and the core `run_hosted_job` loop.
//! - `routing` — command dispatch routing: capability resolution, dispatch-
//!   family selection, and per-command dispatch wrappers.
//! - `dispatch/` — concrete dispatch family implementations (batched text,
//!   FA, transcribe, benchmark, compare, media-analysis).
//! - `policy` — command→infer-task mapping and result-filename rules.
//! - `util/` — progress tracking, event sinks, preflight helpers.
//! - `debug_dumper` — optional debug output capture.

mod context;
pub(crate) mod debug_dumper;
mod dispatch;
mod execution;
mod policy;
mod routing;
mod test_echo;
pub(crate) mod util;

// --- Re-exports for crate-internal consumers ---

// Types and traits from context
pub(crate) use context::{
    DirectExecutionHost, DispatchHostContext, ExecutionEngine, MemoryGateRejectionDisposition,
    QueuedJobOrchestrator, RunnerExecutionContext, ServerExecutionHost,
};

// Job lifecycle entry points from execution
pub(crate) use execution::{job_task, run_direct_job};

// Re-exports used only by tests
#[cfg(test)]
use execution::record_preflight_media_failures;
#[cfg(test)]
use policy::{command_requires_chat_infer, infer_task_for_command, result_filename_for_command};

#[cfg(test)]
mod tests;
