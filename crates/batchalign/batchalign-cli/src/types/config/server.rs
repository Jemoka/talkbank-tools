//! `ServerConfig` struct and serde defaults.
//!
//! The main configuration type deserialized from `server.yaml`. All fields
//! have sensible defaults so an empty YAML file (or a missing file) produces
//! a working configuration. Warmup presets and the `FleetTarget` sub-struct
//! also live here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::{LanguageCode3, MemoryMb, TemporalTaskQueue};
use crate::host_facts::serde_helpers::zero_as_none;

/// Minimal warmup preset — morphotag only.
///
/// The CLI `--warmup minimal` expands to this list.
pub const WARMUP_PRESET_MINIMAL: &[&str] = &["morphotag"];

/// Full warmup preset — morphotag, align, transcribe.
///
/// The CLI `--warmup full` expands to this list. This is also the default
/// when no `--warmup` flag or `warmup_commands` config is given.
pub const WARMUP_PRESET_FULL: &[&str] = &["morphotag", "align", "transcribe"];

/// Configuration for the Batchalign processing server.
///
/// Deserialized from the runtime-owned `server.yaml`. All fields have sensible
/// defaults so an empty YAML file (or a missing file) produces a working
/// configuration.  The [`validate`](Self::validate) method clamps out-of-range
/// values and returns non-fatal warnings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Filesystem directories the server searches when resolving media files
    /// for transcribe/align.  Paths that do not exist at startup produce a
    /// validation warning but are not fatal.
    #[serde(default)]
    pub media_roots: Vec<batchalign_types::paths::ServerPath>,
    /// Named media directory mappings (e.g. `{"childes-data": "/nfs/childes"}`).
    /// Clients reference the key in `JobSubmission.media_mapping`; the server
    /// resolves it to the filesystem root.  Allows stable logical names even
    /// when mount paths change.
    #[serde(default)]
    pub media_mappings:
        BTreeMap<batchalign_types::paths::MediaMappingKey, batchalign_types::paths::ServerPath>,
    /// Server log verbosity level: 0=warn (default), 1=info, 2=debug.
    /// Controls the tracing filter when the server starts in daemon mode.
    /// Equivalent to the number of `-v` flags on the CLI.
    /// Set to 1 in fleet server.yaml for production observability.
    #[serde(default)]
    pub verbose: u8,
    /// 3-letter ISO language code used when the client omits `lang`.
    /// Defaults to `"eng"`.
    #[serde(default = "default_lang")]
    pub default_lang: LanguageCode3,
    /// Operator override for the maximum number of jobs processed in
    /// parallel. `None` (the canonical post-migration form) means
    /// "let `JobStore::new` auto-tune from the resolved memory tier
    /// plus CPU availability via
    /// `HostExecutionPolicy::auto_max_concurrent_jobs`": Phase B4 of
    /// the host-facts migration replaces that helper with
    /// `EffectiveConfig::max_concurrent_jobs`. The `zero_as_none`
    /// shim collapses pre-migration `max_concurrent_jobs: 0` to
    /// `None`, and `validate()` no longer clamps negative values
    /// because the field is now unsigned and the legacy sentinel
    /// has migrated to the type system.
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_concurrent_jobs: Option<u32>,
    /// TCP port for the HTTP server.  Must be 1..=65535; 0 is clamped to
    /// 8000 by `validate()`.  Default: 8000.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Bind address for the HTTP server.  Default: `"0.0.0.0"` (all interfaces).
    #[serde(default = "default_host")]
    pub host: String,
    /// Accepted for YAML backward compatibility (`backend: temporal` in old
    /// configs). Not read at runtime: backend selection is driven by
    /// [`Self::temporal_server_url`] via [`Self::temporal_backend`] in
    /// `resolve.rs`. Empty / `"none"` / `"local"` / `"disabled"` URL selects
    /// the in-process backend (`bootstrap_test_server_backend`); a real URL
    /// selects the Temporal backend (`bootstrap_temporal_server_backend`).
    /// Both backends are supported production paths.
    #[serde(default, rename = "backend", skip_serializing)]
    pub backend_compat: Option<String>,

    /// Operator override for the worker-side "skip GPU detection,
    /// force CPU-only inference" mode. `None` (the canonical post-
    /// migration form) means "let `EffectiveConfig::resolve` derive
    /// the value from `HostFacts`": today
    /// `recommend_force_cpu` returns `true` when the GPU is
    /// non-functional for batchalign (Apple Silicon MPS, no GPU,
    /// CUDA with `device_count == 0`) and `false` on functional
    /// CUDA hosts. The CLI `--force-cpu` switch is converted to
    /// `Some(true)` at the builder boundary; an operator can also
    /// set `force_cpu: false` in `server.yaml` to assert "use GPU
    /// even on a host where the recommendation would force CPU"
    /// (the validator may warn).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_cpu: Option<bool>,
    /// Operator override for the per-job worker-process cap. `None`
    /// (the canonical post-migration form) means "let
    /// `EffectiveConfig::max_workers_per_job(command)` derive the
    /// per-command value from `HostFacts`": today the recommendation
    /// formula matches the legacy `compute_job_workers` auto-tune
    /// logic. `Some(n)` applies uniformly across every command (the
    /// legacy single-knob shape). The `zero_as_none` shim collapses
    /// pre-migration `max_workers_per_job: 0` to `None`. Phase G2 of
    /// the host-facts migration removes the shim.
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_workers_per_job: Option<u32>,
    /// Number of days to retain completed/failed job metadata in SQLite
    /// before automatic purge.  Must be >= 1; values < 1 are clamped to 1
    /// by `validate()`.  Default: 7.
    #[serde(default = "default_job_ttl_days")]
    pub job_ttl_days: i32,
    /// Commands to pre-warm at startup (e.g. `["morphotag", "align"]`).
    /// When empty (default), the server uses the `full` preset
    /// (`morphotag`, `align`, `transcribe`), filtered by actual worker
    /// capabilities.  The CLI `--warmup` flag sets this field.
    #[serde(default = "default_warmup_commands")]
    pub warmup_commands: Vec<String>,
    /// Whether the CLI should auto-spawn a local daemon when no explicit
    /// `--server` is configured. Default: `true`.
    #[serde(default = "default_true")]
    pub auto_daemon: bool,
    /// Temporal service URL used when `backend: temporal`.
    #[serde(default = "default_temporal_server_url")]
    pub temporal_server_url: String,
    /// Temporal namespace used when `backend: temporal`.
    #[serde(default = "default_temporal_namespace")]
    pub temporal_namespace: String,
    /// Temporal task queue used when `backend: temporal`. **Must be unique
    /// per fleet machine** — each batchalign3 server owns a local
    /// `JobStore`, so a workflow's activities can only be executed by the
    /// server whose store persisted the job. The default derives from the
    /// system hostname (`batchalign3-{hostname}`); any override in
    /// `server.yaml` must preserve the per-host uniqueness invariant. See
    /// the `architecture/temporal-fleet-topology.md` book page.
    #[serde(default = "default_temporal_task_queue")]
    pub temporal_task_queue: TemporalTaskQueue,
    /// Activity heartbeat interval in seconds for the Temporal backend.
    #[serde(default = "default_temporal_heartbeat_s")]
    pub temporal_heartbeat_s: u64,
    /// Per-attempt activity timeout in seconds for the Temporal backend.
    #[serde(default = "default_temporal_activity_timeout_s")]
    pub temporal_activity_timeout_s: u64,
    /// Operator override for the host-memory headroom (MB) the
    /// coordinator keeps free after worker-start and job-execution
    /// reservations. `None` (the canonical post-migration form) means
    /// "let `resolved_memory_gate_mb` derive the value from
    /// `resolved_memory_tier().headroom_mb`": that's tier-aware and
    /// honors any `memory_tier` override. The `zero_as_none` shim
    /// collapses pre-migration `memory_gate_mb: 0` (the legacy
    /// "disable host-memory checks" sentinel and the value used by
    /// `--sequential` mode) to `None`; setting an explicit small
    /// value like `MemoryMb(1)` still expresses "effectively disabled
    /// without skipping resolution".
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub memory_gate_mb: Option<MemoryMb>,
    /// Seconds between worker health checks. 0 = use pool default (30).
    #[serde(default = "default_worker_health_interval_s")]
    pub worker_health_interval_s: u64,

    /// Maximum number of local worker/model startups allowed at once across all
    /// participating batchalign3 processes on the host. Default: 1.
    #[serde(default = "default_max_concurrent_worker_startups")]
    pub max_concurrent_worker_startups: u32,

    /// Operator override for the maximum Python worker processes per
    /// `(profile, lang, engine)` key. `None` (the canonical
    /// post-migration form) means "use the built-in pool default
    /// (DEFAULT_MAX_WORKERS_PER_KEY = 8)". `Some(n)` applies
    /// uniformly across every profile (the legacy single-knob shape);
    /// the host-facts pipeline carries this through
    /// `ConfigOverrides::max_workers_per_key_by_profile` (each of
    /// gpu/stanza/io receives the same `Some(n)`). The `zero_as_none`
    /// shim collapses pre-migration `max_workers_per_key: 0` to
    /// `None`. A future PR introducing a per-profile `ServerConfig`
    /// shape would let operators set differentiated values; until
    /// then, the recommendation's per-profile RAM-derived formula
    /// (gpu = ram/16GB, stanza = ram/12GB, io = 1) lives only in
    /// `EffectiveConfig::max_workers_per_key_by_profile`.
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_workers_per_key: Option<u32>,

    /// Operator override for the hard ceiling on total workers across
    /// all `(profile, lang, engine)` keys. `None` (the canonical
    /// post-migration form) means "let `EffectiveConfig::resolve`
    /// derive the value from `HostFacts`": today
    /// `recommend_max_total_workers` returns
    /// `clamp(ram_total_mb / 6GB, 2, 32)`, with a fallback to 4 when
    /// `ram_total_mb` is zero. The `zero_as_none` shim collapses
    /// legacy `max_total_workers: 0` from pre-migration `server.yaml`
    /// files to `None` so deployed configs continue to mean "auto".
    /// Phase G2 of the host-facts migration removes the shim once
    /// every fleet `server.yaml` has been re-rendered.
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_total_workers: Option<u32>,

    /// Seconds to wait for a Python worker to become ready after spawn.
    /// Default: 120.
    #[serde(default = "default_worker_ready_timeout_s")]
    pub worker_ready_timeout_s: u64,

    /// Maximum HTTP request body size in megabytes. Default: 100.
    #[serde(default = "default_max_body_bytes_mb")]
    pub max_body_bytes_mb: MemoryMb,

    /// Seconds to wait for host-memory reservations to fit before rejecting or
    /// deferring a job. Default: 120. 0 = reject immediately if no plan fits.
    #[serde(default = "default_memory_gate_timeout_s")]
    pub memory_gate_timeout_s: u64,

    /// Seconds between host-memory reservation polling checks. Default: 5.
    #[serde(default = "default_memory_gate_poll_s")]
    pub memory_gate_poll_s: u64,

    /// Low-memory warning threshold in MB. Default: 4096.
    #[serde(default = "default_memory_warning_mb")]
    pub memory_warning_mb: MemoryMb,

    /// Operator override for the GPU worker's in-process dispatch
    /// concurrency. `None` (the canonical post-migration form) means
    /// "let `EffectiveConfig::resolve` derive the value from
    /// `HostFacts`": today that yields `4` on CUDA-functional hosts
    /// and `1` on hosts where the GPU is non-functional (Apple Silicon
    /// MPS, no GPU). The `zero_as_none` shim collapses legacy
    /// `gpu_thread_pool_size: 0` from pre-migration `server.yaml`
    /// files to `None` so deployed configs continue to mean "auto".
    /// Phase G2 of the host-facts migration removes the shim once
    /// every fleet `server.yaml` has been re-rendered.
    #[serde(
        default,
        deserialize_with = "zero_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub gpu_thread_pool_size: Option<u32>,

    /// Seconds before a locally-dispatched file lease is considered orphaned.
    /// Default: 300.
    #[serde(default = "default_local_lease_ttl_s")]
    pub local_lease_ttl_s: u64,

    /// Timeout in seconds for audio-heavy worker tasks (ASR, FA, speaker).
    /// 0 = use built-in default (1800). Increase for very long recordings.
    #[serde(default)]
    pub audio_task_timeout_s: u64,

    /// Timeout in seconds for lightweight analysis tasks (OpenSMILE, AVQI).
    /// 0 = use built-in default (120).
    #[serde(default)]
    pub analysis_task_timeout_s: u64,

    /// Timeout in seconds for on-demand model loading via `ensure_task` IPC.
    /// 0 = use built-in default (120). Increase on slow networks where
    /// first-time model downloads (Stanza, Whisper) may take longer.
    #[serde(default)]
    pub ensure_task_timeout_s: u64,

    /// Path to the worker registry file for discovering pre-started TCP
    /// workers. Empty string (default) uses `~/.batchalign3/workers.json`.
    #[serde(default)]
    pub worker_registry_path: String,

    /// Override the auto-detected memory tier. When absent, the tier is
    /// detected from total system RAM. This overrides all tier-derived
    /// defaults (headroom, startup reservations, idle timeout, max workers)
    /// unless those fields are also explicitly set.
    #[serde(default)]
    pub memory_tier: Option<crate::types::runtime::MemoryTierKind>,

    /// Override GPU worker startup reservation (MB).
    ///
    /// `None` (or YAML `0`, via the `zero_as_none` shim) → use the
    /// resolved tier's default. `Some(value)` overrides unconditionally.
    /// The shim preserves wire-format compatibility with operators who
    /// have `gpu_startup_mb: 0` in `server.yaml` from before the type
    /// migration; the `0`-sentinel pattern is no longer expressible
    /// in-code (per `~/.claude/CLAUDE.md` "no sentinel values").
    #[serde(default, deserialize_with = "zero_as_none")]
    pub gpu_startup_mb: Option<MemoryMb>,

    /// Override Stanza worker startup reservation (MB). See
    /// [`Self::gpu_startup_mb`] for the `None`/`Some` semantics.
    #[serde(default, deserialize_with = "zero_as_none")]
    pub stanza_startup_mb: Option<MemoryMb>,

    /// Override IO worker startup reservation (MB). See
    /// [`Self::gpu_startup_mb`] for the `None`/`Some` semantics.
    #[serde(default, deserialize_with = "zero_as_none")]
    pub io_startup_mb: Option<MemoryMb>,

    /// Fleet server target for automatic remote execution routing.
    ///
    /// When present and non-empty, the local daemon auto-routes jobs:
    /// - Media already visible on the fleet server (NFS) → submit directly
    /// - Media only local → stage via rsync, execute remotely, copy back
    /// - Fleet server unreachable → fall back to local execution
    ///
    /// When absent or empty (the default for external `uv tool install`
    /// users), all jobs execute locally. No network probing, no SSH
    /// attempts, no fleet behavior.
    ///
    /// Deployed by Ansible to fleet machines only.
    #[serde(default)]
    pub fleet_target: Option<FleetTarget>,
}

/// Fleet server connection details.
///
/// Tells the local daemon how to reach the fleet's primary compute
/// server for automatic job routing. Deployed by Ansible — external
/// users never see this.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetTarget {
    /// HTTP URL of the batchalign server (e.g. `"http://net:8001"`).
    pub url: String,
    /// SSH host for rsync file transfer (e.g. `"net"`).
    pub ssh_host: String,
    /// SSH user for rsync file transfer (e.g. `"operator"`).
    pub ssh_user: String,
    /// Base directory on the remote host for staged job scratch space.
    pub scratch_base: String,
}

// ---------------------------------------------------------------------------
// Serde default functions
// ---------------------------------------------------------------------------

pub(crate) fn default_lang() -> LanguageCode3 {
    LanguageCode3::eng()
}

pub(crate) fn default_port() -> u16 {
    8000
}

pub(crate) fn default_host() -> String {
    "0.0.0.0".to_string()
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_temporal_server_url() -> String {
    // Default to empty so the server uses the local backend when
    // no temporal_server_url is set in server.yaml.
    String::new()
}

pub(crate) fn default_temporal_namespace() -> String {
    "default".to_string()
}

pub(crate) fn default_temporal_task_queue() -> TemporalTaskQueue {
    // Startup-only. Panic (not a shared `batchalign3-unknown` fallback) is
    // deliberate: multiple hosts without stable hostnames would otherwise
    // collapse into a shared queue and violate the per-host invariant.
    // The expect rationale is documented at the user-visible level (the
    // panic message itself names the operator override).
    #[allow(clippy::expect_used)]
    let hostname = sysinfo::System::host_name().expect(
        "sysinfo::System::host_name() returned None — set `temporal_task_queue` \
         explicitly in server.yaml to override.",
    );
    TemporalTaskQueue::from(format!("batchalign3-{hostname}"))
}

pub(crate) fn default_temporal_heartbeat_s() -> u64 {
    10
}

/// Minimum acceptable value for `temporal_heartbeat_s`. A value of 0 is
/// not a valid heartbeat interval; the validator treats it as "field
/// forgotten" and restores the default.
pub(crate) const MIN_TEMPORAL_HEARTBEAT_S: u64 = 1;

/// Maximum acceptable value for `temporal_heartbeat_s`. Temporal worker
/// liveness detection becomes lossy past about a minute; values above
/// this clamp down to the max so the worker stays observable.
pub(crate) const MAX_TEMPORAL_HEARTBEAT_S: u64 = 60;

pub(crate) fn default_temporal_activity_timeout_s() -> u64 {
    60 * 60 * 24
}

/// Minimum acceptable value for `temporal_activity_timeout_s`.
///
/// Values below this are clamped up to the default with a warning, on the
/// principle that a single-activity Temporal workflow shorter than the
/// natural duration of a real align/transcribe batch is structurally
/// guaranteed to cancel-cascade. 6 hours is well below any real overnight
/// job (typical operator workloads run 12+ hours) and well above the
/// cascade window observed in production (1 hour). The clamp is
/// defense-in-depth against a future deploy bug, hand-edit, or operator
/// override that drops the value back to a too-short number.
pub(crate) const MIN_TEMPORAL_ACTIVITY_TIMEOUT_S: u64 = 6 * 60 * 60;

pub(crate) fn default_warmup_commands() -> Vec<String> {
    WARMUP_PRESET_FULL
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

pub(crate) fn default_job_ttl_days() -> i32 {
    7
}

pub(crate) fn default_worker_health_interval_s() -> u64 {
    30
}

pub(crate) fn default_max_concurrent_worker_startups() -> u32 {
    1
}

pub(crate) fn default_worker_ready_timeout_s() -> u64 {
    300
}

pub(crate) fn default_max_body_bytes_mb() -> MemoryMb {
    // 500 files of CHAT text from large corpora (e.g. childes-eng-uk,
    // childes-other) routinely exceed 100 MB when the CLI ships content
    // (`paths_mode=false`). 512 MB is the observed ceiling for a
    // 500-file chunk plus headroom; `server.yaml`'s `max_body_bytes_mb`
    // still overrides if a deployment needs a different ceiling.
    MemoryMb(512)
}

pub(crate) fn default_memory_gate_timeout_s() -> u64 {
    120
}

pub(crate) fn default_memory_gate_poll_s() -> u64 {
    5
}

pub(crate) fn default_memory_warning_mb() -> MemoryMb {
    MemoryMb(4096)
}

pub(crate) fn default_local_lease_ttl_s() -> u64 {
    300
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            verbose: 0,
            media_roots: Vec::new(),
            media_mappings: BTreeMap::new(),
            default_lang: LanguageCode3::eng(),
            max_concurrent_jobs: None,
            port: 8000,
            host: "0.0.0.0".to_string(),
            backend_compat: None,
            force_cpu: None,
            max_workers_per_job: None,
            job_ttl_days: 7,
            warmup_commands: default_warmup_commands(),
            auto_daemon: true,
            temporal_server_url: default_temporal_server_url(),
            temporal_namespace: default_temporal_namespace(),
            temporal_task_queue: default_temporal_task_queue(),
            temporal_heartbeat_s: default_temporal_heartbeat_s(),
            temporal_activity_timeout_s: default_temporal_activity_timeout_s(),
            memory_gate_mb: None,
            worker_health_interval_s: default_worker_health_interval_s(),
            max_concurrent_worker_startups: default_max_concurrent_worker_startups(),
            max_workers_per_key: None,
            max_total_workers: None,
            worker_ready_timeout_s: default_worker_ready_timeout_s(),
            max_body_bytes_mb: default_max_body_bytes_mb(),
            memory_gate_timeout_s: default_memory_gate_timeout_s(),
            memory_gate_poll_s: default_memory_gate_poll_s(),
            memory_warning_mb: default_memory_warning_mb(),
            gpu_thread_pool_size: None,
            local_lease_ttl_s: default_local_lease_ttl_s(),
            audio_task_timeout_s: 0,
            analysis_task_timeout_s: 0,
            ensure_task_timeout_s: 0,
            worker_registry_path: String::new(),
            memory_tier: None,
            gpu_startup_mb: None,
            stanza_startup_mb: None,
            io_startup_mb: None,
            fleet_target: None,
        }
    }
}
