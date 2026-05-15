//! Read-only status projections for the worker pool.
//!
//! These methods query pool state without mutating it. Sequential worker groups
//! use `lock_recovered()` (blocking `std::sync::Mutex`). GPU worker maps use
//! `tokio::sync::Mutex` and are accessed via `.lock().await` — the methods are
//! async so they always acquire the lock and never silently report stale data.
//!
//! The `std::sync::MutexGuard` from `lock_recovered()` is `!Send`, so it must
//! be scoped in a block that ends before any `.await` point. This ensures the
//! resulting future is `Send` (required by axum handlers and tokio tasks).

use std::sync::atomic::Ordering;

use crate::api::{ReleasedCommand, WorkerLanguage};
use crate::worker::WorkerTarget;

use super::{WorkerPool, lock_recovered};

impl WorkerPool {
    /// Check if there are idle workers for a given `(command, lang)` key.
    ///
    /// Used by the memory gate to skip the system memory check when reusable
    /// workers already exist -- those workers are already loaded, so no new
    /// memory allocation is needed. Checks both stdio and TCP workers.
    pub async fn has_idle_workers(
        &self,
        command: ReleasedCommand,
        lang: impl Into<WorkerLanguage>,
    ) -> bool {
        let lang = lang.into();
        let Some(target) =
            WorkerTarget::for_command_with_mode(command, self.config.runtime.bootstrap_mode)
        else {
            return false;
        };

        // GPU profile workers are always "available" (shared, concurrent).
        if target.is_concurrent() {
            // Check TCP GPU workers first.
            {
                let tcp_gpu_workers = self.gpu_tcp_workers.lock().await;
                if tcp_gpu_workers.keys().any(|(group_target, group_lang, _)| {
                    group_target == &target && group_lang == &lang
                }) {
                    return true;
                }
            }
            let gpu_workers = self.gpu_workers.lock().await;
            return gpu_workers.keys().any(|(group_target, group_lang, _)| {
                group_target == &target && group_lang == &lang
            });
        }

        // Sequential groups use std::sync::Mutex — scope the guard so it
        // does not live across any .await (MutexGuard is !Send).
        let groups = lock_recovered(&self.groups);
        groups.iter().any(|((group_target, group_lang, _), group)| {
            if *group_target != target || group_lang != &lang {
                return false;
            }
            !lock_recovered(&group.idle).is_empty()
                || !lock_recovered(&group.tcp_workers).is_empty()
        })
    }

    /// Number of active workers (total across all keys, including checked-out).
    ///
    /// Counts sequential group workers, shared GPU workers, and TCP workers.
    /// The sequential group count uses an atomic per-group total, so no lock
    /// is held across the subsequent GPU lock awaits.
    pub async fn worker_count(&self) -> usize {
        // Scope the std::sync::MutexGuard so it is dropped before .await.
        let groups_count: usize = {
            let groups = lock_recovered(&self.groups);
            groups
                .values()
                .map(|g| g.total.load(Ordering::Relaxed))
                .sum()
        };
        let gpu_count = self.gpu_workers.lock().await.len();
        let tcp_gpu_count = self.gpu_tcp_workers.lock().await.len();
        groups_count + gpu_count + tcp_gpu_count
    }

    /// Active worker keys: `["profile:stanza:eng (2 total, 1 idle)", ...]`.
    ///
    /// Includes both sequential group workers and shared GPU workers.
    pub async fn worker_keys(&self) -> Vec<String> {
        // Scope the std::sync::MutexGuard so it is dropped before .await.
        let mut keys: Vec<String> = {
            let groups = lock_recovered(&self.groups);
            groups
                .iter()
                .map(|((target, lang, engine_overrides), group)| {
                    let total = group.total.load(Ordering::Relaxed);
                    let idle = lock_recovered(&group.idle).len();
                    let suffix = if engine_overrides.is_empty() {
                        String::new()
                    } else {
                        format!(":{}", engine_overrides)
                    };
                    format!(
                        "{}:{lang}{suffix} ({total} total, {idle} idle)",
                        target.label()
                    )
                })
                .collect()
        };

        {
            let gpu_workers = self.gpu_workers.lock().await;
            for ((target, lang, engine_overrides), _worker) in gpu_workers.iter() {
                let suffix = if engine_overrides.is_empty() {
                    String::new()
                } else {
                    format!(":{}", engine_overrides)
                };
                keys.push(format!(
                    "{}:{lang}{suffix} (1 total, shared)",
                    target.label()
                ));
            }
        }

        {
            let tcp_gpu_workers = self.gpu_tcp_workers.lock().await;
            for ((target, lang, engine_overrides), _worker) in tcp_gpu_workers.iter() {
                let suffix = if engine_overrides.is_empty() {
                    String::new()
                } else {
                    format!(":{}", engine_overrides)
                };
                keys.push(format!(
                    "{}:{lang}{suffix} (1 total, tcp-shared)",
                    target.label()
                ));
            }
        }

        keys.sort();
        keys
    }

    /// Backward-compatible string summary derived from
    /// [`worker_summary_entries`]. Format:
    /// `["profile:stanza:eng:pid=1234:transport=stdio", ...]`.
    ///
    /// The `loaded_pipelines` health/websocket field is wire-typed
    /// as `Vec<String>` and consumed by the frontend in that form;
    /// this method preserves that contract. Internal callers
    /// (tests, future tooling) should prefer the typed version.
    pub async fn worker_summary(&self) -> Vec<String> {
        let mut s: Vec<String> = self
            .worker_summary_entries()
            .await
            .into_iter()
            .map(|e| e.to_string())
            .collect();
        s.sort();
        s
    }

    /// Structured per-worker summary. Source of truth for
    /// [`worker_summary`]. Reports idle sequential workers + shared
    /// GPU workers (stdio + TCP) + sequential TCP workers. Checked-out
    /// sequential workers are not listed; use `worker_count()` for
    /// full totals.
    pub async fn worker_summary_entries(&self) -> Vec<WorkerSummaryEntry> {
        let mut entries: Vec<WorkerSummaryEntry> = {
            let groups = lock_recovered(&self.groups);
            let mut v = Vec::new();
            for group in groups.values() {
                let idle = lock_recovered(&group.idle);
                for worker in idle.iter() {
                    v.push(WorkerSummaryEntry {
                        profile: worker.profile_label().to_string(),
                        lang: worker.lang().to_string(),
                        pid: worker.pid(),
                        transport: WorkerTransport::from_str(worker.transport()),
                        concurrent: false,
                    });
                }
            }
            v
        };

        {
            let gpu_workers = self.gpu_workers.lock().await;
            for ((_target, _lang, _engine_overrides), worker) in gpu_workers.iter() {
                entries.push(WorkerSummaryEntry {
                    profile: worker.profile_label(),
                    lang: worker.lang().to_string(),
                    pid: worker.pid(),
                    transport: WorkerTransport::Stdio,
                    concurrent: true,
                });
            }
        }

        {
            let tcp_gpu_workers = self.gpu_tcp_workers.lock().await;
            for ((target, lang, _engine_overrides), worker) in tcp_gpu_workers.iter() {
                entries.push(WorkerSummaryEntry {
                    profile: target.label().to_string().to_string(),
                    lang: lang.to_string(),
                    pid: worker.pid(),
                    transport: WorkerTransport::Tcp,
                    concurrent: true,
                });
            }
        }

        {
            let groups = lock_recovered(&self.groups);
            for group in groups.values() {
                let tcp = lock_recovered(&group.tcp_workers);
                for worker in tcp.iter() {
                    entries.push(WorkerSummaryEntry {
                        profile: worker.profile_label().to_string(),
                        lang: worker.lang().to_string(),
                        pid: worker.pid(),
                        transport: WorkerTransport::Tcp,
                        concurrent: false,
                    });
                }
            }
        }

        entries
    }
}

/// Transport channel for one worker process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerTransport {
    /// stdio JSON-lines IPC (the default for spawned worker children).
    Stdio,
    /// TCP socket (registry-discovered or shared-GPU concurrent transport).
    Tcp,
}

impl WorkerTransport {
    fn from_str(s: &str) -> Self {
        match s {
            "tcp" => Self::Tcp,
            _ => Self::Stdio,
        }
    }

    /// Wire-format token for the transport, matching the format
    /// the older `Vec<String>` summary embedded as `transport=...`.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Tcp => "tcp",
        }
    }
}

/// One row in the structured worker summary. Consumed by tests and
/// in-process tooling that previously parsed `pid=N` substrings out
/// of the string form returned by [`WorkerPool::worker_summary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSummaryEntry {
    /// Profile label (`profile:stanza`, `profile:gpu`, `profile:io`).
    pub profile: String,
    /// ISO 639-3 language code the worker was spawned for.
    pub lang: String,
    /// Worker process ID.
    pub pid: crate::worker::WorkerPid,
    /// IPC transport.
    pub transport: WorkerTransport,
    /// True for shared concurrent GPU workers; false for sequential pool workers.
    pub concurrent: bool,
}

impl std::fmt::Display for WorkerSummaryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:pid={}:transport={}",
            self.profile,
            self.lang,
            self.pid,
            self.transport.as_wire(),
        )?;
        if self.concurrent {
            write!(f, ":concurrent")?;
        }
        Ok(())
    }
}
