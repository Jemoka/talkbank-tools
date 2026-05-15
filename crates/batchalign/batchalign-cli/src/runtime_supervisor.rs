//! Explicit owner for background runtime tasks.
//!
//! The server has two long-lived categories of background work:
//!
//! - the queue dispatcher loop
//! - per-job runner tasks
//!
//! This module keeps those tasks behind one owned actor instead of exposing
//! shared `Mutex<JoinSet<_>>` and `Mutex<Option<JoinHandle<_>>>` fields across
//! the application state.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio::task::JoinSet;

/// Heap-allocated background task future accepted by the supervisor.
type BackgroundTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Outcome of a spawned job task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnedTaskOutcome {
    /// The task ran to completion on the main runtime.
    Completed,
    /// The supervisor was unavailable — the task was never spawned.
    NotSpawned,
    /// The task was spawned but the completion channel was dropped before
    /// it finished (supervisor shutdown or task abort).
    ChannelDropped,
}

/// Handle for tracking a spawned job task's completion.
///
/// Implements `Future<Output = SpawnedTaskOutcome>` so callers can `.await`
/// it directly. For fire-and-forget dispatch, use
/// [`RuntimeSupervisor::spawn_detached`] instead.
pub struct TaskCompletion(TaskCompletionInner);

enum TaskCompletionInner {
    /// Task was spawned; the receiver signals when it completes.
    Live(oneshot::Receiver<()>),
    /// Supervisor was unavailable; resolves immediately to `NotSpawned`.
    Failed,
}

impl Future for TaskCompletion {
    type Output = SpawnedTaskOutcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<SpawnedTaskOutcome> {
        match &mut self.0 {
            TaskCompletionInner::Failed => Poll::Ready(SpawnedTaskOutcome::NotSpawned),
            TaskCompletionInner::Live(rx) => match Pin::new(rx).poll(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(SpawnedTaskOutcome::Completed),
                Poll::Ready(Err(_)) => Poll::Ready(SpawnedTaskOutcome::ChannelDropped),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

/// Summary of the runtime supervisor shutdown sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownSummary {
    /// True when the shutdown wait hit its deadline before every job task
    /// finished naturally.
    pub timed_out: bool,
    /// Number of job tasks still present in the supervisor when the deadline
    /// expired.
    pub remaining_jobs: usize,
}

/// Error returned when the runtime supervisor cannot report shutdown status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShutdownError {
    /// The supervisor actor was already gone before the shutdown command could be sent.
    #[error("runtime supervisor unavailable before shutdown command could be sent")]
    Unavailable,
    /// The supervisor accepted shutdown but dropped the reply channel before responding.
    #[error("runtime supervisor dropped shutdown status before replying")]
    ReplyDropped,
}

/// Cloneable handle for the runtime supervisor actor.
///
/// Clones are cheap and all send commands into the same owned supervisor task.
#[derive(Clone)]
pub struct RuntimeSupervisor {
    commands: UnboundedSender<SupervisorCommand>,
}

/// Command sent to the runtime supervisor actor.
enum SupervisorCommand {
    /// Spawn one tracked per-job background task.
    SpawnJob {
        /// Future that owns the complete job lifecycle.
        task: BackgroundTask,
    },
    /// Stop the queue loop and wait for tracked jobs to finish.
    Shutdown {
        /// Maximum time to wait for job tasks before returning.
        timeout: Duration,
        /// Channel used to send the shutdown summary back to the caller.
        reply: oneshot::Sender<ShutdownSummary>,
    },
}

impl RuntimeSupervisor {
    /// Create and start a new runtime supervisor actor.
    pub fn new() -> Self {
        let (commands, receiver) = unbounded_channel();
        tokio::spawn(run_supervisor(receiver));
        Self { commands }
    }

    /// Spawn one tracked per-job background task, returning a
    /// [`TaskCompletion`] future that resolves when the task finishes.
    ///
    /// The completion future is the **only** reliable cross-runtime
    /// notification mechanism. The Temporal activity handler uses it instead
    /// of polling the `JobStore` from a separate tokio runtime where actor
    /// reply channels are unreliable.
    pub fn spawn_job<F>(&self, task: F) -> TaskCompletion
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let (done_tx, done_rx) = oneshot::channel();
        let wrapped = async move {
            task.await;
            let _ = done_tx.send(());
        };
        if self
            .commands
            .send(SupervisorCommand::SpawnJob {
                task: Box::pin(wrapped),
            })
            .is_err()
        {
            tracing::error!("RuntimeSupervisor channel closed — job task dropped silently");
            return TaskCompletion(TaskCompletionInner::Failed);
        }
        TaskCompletion(TaskCompletionInner::Live(done_rx))
    }

    /// Spawn a tracked background task without a completion signal.
    ///
    /// Same as [`spawn_job`](Self::spawn_job) but skips the oneshot channel
    /// allocation. Use this for fire-and-forget dispatch where the caller
    /// does not need to know when the task finishes.
    pub fn spawn_detached<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if self
            .commands
            .send(SupervisorCommand::SpawnJob {
                task: Box::pin(task),
            })
            .is_err()
        {
            tracing::error!("RuntimeSupervisor channel closed — job task dropped silently");
        }
    }

    /// Stop the queue task and wait for tracked jobs to finish.
    pub async fn shutdown(&self, timeout: Duration) -> Result<ShutdownSummary, ShutdownError> {
        let (reply, receiver) = oneshot::channel();
        if self
            .commands
            .send(SupervisorCommand::Shutdown { timeout, reply })
            .is_err()
        {
            return Err(ShutdownError::Unavailable);
        }

        receiver.await.map_err(|_| ShutdownError::ReplyDropped)
    }
}

/// Run the task-supervisor actor loop.
async fn run_supervisor(mut receiver: UnboundedReceiver<SupervisorCommand>) {
    let mut queue_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut job_tasks = JoinSet::new();

    while let Some(command) = receiver.recv().await {
        match command {
            SupervisorCommand::SpawnJob { task } => {
                tracing::debug!("runtime supervisor: spawning job task on main runtime JoinSet");
                job_tasks.spawn(task);
            }
            SupervisorCommand::Shutdown { timeout, reply } => {
                tracing::debug!("runtime supervisor: shutdown requested");
                if let Some(handle) = queue_task.take() {
                    handle.abort();
                }

                let timed_out = tokio::time::timeout(timeout, async {
                    while job_tasks.join_next().await.is_some() {}
                })
                .await
                .is_err();
                let remaining_jobs = job_tasks.len();
                let _ = reply.send(ShutdownSummary {
                    timed_out,
                    remaining_jobs,
                });
                break;
            }
        }
    }

    if let Some(handle) = queue_task.take() {
        handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    /// Shutdown waits for tracked job tasks when they complete before the deadline.
    #[tokio::test]
    async fn shutdown_waits_for_jobs() {
        let supervisor = RuntimeSupervisor::new();
        let completed = Arc::new(AtomicUsize::new(0));
        let completed_for_task = completed.clone();

        supervisor.spawn_detached(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            completed_for_task.store(1, Ordering::SeqCst);
        });

        let summary = supervisor
            .shutdown(Duration::from_secs(1))
            .await
            .expect("shutdown should succeed");

        assert!(!summary.timed_out);
        assert_eq!(summary.remaining_jobs, 0);
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    /// Shutdown reports timeout when a tracked job exceeds the deadline.
    #[tokio::test]
    async fn shutdown_reports_timed_out_jobs() {
        let supervisor = RuntimeSupervisor::new();

        supervisor.spawn_detached(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let summary = supervisor
            .shutdown(Duration::from_millis(10))
            .await
            .expect("shutdown should succeed");

        assert!(summary.timed_out);
        assert!(summary.remaining_jobs >= 1);
    }

    /// Shutdown reports an explicit error instead of fabricating a clean summary
    /// when the supervisor actor is already unavailable.
    #[tokio::test]
    async fn shutdown_reports_unavailable_supervisor() {
        let (commands, receiver) = unbounded_channel();
        drop(receiver);
        let supervisor = RuntimeSupervisor { commands };

        let error = supervisor
            .shutdown(Duration::from_secs(1))
            .await
            .expect_err("shutdown should fail when supervisor is unavailable");

        assert_eq!(error, ShutdownError::Unavailable);
    }

    /// Tasks dispatched from a separate OS thread with its own `current_thread`
    /// + `LocalSet` runtime (the exact pattern used by the Temporal worker)
    /// must execute on the main runtime where the supervisor actor lives.
    #[tokio::test]
    async fn spawn_job_from_separate_current_thread_runtime() {
        let supervisor = RuntimeSupervisor::new();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        let sup = supervisor.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("worker thread should build tokio runtime");
            let local = tokio::task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                sup.spawn_detached(async move {
                    completed_clone.store(true, Ordering::SeqCst);
                });
            }));
        })
        .join()
        .expect("worker thread should not panic");

        // Give the main runtime time to poll the spawned task.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            completed.load(Ordering::SeqCst),
            "job task dispatched from separate current_thread runtime should execute on main runtime"
        );
    }

    /// The completion signal fires when the spawned task finishes.
    #[tokio::test]
    async fn spawn_job_returns_completion_signal() {
        let supervisor = RuntimeSupervisor::new();
        let outcome = supervisor
            .spawn_job(async {
                tokio::time::sleep(Duration::from_millis(10)).await;
            })
            .await;
        assert_eq!(outcome, SpawnedTaskOutcome::Completed);
    }

    /// The completion signal crosses runtime boundaries: a receiver awaited
    /// on a separate `current_thread` runtime receives the outcome from the
    /// main runtime where the task actually executes.
    #[tokio::test]
    async fn completion_signal_crosses_runtime_boundary() {
        let supervisor = RuntimeSupervisor::new();
        let sup = supervisor.clone();

        let outcome = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("worker thread should build tokio runtime");
            let local = tokio::task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                sup.spawn_job(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                })
                .await
            }))
        })
        .await
        .expect("spawn_blocking should not panic");

        assert_eq!(outcome, SpawnedTaskOutcome::Completed);
    }

    /// When the supervisor is unavailable, `spawn_job` returns `NotSpawned`
    /// immediately instead of silently dropping the task.
    #[tokio::test]
    async fn spawn_job_reports_not_spawned_when_supervisor_gone() {
        let (commands, receiver) = unbounded_channel();
        drop(receiver);
        let supervisor = RuntimeSupervisor { commands };

        let outcome = supervisor.spawn_job(async {}).await;
        assert_eq!(outcome, SpawnedTaskOutcome::NotSpawned);
    }

    /// Shutdown reports an explicit error when the supervisor drops the reply
    /// channel before sending a summary.
    #[tokio::test]
    async fn shutdown_reports_dropped_reply() {
        let (commands, mut receiver) = unbounded_channel();
        tokio::spawn(async move {
            if let Some(SupervisorCommand::Shutdown { .. }) = receiver.recv().await {
                // Drop the reply without sending a summary.
            }
        });
        let supervisor = RuntimeSupervisor { commands };

        let error = supervisor
            .shutdown(Duration::from_secs(1))
            .await
            .expect_err("shutdown should fail when reply is dropped");

        assert_eq!(error, ShutdownError::ReplyDropped);
    }
}
