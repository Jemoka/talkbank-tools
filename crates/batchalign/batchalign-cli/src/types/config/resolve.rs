//! `ServerConfig` methods: validation, Temporal backend parsing, and
//! memory-tier resolution.
//!
//! These are `impl ServerConfig` blocks that depend on runtime types
//! (`MemoryTier`) and the default-value helpers from [`super::server`].

use super::server::*;

/// Parsed Temporal backend configuration.
///
/// Derived from `temporal_server_url` in `server.yaml`. Sentinel values
/// (`""`, `"none"`, `"local"`, `"disabled"`) all parse to `Disabled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalBackend {
    /// Local backend — no Temporal orchestration.
    Disabled,
    /// Temporal server at the given URL.
    Server {
        /// Temporal server URL (e.g. `"http://net:7233"`).
        url: String,
    },
}

impl ServerConfig {
    /// Parse the `temporal_server_url` field into a typed backend choice.
    pub fn temporal_backend(&self) -> TemporalBackend {
        let url = self.temporal_server_url.trim();
        if url.is_empty() || url == "none" || url == "local" || url == "disabled" {
            TemporalBackend::Disabled
        } else {
            TemporalBackend::Server {
                url: url.to_string(),
            }
        }
    }

    /// Whether the server should use the Temporal backend.
    pub fn use_temporal(&self) -> bool {
        matches!(self.temporal_backend(), TemporalBackend::Server { .. })
    }

    /// Resolve the effective memory tier, applying config overrides.
    ///
    /// Priority: explicit `memory_tier` field in config → auto-detect from RAM.
    /// Individual startup reservation overrides (`gpu_startup_mb`, etc.) are
    /// applied on top of the resolved tier.
    pub fn resolved_memory_tier(&self) -> crate::types::runtime::MemoryTier {
        use crate::types::runtime::{MemoryTier, MemoryTierKind};

        let mut tier = match self.memory_tier {
            Some(MemoryTierKind::Small) => MemoryTier::from_total_mb(16_000),
            Some(MemoryTierKind::Medium) => MemoryTier::from_total_mb(32_000),
            Some(MemoryTierKind::Large) => MemoryTier::from_total_mb(64_000),
            Some(MemoryTierKind::Fleet) => MemoryTier::from_total_mb(256_000),
            None => MemoryTier::detect(),
        };
        // Apply individual overrides. `None` falls through to the tier
        // default; `Some(value)` overrides unconditionally. The
        // `zero_as_none` deserializer collapses YAML `0` to `None`
        // for wire-format compat with the pre-migration sentinel.
        if let Some(value) = self.gpu_startup_mb {
            tier.gpu_startup_mb = value;
        }
        if let Some(value) = self.stanza_startup_mb {
            tier.stanza_startup_mb = value;
        }
        if let Some(value) = self.io_startup_mb {
            tier.io_startup_mb = value;
        }
        tier
    }

    /// Resolve host-memory headroom (MB).
    ///
    /// `Some(n)` is an explicit operator override (used today by
    /// `--sequential` mode to set `MemoryMb(1)`); `None` falls
    /// through to the hardcoded
    /// [`crate::worker::pool::memory_gate::MIN_FREE_MEMORY_MB`]
    /// floor, the same number the worker-pool admission gate
    /// enforces. The previous tier-derived fallback
    /// (`resolved_memory_tier().headroom_mb`, 2/4/8 GB by host RAM)
    /// has been retired — it tried to encode workload sizing into a
    /// floor that should only express OS-protection headroom.
    pub fn resolved_memory_gate_mb(&self) -> crate::api::MemoryMb {
        match self.memory_gate_mb {
            Some(value) => value,
            None => crate::api::MemoryMb(crate::worker::pool::memory_gate::MIN_FREE_MEMORY_MB),
        }
    }

    /// Resolve warmup commands before server-side capability filtering.
    ///
    /// Returns `warmup_commands` directly — the CLI `--warmup` flag and
    /// `server.yaml` both write to this field. An empty list means no warmup.
    pub fn resolved_warmup_commands(&self) -> &[String] {
        &self.warmup_commands
    }

    /// Return a list of warnings (non-fatal) about the config.
    pub fn validate(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        for root in &self.media_roots {
            if !root.as_path().is_dir() {
                warnings.push(format!("media_root does not exist: {root}"));
            }
        }
        for (key, root) in &self.media_mappings {
            if !root.as_path().is_dir() {
                warnings.push(format!("media_mapping '{key}' root does not exist: {root}"));
            }
        }
        // `max_concurrent_jobs` is `Option<u32>`: the legacy
        // negative-value clamp is no longer expressible, and `0 ->
        // None` happens at deserialize via the `zero_as_none` shim.
        if self.port == 0 {
            warnings.push(format!(
                "port must be 1-65535 (got {}), defaulting to 8000",
                self.port
            ));
            self.port = 8000;
        }
        if self.job_ttl_days < 1 {
            warnings.push(format!(
                "job_ttl_days must be >= 1 (got {}), defaulting to 1",
                self.job_ttl_days
            ));
            self.job_ttl_days = 1;
        }
        if self.memory_gate_poll_s == 0 {
            warnings.push("memory_gate_poll_s must be >= 1, defaulting to 1".into());
            self.memory_gate_poll_s = 1;
        }
        if self.max_concurrent_worker_startups == 0 {
            warnings.push("max_concurrent_worker_startups must be >= 1, defaulting to 1".into());
            self.max_concurrent_worker_startups = 1;
        }
        // `gpu_thread_pool_size` is `Option<u32>`: `0 -> None` via
        // the `zero_as_none` shim, so the validator has no
        // in-band sentinel left to clamp.
        // Empty temporal_server_url is valid — it means use the local backend.
        if self.temporal_namespace.trim().is_empty() {
            warnings.push("temporal_namespace must not be empty, defaulting to default".into());
            self.temporal_namespace = default_temporal_namespace();
        }
        // `Deserialize` rejects empty strings, but the infallible
        // `From<String>` on `validated_string_id!` types can still land an
        // empty value from tests or migrations — re-default defensively.
        if self.temporal_task_queue.trim().is_empty() {
            warnings.push(
                "temporal_task_queue must not be empty, defaulting to \
                 batchalign3-{hostname}"
                    .into(),
            );
            self.temporal_task_queue = default_temporal_task_queue();
        }
        if self.temporal_heartbeat_s < MIN_TEMPORAL_HEARTBEAT_S {
            warnings.push(format!(
                "temporal_heartbeat_s must be between {} and {}; was {}, restoring default {}",
                MIN_TEMPORAL_HEARTBEAT_S,
                MAX_TEMPORAL_HEARTBEAT_S,
                self.temporal_heartbeat_s,
                default_temporal_heartbeat_s(),
            ));
            self.temporal_heartbeat_s = default_temporal_heartbeat_s();
        } else if self.temporal_heartbeat_s > MAX_TEMPORAL_HEARTBEAT_S {
            warnings.push(format!(
                "temporal_heartbeat_s must be between {} and {}; was {}, clamping to max {}",
                MIN_TEMPORAL_HEARTBEAT_S,
                MAX_TEMPORAL_HEARTBEAT_S,
                self.temporal_heartbeat_s,
                MAX_TEMPORAL_HEARTBEAT_S,
            ));
            self.temporal_heartbeat_s = MAX_TEMPORAL_HEARTBEAT_S;
        }
        if self.temporal_activity_timeout_s < MIN_TEMPORAL_ACTIVITY_TIMEOUT_S {
            // Defense-in-depth against a recurrence of the 2026-04-28
            // cancel-cascade incident, in which a too-short
            // `temporal_activity_timeout_s` (1 hour) was hardcoded by the
            // pyinfra renderer and silently accepted here. See
            // `docs/postmortems/2026-04-28-temporal-activity-timeout-cancel-cascade.md`.
            warnings.push(format!(
                "temporal_activity_timeout_s must be >= {} ({}h) to span typical \
                 long-running align/transcribe batches without cancel-cascading; \
                 was {}, clamping to default {}",
                MIN_TEMPORAL_ACTIVITY_TIMEOUT_S,
                MIN_TEMPORAL_ACTIVITY_TIMEOUT_S / 3600,
                self.temporal_activity_timeout_s,
                default_temporal_activity_timeout_s(),
            ));
            self.temporal_activity_timeout_s = default_temporal_activity_timeout_s();
        }
        warnings
    }
}
