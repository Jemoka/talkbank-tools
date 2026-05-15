//! File status tracking, progress reporting, and retry state management.
//!
//! Split into focused sub-modules:
//! - [`file_stage`] — canonical stage labels for file lifecycles
//! - [`event_sink`] — runner event sink trait and store-backed implementation
//! - [`tracker`] — per-file lifecycle helper and free state-mutation functions
//! - [`supervision`] — spawning/draining supervised file tasks and fallback cleanup

mod event_sink;
mod file_stage;
mod supervision;
pub(crate) mod tracker;

#[cfg(test)]
mod tests;

// Re-export everything at the same paths callers already use.
pub(crate) use event_sink::{RunnerEventSink, StoreRunnerEventSink};
pub(crate) use file_stage::FileStage;
pub(crate) use tracker::{FileRunTracker, FileTaskOutcome, ProgressSender, ProgressUpdate};

pub(crate) use supervision::{
    drain_supervised_file_tasks, force_terminal_file_states, spawn_progress_forwarder,
    spawn_supervised_file_task,
};
pub(crate) use tracker::set_file_progress;
