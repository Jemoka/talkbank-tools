//! Worker warmup and pre-scaling.
//!
//! `warmup()` pre-spawns TCP daemon workers at server startup so the first job
//! does not pay cold-start costs. `pre_scale()` / `pre_scale_with_overrides()`
//! eagerly spawn workers for a specific command/lang before file dispatch begins.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::api::{NumSpeakers, ReleasedCommand, WorkerLanguage};
use crate::worker::handle::WorkerConfig;
use crate::worker::tcp_handle::TcpWorkerInfo;
use crate::worker::{WorkerPid, WorkerTarget};
use tracing::{error, info, warn};

use super::{WarmupStatus, WorkerGroup, WorkerKey, WorkerPool, lock_recovered, shared_gpu};

impl WorkerPool {
    /// Pre-start workers for the given commands (warmup).
    ///
    /// Spawns **server-owned TCP daemon workers** that are detached at the OS
    /// level but still belong to the current Rust server instance. They can be
    /// reused across app/router rebuilds inside the same process, but routine
    /// server shutdown is responsible for retiring them.
    ///
    /// Each command spawns concurrently so independent models load in parallel.
    /// The caller is responsible for setting [`mark_warmup_complete()`] after
    /// this returns (or spawning this method in a background task).
    pub async fn warmup(&self, targets: &[crate::worker_setup::WarmupTarget]) {
        use crate::worker::handle::spawn_tcp_daemon;

        struct WarmupItem {
            target: WorkerTarget,
            lang: WorkerLanguage,
            engine_overrides: String,
        }

        let items: Vec<WarmupItem> = targets
            .iter()
            .filter_map(|target| {
                let profile = crate::worker::WorkerProfile::for_command(target.command);
                match profile {
                    Some(profile) => Some(WarmupItem {
                        target: WorkerTarget::profile(profile),
                        lang: target.lang.clone(),
                        engine_overrides: self.config.engine_overrides.clone(),
                    }),
                    None => {
                        warn!(command = %target.command, lang = %target.lang, "Skipping warmup for unknown command profile");
                        None
                    }
                }
            })
            .collect();

        let gpu_tcp_ref = self.gpu_tcp_workers.clone();
        let groups_ref = self.groups.clone();
        let worker_returned_ref = self.worker_returned.clone();
        let spawn_permits_ref = self.spawn_permits.clone();
        let mut set = tokio::task::JoinSet::new();
        for item in items {
            let config = self.config.clone();
            let gpu_tcp_ref = gpu_tcp_ref.clone();
            let groups_ref = groups_ref.clone();
            let worker_returned_ref = worker_returned_ref.clone();
            let spawn_permits_ref = spawn_permits_ref.clone();
            set.spawn(async move {
                let target_label = item.target.label();
                let wc = WorkerConfig {
                    python_path: config.python_path.clone(),
                    profile: item.target.profile_kind(),
                    task: item.target.task(),
                    lang: item.lang.clone(),
                    num_speakers: NumSpeakers(1),
                    engine_overrides: item.engine_overrides.clone(),
                    test_echo: config.test_echo,
                    ready_timeout_s: config.ready_timeout_s,
                    verbose: config.verbose,
                    runtime: config.runtime.clone(),
                    audio_task_timeout_s: config.audio_task_timeout_s,
                    analysis_task_timeout_s: config.analysis_task_timeout_s,
                    test_delay_ms: config.test_delay_ms,
                };

                // Spawn a detached TCP daemon. It registers in workers.json as
                // owned by this server instance.
                let (pid, port) = match spawn_tcp_daemon(&wc, 0).await {
                    Ok(result) => result,
                    Err(e) => {
                        error!(
                            target = %target_label,
                            lang = %item.lang,
                            error = %e,
                            "TCP daemon warmup failed"
                        );
                        return;
                    }
                };

                // Connect to the just-spawned daemon.
                let tcp_info = TcpWorkerInfo {
                    host: "127.0.0.1".to_string(),
                    port,
                    profile: item.target.profile_kind(),
                    lang: item.lang.clone(),
                    engine_overrides: item.engine_overrides.clone(),
                    pid: WorkerPid(pid),
                    audio_task_timeout_s: config.audio_task_timeout_s,
                    analysis_task_timeout_s: config.analysis_task_timeout_s,
                    gpu_thread_pool_size: config.runtime.gpu_thread_pool_size,
                };

                if item.target.is_concurrent() {
                    // GPU warmup — connect as SharedGpuTcpWorker.
                    match shared_gpu::SharedGpuTcpWorker::connect(tcp_info).await {
                        Ok(shared) => {
                            let key = (
                                item.target,
                                item.lang.clone(),
                                item.engine_overrides.clone(),
                            );
                            gpu_tcp_ref
                                .lock()
                                .await
                                .entry(key)
                                .or_insert_with(|| Arc::new(shared));
                            info!(
                                target = %target_label,
                                lang = %item.lang,
                                pid = pid,
                                port = port,
                                "GPU TCP worker warmed up (server-owned daemon)"
                            );
                        }
                        Err(e) => {
                            error!(
                                target = %target_label,
                                lang = %item.lang,
                                error = %e,
                                "Failed to connect to GPU TCP daemon after spawn"
                            );
                        }
                    }
                } else {
                    // Non-GPU warmup — connect as TcpWorkerHandle.
                    match crate::worker::tcp_handle::TcpWorkerHandle::connect(tcp_info).await {
                        Ok(handle) => {
                            let key: WorkerKey = (
                                item.target,
                                item.lang.clone(),
                                item.engine_overrides.clone(),
                            );
                            let profile = item.target.profile_kind();
                            let mut groups = lock_recovered(&groups_ref);
                            let group = groups
                                .entry(key)
                                .or_insert_with(|| {
                                    Arc::new(WorkerGroup::new(
                                        worker_returned_ref.clone(),
                                        spawn_permits_ref.clone(),
                                        profile,
                                    ))
                                })
                                .clone();
                            drop(groups);

                            // Acquire a global-cap permit before
                            // counting the warmed-up worker. If the
                            // pool is already full, log + skip; the
                            // daemon process is left running but not
                            // registered (a future discovery sweep
                            // can pick it up when capacity opens).
                            let Some(permit_guard) =
                                super::permit::SpawnPermitGuard::try_acquire_or_skip(
                                    &group.spawn_permits,
                                    || {
                                        warn!(
                                            target = %target_label,
                                            lang = %item.lang,
                                            "Skipping warmup integration: global cap reached"
                                        );
                                    },
                                )
                            else {
                                return;
                            };
                            lock_recovered(&group.tcp_workers).push_back(handle);
                            group.tcp_available.add_permits(1);
                            group.total.fetch_add(1, Ordering::Relaxed);
                            permit_guard.forget();
                            info!(
                                target = %target_label,
                                lang = %item.lang,
                                pid = pid,
                                port = port,
                                "TCP worker warmed up (server-owned daemon)"
                            );
                        }
                        Err(e) => {
                            error!(
                                target = %target_label,
                                lang = %item.lang,
                                error = %e,
                                "Failed to connect to TCP daemon after spawn"
                            );
                        }
                    }
                }
            });
        }

        // Wait for all concurrent warmup spawns.
        while set.join_next().await.is_some() {}

        if self.lazy_capabilities.get().is_none() {
            let probe_key = {
                let groups = lock_recovered(&self.groups);
                groups.iter().find_map(|(key, group)| {
                    (!lock_recovered(&group.tcp_workers).is_empty()).then(|| key.clone())
                })
            };

            if let Some((target, lang, engine_overrides)) = probe_key
                && let Some(mut probe_handle) =
                    self.try_checkout_tcp(&target, &lang, &engine_overrides)
            {
                match probe_handle.capabilities().await {
                    Ok(caps) => {
                        info!(
                            source = "warmup-tcp-worker",
                            infer_tasks = ?caps.infer_tasks,
                            engine_versions = ?caps.engine_versions,
                            "Recorded detected worker capabilities"
                        );
                        self.record_capabilities(caps);
                    }
                    Err(e) => {
                        warn!(
                            target = %target.label(),
                            lang = %lang,
                            error = %e,
                            "Failed to probe warmed TCP worker capabilities"
                        );
                    }
                }
                self.return_tcp_worker(probe_handle, &target, &lang, &engine_overrides);
            }
        }
    }

    /// Transition warmup state to `InProgress`.
    pub fn mark_warmup_started(&self) {
        self.warmup_status
            .store(WarmupStatus::InProgress.as_u8(), Ordering::Release);
    }

    /// Transition warmup state to `Complete`.
    pub fn mark_warmup_complete(&self) {
        self.warmup_status
            .store(WarmupStatus::Complete.as_u8(), Ordering::Release);
    }

    /// Current warmup lifecycle state.
    pub fn warmup_status(&self) -> WarmupStatus {
        WarmupStatus::from_u8(self.warmup_status.load(Ordering::Acquire))
    }

    /// Pre-scale workers for a given command/lang up to `target` count.
    ///
    /// Delegates to [`pre_scale_with_overrides`] using the pool's default
    /// engine overrides.
    pub async fn pre_scale(
        &self,
        command: ReleasedCommand,
        lang: impl Into<WorkerLanguage>,
        target: usize,
    ) {
        self.pre_scale_with_overrides(command, lang, target, &self.config.engine_overrides)
            .await;
    }

    /// Pre-scale workers with explicit engine overrides.
    ///
    /// Spawns workers eagerly so they're ready before file dispatch begins.
    /// The `engine_overrides` must match the overrides that dispatch will use
    /// (typically from the job's `CommonOptions`), otherwise the pre-scaled
    /// worker will have a different key than what dispatch looks up.
    ///
    /// **TCP worker shortcut:** If a TCP worker is already discovered from the
    /// registry for this profile/lang, pre-scale is a no-op — the worker is
    /// already running and ready. This eliminates the TOCTOU race, ready
    /// timeout, and cold-start delay that motivated pre-scale in the first
    /// place.
    ///
    /// For GPU-profile commands, pre-creates the `SharedGpuWorker` so all
    /// concurrent file dispatches hit the fast path (no spawn race).
    /// For non-GPU commands, uses `compare_exchange` on `total` for
    /// concurrent-safe slot claiming.
    pub async fn pre_scale_with_overrides(
        &self,
        command: ReleasedCommand,
        lang: impl Into<WorkerLanguage>,
        target: usize,
        engine_overrides: &str,
    ) {
        let lang = lang.into();
        let Some(worker_target) =
            WorkerTarget::for_command_with_mode(command, self.config.runtime.bootstrap_mode)
        else {
            warn!(command = %command, lang = %lang, "Skipping pre-scale for unknown command target");
            return;
        };
        let target = target.min(self.max_workers_per_key_for(worker_target.profile_kind()));

        // TCP worker shortcut: if a TCP worker already exists for this
        // profile/lang, skip spawning — the worker is already running.
        if worker_target.is_concurrent() {
            let tcp_key = (worker_target, lang.clone(), engine_overrides.to_owned());
            if matches!(worker_target, WorkerTarget::Profile(_))
                && self.gpu_tcp_workers.lock().await.contains_key(&tcp_key)
            {
                info!(
                    command = %command,
                    lang = %lang,
                    "GPU TCP worker already discovered, skipping pre-scale"
                );
                return;
            }
        } else {
            let key: WorkerKey = (worker_target, lang.clone(), engine_overrides.to_owned());
            let has_tcp = {
                let groups = lock_recovered(&self.groups);
                groups
                    .get(&key)
                    .is_some_and(|g| !lock_recovered(&g.tcp_workers).is_empty())
            };
            if has_tcp {
                info!(
                    command = %command,
                    lang = %lang,
                    profile = %worker_target.label(),
                    "TCP worker already discovered, skipping pre-scale"
                );
                return;
            }
        }

        // GPU workers use the shared concurrent worker map. Pre-creating the
        // worker here ensures it's ready before file dispatch begins, avoiding
        // the TOCTOU race in `get_or_create_gpu_worker` where multiple tasks
        // would each try to spawn their own worker process.
        if worker_target.is_concurrent() {
            match self
                .get_or_create_gpu_worker(&worker_target, &lang, engine_overrides)
                .await
            {
                Ok(_) => {
                    info!(
                        command = %command,
                        target = %worker_target.label(),
                        lang = %lang,
                        engine_overrides = %engine_overrides,
                        "GPU worker pre-scaled (ready for concurrent dispatch)"
                    );
                }
                Err(e) => {
                    warn!(
                        command = %command,
                        lang = %lang,
                        target = %worker_target.label(),
                        error = %e,
                        "GPU worker pre-scale failed"
                    );
                }
            }
            return;
        }

        let group = self.get_or_create_group(&worker_target, &lang, engine_overrides);

        loop {
            let current = group.total.load(Ordering::Relaxed);
            if current >= target {
                break;
            }

            match self
                .try_spawn_into_group(&group, &worker_target, &lang, engine_overrides)
                .await
            {
                Ok(true) => {}      // Keep going
                Ok(false) => break, // At capacity
                Err(e) => {
                    warn!(
                        target = %worker_target.label(),
                        lang = %lang,
                        current = group.total.load(Ordering::Relaxed),
                        target = target,
                        error = %e,
                        "Pre-scale spawn failed, stopping early"
                    );
                    break;
                }
            }
        }
    }
}
