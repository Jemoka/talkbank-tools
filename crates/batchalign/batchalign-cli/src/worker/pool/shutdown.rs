//! Pool shutdown and drop cleanup.
//!
//! `shutdown()` drains all worker groups gracefully: idle workers are shut down
//! immediately, checked-out workers are warned (they will be killed when their
//! RAII guard drops), GPU workers use their concurrent shutdown path, and
//! server-owned TCP daemons are killed via SIGTERM.
//!
//! The `Drop` impl (Layer 2) catches test code and panic unwinds where the pool
//! goes out of scope without a graceful `shutdown()` call.

use std::sync::atomic::Ordering;

use tracing::{info, warn};

use super::{WorkerPool, lock_recovered};

impl WorkerPool {
    /// Shut down all workers gracefully.
    ///
    /// Idle workers are shut down immediately.  Checked-out workers (currently
    /// processing a request) are logged as warnings -- they'll be killed when
    /// the `CheckedOutWorker` RAII guard drops. Shared GPU workers are shut
    /// down via their concurrent shutdown path. TCP daemon workers owned by the
    /// current server instance are killed via SIGTERM and removed from the
    /// registry. External daemons are preserved.
    pub async fn shutdown(&self) {
        self.cancel.cancel();

        // Retire only the TCP daemon workers owned by this server instance.
        let registry_path = if self.config.worker_registry_path.is_empty() {
            super::super::registry::default_registry_path()
        } else {
            std::path::PathBuf::from(&self.config.worker_registry_path)
        };
        super::super::registry::kill_owned_daemons(
            &registry_path,
            self.current_server_instance_id(),
        );

        // Shut down shared GPU workers (stdio).
        {
            let mut gpu_workers = self.gpu_workers.lock().await;
            for ((target, lang, overrides), worker) in gpu_workers.drain() {
                info!(
                    target = %target.label(),
                    lang = %lang,
                    engine_overrides = %overrides,
                    pid = %worker.pid(),
                    "Shutting down GPU worker"
                );
                worker.shutdown().await;
            }
        }

        // Disconnect shared TCP GPU workers (does not kill the daemon).
        {
            let mut tcp_gpu_workers = self.gpu_tcp_workers.lock().await;
            for ((target, lang, overrides), worker) in tcp_gpu_workers.drain() {
                info!(
                    target = %target.label(),
                    lang = %lang,
                    engine_overrides = %overrides,
                    pid = %worker.pid(),
                    "Disconnecting TCP GPU worker"
                );
                worker.shutdown().await;
            }
        }

        let all_groups: Vec<(super::WorkerKey, std::sync::Arc<super::WorkerGroup>)> = {
            let mut groups = lock_recovered(&self.groups);
            groups.drain().collect()
        };

        for (key, group) in all_groups {
            let workers: Vec<crate::worker::handle::WorkerHandle> =
                { lock_recovered(&group.idle).drain(..).collect() };
            let idle_count = workers.len();
            let total = group.total.load(Ordering::Relaxed);
            let checked_out = total.saturating_sub(idle_count);

            if checked_out > 0 {
                warn!(
                    target = %key.0.label(),
                    lang = %key.1,
                    engine_overrides = %key.2,
                    checked_out,
                    "Workers still checked out during shutdown — \
                     they will be killed when their RAII guard drops"
                );
            }

            // Decrement total for drained workers and refund the
            // matching number of global-cap permits. (Checked-out
            // workers are not refunded here — their permits will
            // refund via CheckedOutWorker's drop/take paths once the
            // dispatch path returns. If shutdown races those returns,
            // the worst-case is a transient permit underflow that
            // resolves naturally.)
            group.total.fetch_sub(idle_count, Ordering::Relaxed);
            super::permit::SpawnPermitGuard::release_n(&group.spawn_permits, idle_count);

            for mut handle in workers {
                if let Err(e) = handle.shutdown_in_place().await {
                    warn!(
                        target = %key.0.label(),
                        lang = %key.1,
                        engine_overrides = %key.2,
                        error = %e,
                        "Error shutting down worker"
                    );
                }
            }
        }
    }
}

/// Layer 2: kill idle workers when the pool is dropped without calling
/// `shutdown()`. This catches test code and panic unwinds where the pool
/// goes out of scope without graceful shutdown.
///
/// GPU workers behind `tokio::sync::Mutex` cannot be locked outside a
/// runtime, but their shared-worker owners are dropped when Arc refcounts
/// hit zero. The stdio variant's `Drop` impl kills the worker process; the
/// TCP variant only disconnects from the daemon it does not own.
impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.cancel.cancel();

        // Drain all groups and kill workers synchronously.
        // This works even outside a tokio context.
        if let Ok(mut groups) = self.groups.lock() {
            for (_, group) in groups.drain() {
                if let Ok(mut idle) = group.idle.lock() {
                    for handle in idle.drain(..) {
                        // WorkerHandle::Drop sends SIGTERM+SIGKILL.
                        drop(handle);
                    }
                }
            }
        }
    }
}
