//! Worker checkout and dispatch routing.
//!
//! Contains the core `checkout()` loop (semaphore acquire → pop idle worker →
//! RAII guard), `dispatch_batch_infer`, `dispatch_execute_v2`, and TCP worker
//! checkout/return helpers. Routes GPU-profile tasks to shared concurrent
//! workers; non-GPU tasks use the traditional exclusive-checkout model.

use crate::api::{LanguageCode3, ReleasedCommand, WorkerLanguage};
use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2};
use crate::worker::error::WorkerError;
use crate::worker::tcp_handle::TcpWorkerHandle;
use crate::worker::{BatchInferRequest, BatchInferResponse, WorkerBootstrapMode, WorkerTarget};
use tracing::{info, warn};

use super::checkout::CheckedOutWorker;
use super::eviction::EvictionOutcome;
use super::execute_v2::{self, execute_v2_worker_key};
use super::job_tracker::TrackerGuard;
use super::{WorkerKey, WorkerPool, lock_recovered};

/// Build the typed error returned when a saturated checkout exhausts its
/// wait deadline without freeing a slot. Factored out so the three
/// dispatch sites (initial timeout, race-after-eviction, notify timeout)
/// produce one consistent message shape.
fn saturation_timeout_err(
    target: &WorkerTarget,
    lang: &WorkerLanguage,
    wait_secs: u64,
) -> WorkerError {
    WorkerError::SpawnFailed(format!(
        "no worker available for {target:?}/{lang} within {wait_secs}s — \
         pool saturated with no idle workers to evict"
    ))
}

impl WorkerPool {
    /// Check out an idle worker or spawn a new one.
    ///
    /// 1. Try to acquire a semaphore permit immediately.
    /// 2. If none available, try to spawn a new worker (if under capacity).
    /// 3. If at capacity, wait for a permit (async suspend).
    /// 4. Pop from the idle queue and wrap in `CheckedOutWorker` (RAII guard).
    pub(super) async fn checkout(
        &self,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) -> Result<CheckedOutWorker, WorkerError> {
        let group = self.get_or_create_group(target, lang, engine_overrides);

        // Deadline for the saturation branch (no workers for this key,
        // global cap reached, no idle worker to evict). Bounds how long
        // we park on `worker_returned` before returning a typed error
        // the orchestrator can surface as a per-file failure.
        let wait_deadline = tokio::time::Instant::now() + self.config.checkout_wait_timeout();

        loop {
            // Invariant (normal operation): `group.available.permits() ==
            // group.idle.len()`. When the invariant holds, a successful
            // `try_acquire` is matched by a non-empty idle queue and we
            // return immediately.
            //
            // Degenerate case: `permits > idle.len()` — a permit without a
            // matching worker. The health-check task drains idle before
            // re-adding permits for survivors, so this window is real.
            // Returning the permit and falling through is correct; the
            // `yield_now().await` is LOAD-BEARING. Without it, a tight
            // `try_acquire → pop-None → add_permits → continue` cycle
            // starves the tokio runtime — other tasks that might refill
            // idle never run, and one worker thread burns a core.
            if let Ok(permit) = group.available.try_acquire() {
                permit.forget();
                if let Some(handle) = lock_recovered(&group.idle).pop_front() {
                    return Ok(CheckedOutWorker {
                        handle: Some(handle),
                        group: group.clone(),
                    });
                }
                group.available.add_permits(1);
                tokio::task::yield_now().await;
            }

            // Slow path: try to spawn a new worker (if under cap).
            match self
                .try_spawn_into_group(&group, target, lang, engine_overrides)
                .await
            {
                Ok(true) => {
                    // Spawned — loop back: the new permit is backed by
                    // a real enqueued worker, so the fast path will hit.
                    continue;
                }
                Ok(false) => {
                    // At capacity. If this group already has live
                    // workers they will eventually return permits —
                    // fall through to the async wait below. Otherwise
                    // try to free a slot by evicting an idle worker
                    // from another group; if that fails, park on the
                    // pool-wide `worker_returned` Notify with a
                    // bounded deadline.
                    if group.is_empty() {
                        let key: WorkerKey = (*target, lang.clone(), engine_overrides.to_owned());

                        // Register on `worker_returned` BEFORE the
                        // eviction probe. `Notified::enable()` puts
                        // this task on the wait list without polling
                        // the future, so any `notify_one()` that
                        // fires during the probe is delivered here
                        // instead of being absorbed by the Notify's
                        // single-slot buffer (a burst of N>1 returns
                        // would otherwise lose N−1 wakeups, forcing
                        // late-comers to wait the full deadline).
                        // checkout.rs uses `notify_one()` (not
                        // `notify_waiters()`) so each return wakes
                        // exactly one waiter — the BUG-028 herd fix.
                        let notified = self.worker_returned.notified();
                        tokio::pin!(notified);
                        notified.as_mut().enable();

                        if let EvictionOutcome::Evicted = self.try_evict_idle_from_other_group(&key)
                        {
                            continue;
                        }

                        if tokio::time::timeout_at(wait_deadline, notified)
                            .await
                            .is_err()
                        {
                            return Err(saturation_timeout_err(
                                target,
                                lang,
                                self.config.checkout_wait_timeout().as_secs(),
                            ));
                        }
                        continue;
                    }
                    // fall through to this-key async wait below
                }
                Err(e) => return Err(e),
            }

            // All workers busy and at capacity. Wait asynchronously for
            // a permit, but bound the wait with the same checkout deadline
            // used for the zero-worker saturation path. A stale `total > 0`
            // count or a wedged checked-out worker must fail explicitly
            // instead of hanging the caller forever.
            let permit = tokio::time::timeout_at(wait_deadline, group.available.acquire())
                .await
                .map_err(|_| {
                    saturation_timeout_err(
                        target,
                        lang,
                        self.config.checkout_wait_timeout().as_secs(),
                    )
                })?
                .map_err(|_| WorkerError::SpawnFailed("worker pool semaphore closed".into()))?;
            permit.forget();

            if let Some(handle) = lock_recovered(&group.idle).pop_front() {
                return Ok(CheckedOutWorker {
                    handle: Some(handle),
                    group: group.clone(),
                });
            }
            // Rare: async-acquire returned a permit but idle is empty.
            // Same load-bearing yield as above — returning the permit
            // without yielding would reintroduce the runtime starvation.
            group.available.add_permits(1);
            tokio::task::yield_now().await;
        }
    }

    /// Dispatch a batch inference request to a single worker.
    ///
    /// Tries TCP workers first (from registry), then falls back to stdio
    /// workers. Checks out an idle worker (or spawns one), sends the batch
    /// infer request, and returns the response.
    pub async fn dispatch_batch_infer(
        &self,
        lang: &LanguageCode3,
        request: &BatchInferRequest,
    ) -> Result<BatchInferResponse, WorkerError> {
        let target =
            WorkerTarget::from_infer_task(request.task, self.config.runtime.bootstrap_mode);
        let engine_overrides = &self.config.engine_overrides;
        let worker_lang = WorkerLanguage::from(lang);

        // Try TCP worker first.
        if matches!(target, WorkerTarget::Profile(_))
            && let Some(mut tcp_handle) =
                self.try_checkout_tcp(&target, &worker_lang, engine_overrides)
        {
            let result = tcp_handle.batch_infer(request).await;
            self.return_tcp_worker(tcp_handle, &target, &worker_lang, engine_overrides);
            return result;
        }

        // Fall back to stdio worker. TrackerGuard registers this
        // worker against the current job for cancel-driven shutdown;
        // auto-unregisters on drop.
        let mut worker = self
            .checkout(&target, &worker_lang, engine_overrides)
            .await?;
        let _job_guard = TrackerGuard::new(&self.job_tracker, worker.pid());
        let result = worker.batch_infer(request).await;

        // If the worker crashed (I/O error or process exit), it is dead and
        // must not be returned to the idle queue.  Discard it via `take()` so
        // the pool decrements `total` (freeing a slot), then retry once with a
        // freshly spawned replacement.
        //
        // Without this, `Drop` would silently return the corpse to the idle
        // queue, causing the *next* dispatch to also fail with BrokenPipe.
        //
        // Protocol errors also warrant discarding the worker (the stdio stream
        // may be desynchronized), but we do NOT retry them — the framing break
        // may be input-specific and a retry could hang.
        match result {
            Err(ref e @ (WorkerError::Io(_) | WorkerError::ProcessExited { .. })) => {
                warn!(
                    error = %e,
                    "worker crashed during batch_infer — discarding and retrying with a fresh worker"
                );
                worker.take(); // decrement total, do NOT return to idle queue
                drop(worker); // Drop now sees None handle, does nothing
                let mut fresh = self
                    .checkout(&target, &worker_lang, engine_overrides)
                    .await?;
                fresh.batch_infer(request).await
            }
            Err(ref e @ WorkerError::Protocol(_)) => {
                // Desynchronized stream — discard without retry.
                warn!(error = %e, "worker protocol error — discarding worker");
                worker.take();
                result
            }
            other => other,
        }
    }

    /// Try to check out a TCP worker handle (non-blocking).
    pub(super) fn try_checkout_tcp(
        &self,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) -> Option<TcpWorkerHandle> {
        let key: WorkerKey = (*target, lang.clone(), engine_overrides.to_owned());
        let groups = lock_recovered(&self.groups);
        let group = groups.get(&key)?;
        match group.tcp_available.try_acquire() {
            Ok(permit) => {
                permit.forget();
                lock_recovered(&group.tcp_workers).pop_front()
            }
            Err(_) => None,
        }
    }

    /// Return a TCP worker handle to the pool.
    pub(super) fn return_tcp_worker(
        &self,
        handle: TcpWorkerHandle,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) {
        let key: WorkerKey = (*target, lang.clone(), engine_overrides.to_owned());
        let groups = lock_recovered(&self.groups);
        if let Some(group) = groups.get(&key) {
            lock_recovered(&group.tcp_workers).push_back(handle);
            group.tcp_available.add_permits(1);
        }
    }

    /// Dispatch one typed worker-protocol V2 execute request.
    ///
    /// GPU profile tasks are routed to a shared concurrent worker (multiple
    /// requests in flight to one process). Non-GPU tasks try TCP workers first,
    /// then fall back to the traditional exclusive checkout model.
    pub async fn dispatch_execute_v2(
        &self,
        lang: impl Into<WorkerLanguage>,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        self.dispatch_execute_v2_with_progress(lang, request, None)
            .await
    }

    /// Dispatch a V2 execute request, forwarding intermediate progress events
    /// through an optional async channel.
    pub async fn dispatch_execute_v2_with_progress(
        &self,
        lang: impl Into<WorkerLanguage>,
        request: &ExecuteRequestV2,
        progress_tx: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        let lang = lang.into();
        let (target, worker_lang, engine_overrides) = execute_v2_worker_key(
            lang,
            request,
            &self.config.engine_overrides,
            self.config.runtime.bootstrap_mode,
        )?;

        if target.is_concurrent() {
            // GPU workers don't support progress forwarding yet.
            return self
                .dispatch_gpu_execute_v2(&target, &worker_lang, &engine_overrides, request)
                .await;
        }

        // Try TCP worker first.
        if matches!(target, WorkerTarget::Profile(_))
            && let Some(mut tcp_handle) =
                self.try_checkout_tcp(&target, &worker_lang, &engine_overrides)
        {
            let result = tcp_handle
                .execute_v2_with_progress(request, progress_tx)
                .await;
            self.return_tcp_worker(tcp_handle, &target, &worker_lang, &engine_overrides);
            return result;
        }

        // Fall back to stdio worker.
        let mut worker = self
            .checkout(&target, &worker_lang, &engine_overrides)
            .await?;
        let _job_guard = TrackerGuard::new(&self.job_tracker, worker.pid());

        // In LazyProfile mode, ensure the task's models are loaded before
        // dispatching. The worker started with no models; ensure_task tells
        // it which engine to load (idempotent if already loaded).
        if self.config.runtime.bootstrap_mode == WorkerBootstrapMode::LazyProfile {
            let (task_name, overrides) = execute_v2::ensure_task_params(request)?;
            let timeout = self.config.effective_ensure_task_timeout_s();
            worker
                .ensure_task(&task_name, overrides.as_ref(), timeout)
                .await?;
        }

        worker.execute_v2_with_progress(request, progress_tx).await
    }

    /// Dispatch a V2 execute request to a GPU worker.
    ///
    /// Tries TCP workers first (discovered from registry), then falls back to
    /// stdio workers. For TCP workers, multiple callers share one worker via
    /// concurrent dispatch. For stdio workers, uses the existing
    /// `SharedGpuWorker` pattern.
    async fn dispatch_gpu_execute_v2(
        &self,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        // Try TCP worker first (discovered from registry).
        let tcp_key = (*target, lang.clone(), engine_overrides.to_owned());
        if matches!(target, WorkerTarget::Profile(_)) {
            let tcp_workers = self.gpu_tcp_workers.lock().await;
            if let Some(tcp_worker) = tcp_workers.get(&tcp_key) {
                let _job_guard = TrackerGuard::new(&self.job_tracker, tcp_worker.pid());
                return tcp_worker.execute_v2(request).await;
            }
        }

        // Fall back to stdio worker.
        let gpu_worker = self
            .get_or_create_gpu_worker(target, lang, engine_overrides)
            .await?;
        let _job_guard = TrackerGuard::new(&self.job_tracker, gpu_worker.pid());

        if self.config.runtime.bootstrap_mode == WorkerBootstrapMode::LazyProfile {
            let (task_name, overrides) = execute_v2::ensure_task_params(request)?;
            let timeout = self.config.effective_ensure_task_timeout_s();
            gpu_worker
                .ensure_task(&task_name, overrides.as_ref(), timeout)
                .await?;
        }

        gpu_worker.execute_v2(request).await
    }

    /// Ensure the pool has probed at least one real worker for the given command.
    ///
    /// Startup may only have an optimistic command list with no infer-task
    /// metadata yet. Execution paths that need authoritative infer-task data
    /// call this to force one real worker bootstrap/probe before gating.
    pub async fn ensure_command_capabilities_with_overrides(
        &self,
        command: ReleasedCommand,
        lang: impl Into<WorkerLanguage>,
        engine_overrides: &str,
    ) -> Result<(), WorkerError> {
        if self.config.test_echo || self.lazy_capabilities.get().is_some() {
            return Ok(());
        }

        let lang = lang.into();
        let Some(target) =
            WorkerTarget::for_command_with_mode(command, self.config.runtime.bootstrap_mode)
        else {
            return Ok(());
        };

        if target.is_concurrent() {
            let _ = self
                .get_or_create_gpu_worker(&target, &lang, engine_overrides)
                .await?;
            return Ok(());
        }

        if matches!(target, WorkerTarget::Profile(_))
            && let Some(mut tcp_handle) = self.try_checkout_tcp(&target, &lang, engine_overrides)
        {
            if self.lazy_capabilities.get().is_none() {
                let caps = tcp_handle.capabilities().await?;
                info!(
                    source = "checked-out-tcp-worker",
                    infer_tasks = ?caps.infer_tasks,
                    engine_versions = ?caps.engine_versions,
                    "Recorded detected worker capabilities"
                );
                self.record_capabilities(caps);
            }
            self.return_tcp_worker(tcp_handle, &target, &lang, engine_overrides);
            return Ok(());
        }

        let mut worker = self.checkout(&target, &lang, engine_overrides).await?;
        if self.lazy_capabilities.get().is_none() {
            self.detect_capabilities_from_worker(&mut worker).await?;
        }
        Ok(())
    }
}

// The pool-spin fix (see `checkout`) is a structural invariant: every
// degenerate `permits > idle.len()` branch must have a `.await` before
// looping, so co-tenant tokio tasks (health check, HTTP server, other
// checkouts) can make progress. Meaningful runtime coverage of this
// requires the full `WorkerPool` with test-echo workers — a unit test
// over Semaphore + VecDeque alone only exercises the tiny state
// machine, not the real dispatch code path. That broader coverage
// lives in the test-echo integration tests alongside `WorkerPool`.

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use crate::api::LanguageCode3;
    use crate::worker::{InferTask, WorkerTarget};

    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn checkout_times_out_when_group_claims_live_worker_but_no_permit_returns() {
        let pool = WorkerPool::new(super::super::PoolConfig {
            max_workers_per_key: crate::host_facts::PerProfile::uniform(1),
            max_total_workers: 1,
            checkout_wait_timeout_s: 1,
            ..Default::default()
        });
        let target = WorkerTarget::infer_task(InferTask::Morphosyntax);
        let lang = WorkerLanguage::from(LanguageCode3::eng());
        let group = pool.get_or_create_group(&target, &lang, "");

        // Simulate the wedged state seen in the live morphotag job:
        // the pool believes one worker exists for this key, but no idle
        // handle or semaphore permit can ever be returned.
        group.total.store(1, Ordering::Relaxed);

        let result =
            tokio::time::timeout(Duration::from_secs(2), pool.checkout(&target, &lang, ""))
                .await
                .expect("checkout should resolve via its own timeout, not hang forever");

        match result {
            Err(WorkerError::SpawnFailed(message)) => {
                assert!(
                    message.contains("no worker available"),
                    "expected saturation timeout error, got: {message}"
                );
            }
            Ok(_) => panic!("expected timeout-style spawn error, got successful checkout"),
            Err(other) => panic!("expected timeout-style spawn error, got {other}"),
        }
    }

    /// Admission control reads the per-profile cap from
    /// `PoolConfig::max_workers_per_key` based on the requesting
    /// group's `WorkerProfile`. Different profiles can have different
    /// caps; one profile saturating must not affect another.
    #[tokio::test(flavor = "current_thread")]
    async fn try_claim_spawn_slot_uses_per_profile_cap() {
        let pool = WorkerPool::new(super::super::PoolConfig {
            max_workers_per_key: crate::host_facts::PerProfile {
                gpu: 2,
                stanza: 4,
                io: 1,
            },
            max_total_workers: 64, // not the binding constraint here
            // Disable the CPU-loadavg gate so this test isolates
            // per-profile-cap behavior. CI runners are CPU-saturated
            // by parallel cargo-test workers and would otherwise
            // reject every claim with CpuSaturated.
            cpu_gate_threshold_override: Some(f64::INFINITY),
            ..Default::default()
        });
        let lang = WorkerLanguage::from(LanguageCode3::eng());

        // Stanza profile: cap 4. Filling group.total to 3 still admits;
        // 4 rejects.
        let stanza_target = WorkerTarget::infer_task(InferTask::Morphosyntax);
        assert_eq!(
            stanza_target.profile_kind(),
            crate::worker::WorkerProfile::Stanza
        );
        let stanza_group = pool.get_or_create_group(&stanza_target, &lang, "");
        stanza_group.total.store(3, Ordering::Relaxed);
        assert!(
            pool.try_claim_spawn_slot(&stanza_group).is_ok(),
            "stanza cap=4 must admit when current=3"
        );
        // The successful claim incremented total to 4; the next probe
        // must reject.
        assert!(
            pool.try_claim_spawn_slot(&stanza_group).is_err(),
            "stanza cap=4 must reject when current=4"
        );

        // GPU profile: cap 2 (lower). Independent group; not affected
        // by stanza saturation.
        let gpu_target = WorkerTarget::infer_task(InferTask::Asr);
        assert_eq!(gpu_target.profile_kind(), crate::worker::WorkerProfile::Gpu);
        let gpu_group = pool.get_or_create_group(&gpu_target, &lang, "");
        gpu_group.total.store(1, Ordering::Relaxed);
        assert!(
            pool.try_claim_spawn_slot(&gpu_group).is_ok(),
            "gpu cap=2 must admit when current=1 (independent of stanza saturation)"
        );
        assert!(
            pool.try_claim_spawn_slot(&gpu_group).is_err(),
            "gpu cap=2 must reject when current=2"
        );

        // IO profile: cap 1 (smallest). At cap, rejects.
        let io_target = WorkerTarget::infer_task(InferTask::Translate);
        assert_eq!(io_target.profile_kind(), crate::worker::WorkerProfile::Io);
        let io_group = pool.get_or_create_group(&io_target, &lang, "");
        io_group.total.store(1, Ordering::Relaxed);
        assert!(
            pool.try_claim_spawn_slot(&io_group).is_err(),
            "io cap=1 must reject when current=1"
        );
    }
}
