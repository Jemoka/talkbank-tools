//! Configuration types for spawning Python worker processes.

use crate::api::{LanguageCode3, MemoryMb, NumSpeakers, WorkerLanguage};
use crate::host_memory::HostMemoryRuntimeConfig;
use crate::revai::load_revai_api_key;
use crate::types::runtime::MemoryTier;
use crate::worker::python::resolve_python_executable;
use crate::worker::{InferTask, WorkerBootstrapMode, WorkerProfile, WorkerTarget};

/// Runtime-owned launch inputs for one worker subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRuntimeConfig {
    /// Whether the worker should force CPU-only model/device selection.
    pub force_cpu: bool,
    /// Optional Rev.AI key already resolved by the Rust control plane.
    pub revai_api_key: Option<String>,
    /// Maximum concurrent requests served inside one GPU worker process.
    pub gpu_thread_pool_size: u32,
    /// Host-memory coordination settings shared with the worker spawn path.
    pub host_memory: HostMemoryRuntimeConfig,
    /// Resolved memory tier used for startup reservations and other
    /// tier-derived worker-launch decisions.
    pub memory_tier: MemoryTier,
    /// Host-chosen bootstrap policy for local workers.
    pub bootstrap_mode: WorkerBootstrapMode,
    /// Unique identity for the current server instance when it owns spawned
    /// TCP daemons. `None` means spawned daemons should register as external.
    pub server_instance_id: Option<String>,
    /// PID of the current Rust server process when it owns spawned TCP daemons.
    /// `None` means there is no owning server process to record.
    pub server_process_id: Option<u32>,
}

impl Default for WorkerRuntimeConfig {
    fn default() -> Self {
        // `gpu_thread_pool_size` is the in-process default for legacy
        // callers (chiefly `PoolConfig::default()` and tests that
        // construct a `WorkerRuntimeConfig` without specifying every
        // field). Production builders override this via
        // `EffectiveConfig::resolve` against detected `HostFacts`, so
        // this constant only matters when nobody has consulted the
        // host-facts pipeline yet. `4` matches the pre-migration
        // static default; raising or lowering it here would silently
        // shift any test that relies on it.
        const LEGACY_DEFAULT_GPU_THREAD_POOL_SIZE: u32 = 4;
        Self::from_sources(
            false,
            load_revai_api_key()
                .ok()
                .map(|key| key.as_str().to_string()),
            LEGACY_DEFAULT_GPU_THREAD_POOL_SIZE,
            HostMemoryRuntimeConfig::default(),
            crate::config::ServerConfig::default().resolved_memory_tier(),
        )
    }
}

impl WorkerRuntimeConfig {
    /// Build worker runtime inputs from explicit sources.
    pub fn from_sources(
        force_cpu: bool,
        revai_api_key: Option<String>,
        gpu_thread_pool_size: u32,
        host_memory: HostMemoryRuntimeConfig,
        memory_tier: MemoryTier,
    ) -> Self {
        Self {
            force_cpu,
            revai_api_key: revai_api_key
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            gpu_thread_pool_size,
            host_memory,
            memory_tier,
            bootstrap_mode: WorkerBootstrapMode::Profile,
            server_instance_id: None,
            server_process_id: None,
        }
    }

    /// Override the host bootstrap mode for worker spawns built from this runtime.
    pub fn with_bootstrap_mode(mut self, bootstrap_mode: WorkerBootstrapMode) -> Self {
        self.bootstrap_mode = bootstrap_mode;
        self
    }
}

/// Configuration for spawning a worker.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Path to the Python executable (e.g. "python3", "/usr/bin/python3.14t").
    pub python_path: String,
    /// Worker profile describing which task group this worker owns.
    pub profile: WorkerProfile,
    /// Optional infer task for task-targeted bootstrap.
    pub task: Option<InferTask>,
    /// Worker-runtime language string.
    pub lang: WorkerLanguage,
    /// Number of speakers.
    pub num_speakers: NumSpeakers,
    /// Engine overrides as JSON string (empty = none).
    pub engine_overrides: String,
    /// Use test-echo mode (no ML models).
    pub test_echo: bool,
    /// Maximum seconds to wait for the worker to become ready.
    pub ready_timeout_s: u64,
    /// Verbosity level (0=warn, 1=info, 2=debug, 3+=trace).
    /// Forwarded to the Python worker via `--verbose N` to control its logging
    /// level, enabling end-to-end verbosity from a single CLI `-v` flag.
    pub verbose: u8,
    /// Runtime-owned launch inputs resolved before this spawn boundary.
    pub runtime: WorkerRuntimeConfig,
    /// Timeout override for audio-heavy tasks (ASR, FA, speaker).
    /// 0 = use built-in default (1800).
    pub audio_task_timeout_s: u64,
    /// Timeout override for lightweight analysis tasks (OpenSMILE, AVQI).
    /// 0 = use built-in default (120).
    pub analysis_task_timeout_s: u64,
    /// Test-only: artificial delay in milliseconds before each response.
    /// 0 = no delay. Only effective when `test_echo` is true.
    pub test_delay_ms: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            python_path: resolve_python_executable(),
            profile: WorkerProfile::Stanza,
            task: None,
            lang: WorkerLanguage::from(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            engine_overrides: String::new(),
            test_echo: false,
            ready_timeout_s: 300,
            verbose: 0,
            runtime: WorkerRuntimeConfig::default(),
            audio_task_timeout_s: 0,
            analysis_task_timeout_s: 0,
            test_delay_ms: 0,
        }
    }
}

impl WorkerConfig {
    /// Return the actual bootstrap target for this worker spawn.
    pub fn bootstrap_target(&self) -> WorkerTarget {
        match self.task {
            Some(task) => WorkerTarget::infer_task(task),
            None => WorkerTarget::profile(self.profile),
        }
    }

    /// Return the human-readable bootstrap label for logs and status.
    pub fn bootstrap_label(&self) -> String {
        self.bootstrap_target().label()
    }

    /// Resolve the startup reservation for this worker spawn using the
    /// runtime-owned memory tier rather than raw host auto-detection.
    pub fn startup_reservation_mb(&self) -> MemoryMb {
        self.profile
            .startup_reservation_mb_for_tier(&self.runtime.memory_tier)
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkerConfig, WorkerRuntimeConfig};
    use crate::host_memory::HostMemoryRuntimeConfig;
    use crate::types::runtime::MemoryTier;
    use crate::worker::{InferTask, WorkerProfile};

    #[test]
    fn startup_reservation_uses_runtime_memory_tier() {
        let runtime = WorkerRuntimeConfig::from_sources(
            false,
            None,
            4,
            HostMemoryRuntimeConfig::default(),
            MemoryTier::from_total_mb(16_000),
        );
        let config = WorkerConfig {
            profile: WorkerProfile::Stanza,
            runtime,
            ..Default::default()
        };
        assert_eq!(config.startup_reservation_mb().0, 3_000);
    }

    #[test]
    fn bootstrap_target_uses_task_when_present() {
        let config = WorkerConfig {
            profile: WorkerProfile::Stanza,
            task: Some(InferTask::Morphosyntax),
            ..Default::default()
        };
        assert_eq!(config.bootstrap_label(), "infer:morphosyntax");
    }
}
