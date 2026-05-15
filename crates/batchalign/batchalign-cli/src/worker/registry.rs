//! Worker registry — discover pre-started TCP workers from `workers.json`.
//!
//! The registry file is the bridge between independently started worker daemons
//! and the Rust server. Python workers write their entries on startup (via
//! `_registry.py`); the server reads and health-checks them on startup and
//! periodically.
//!
//! Registry path: `~/.batchalign3/workers.json` (configurable via
//! [`ServerConfig::worker_registry_path`] or `BATCHALIGN_STATE_DIR`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::api::WorkerLanguage;
use crate::worker::tcp_handle::{TcpWorkerHandle, TcpWorkerInfo};
use crate::worker::{WorkerCapabilities, WorkerPid, WorkerProfile};

// ---------------------------------------------------------------------------
// Registry entry (JSON schema matches Python `WorkerRegistryEntry`)
// ---------------------------------------------------------------------------

/// How a registry worker relates to the current Rust server lifecycle.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryOwnership {
    /// Independently started worker daemon that may outlive any one server.
    #[default]
    External,
    /// TCP daemon spawned and owned by one Rust server instance.
    ServerOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscoveryDisposition {
    Accept,
    SkipForeignOwner,
    ReapStaleOwned,
}

/// One worker's entry in the `workers.json` registry file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Worker process ID.
    pub pid: u32,
    /// Bind address (usually `"127.0.0.1"`).
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Worker profile name (`"gpu"`, `"stanza"`, `"io"`).
    pub profile: String,
    /// 3-letter language code.
    pub lang: String,
    /// Engine overrides JSON string (empty = none).
    #[serde(default)]
    pub engine_overrides: String,
    /// Whether the worker is external/persistent or owned by one server instance.
    #[serde(default)]
    pub ownership: RegistryOwnership,
    /// Owning Rust server instance id for server-owned daemons.
    #[serde(default)]
    pub owner_server_instance_id: Option<String>,
    /// Owning Rust server PID for server-owned daemons.
    #[serde(default)]
    pub owner_server_pid: Option<u32>,
    /// ISO 8601 timestamp when the worker started.
    #[serde(default)]
    pub started_at: String,
}

impl RegistryEntry {
    /// Parse the profile string into a [`WorkerProfile`].
    pub fn worker_profile(&self) -> Option<WorkerProfile> {
        WorkerProfile::try_from_name(&self.profile)
    }

    fn owner_server_instance_id(&self) -> Option<&str> {
        self.owner_server_instance_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn is_server_owned(&self) -> bool {
        self.ownership == RegistryOwnership::ServerOwned
    }

    fn is_owned_by_server_instance(&self, server_instance_id: &str) -> bool {
        self.is_server_owned() && self.owner_server_instance_id() == Some(server_instance_id)
    }
}

/// A discovered worker that has been health-checked and is ready for use.
#[derive(Debug, Clone)]
pub struct DiscoveredWorker {
    /// Registry entry data.
    pub entry: RegistryEntry,
    /// Parsed worker profile.
    pub profile: WorkerProfile,
    /// Parsed worker-runtime language string.
    pub lang: WorkerLanguage,
}

/// Result of one registry scan.
#[derive(Debug, Clone, Default)]
pub struct RegistryDiscovery {
    /// Healthy registry workers that can be integrated into the pool.
    pub workers: Vec<DiscoveredWorker>,
    /// Capability snapshot probed from one discovered worker connection.
    pub detected_capabilities: Option<WorkerCapabilities>,
}

// ---------------------------------------------------------------------------
// Registry file I/O
// ---------------------------------------------------------------------------

/// Default registry file path: `~/.batchalign3/workers.json`.
pub fn default_registry_path() -> PathBuf {
    if let Ok(state_dir) = std::env::var("BATCHALIGN_STATE_DIR") {
        let state_dir = state_dir.trim();
        if !state_dir.is_empty() {
            return PathBuf::from(state_dir).join("workers.json");
        }
    }
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".batchalign3")
        .join("workers.json")
}

/// Read all entries from the registry file.
pub fn read_registry(path: &Path) -> Vec<RegistryEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(path = %path.display(), error = %e, "Failed to read worker registry");
            }
            return Vec::new();
        }
    };

    match serde_json::from_str::<Vec<RegistryEntry>>(&content) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Failed to parse worker registry");
            Vec::new()
        }
    }
}

/// Write entries back to the registry file (for removing stale entries).
fn write_registry(path: &Path, entries: &[RegistryEntry]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(entries)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) only checks process existence/permission.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

fn discovery_disposition(
    entry: &RegistryEntry,
    current_server_instance_id: &str,
) -> DiscoveryDisposition {
    if !entry.is_server_owned() {
        return DiscoveryDisposition::Accept;
    }

    let Some(owner_server_instance_id) = entry.owner_server_instance_id() else {
        return DiscoveryDisposition::ReapStaleOwned;
    };
    let Some(owner_server_pid) = entry.owner_server_pid else {
        return DiscoveryDisposition::ReapStaleOwned;
    };

    if owner_server_instance_id == current_server_instance_id {
        return DiscoveryDisposition::Accept;
    }

    if process_alive(owner_server_pid) {
        DiscoveryDisposition::SkipForeignOwner
    } else {
        DiscoveryDisposition::ReapStaleOwned
    }
}

fn should_shutdown_entry(entry: &RegistryEntry, current_server_instance_id: &str) -> bool {
    entry.is_owned_by_server_instance(current_server_instance_id)
}

fn terminate_registered_daemon(pid: u32, profile: &str) {
    #[cfg(unix)]
    {
        // SAFETY: sending SIGTERM to a known PID. If the process already
        // exited, `kill()` returns ESRCH which we ignore.
        let ret = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if ret == 0 {
            info!(pid, profile, "Sent SIGTERM to TCP daemon worker");
        } else {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ESRCH) {
                warn!(pid, profile, error = %err, "Failed to SIGTERM TCP daemon worker");
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
        info!(pid, profile, "Sent taskkill to TCP daemon worker");
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Remove a stale entry by PID (for crash cleanup).
pub fn remove_stale_entry(registry_path: &Path, pid: u32) -> bool {
    let entries = read_registry(registry_path);
    let before = entries.len();
    let remaining: Vec<RegistryEntry> = entries.into_iter().filter(|e| e.pid != pid).collect();
    if remaining.len() == before {
        return false;
    }
    if let Err(e) = write_registry(registry_path, &remaining) {
        warn!(error = %e, "Failed to write registry after stale removal");
    }
    true
}

/// Kill all TCP daemon workers owned by the current server instance and remove
/// their registry entries. External daemons are preserved.
pub fn kill_owned_daemons(registry_path: &Path, current_server_instance_id: &str) {
    let entries = read_registry(registry_path);
    if entries.is_empty() {
        return;
    }

    let mut killed = 0usize;
    let mut remaining = Vec::new();
    for entry in entries {
        if should_shutdown_entry(&entry, current_server_instance_id) {
            terminate_registered_daemon(entry.pid, &entry.profile);
            killed += 1;
        } else {
            remaining.push(entry);
        }
    }

    if let Err(e) = write_registry(registry_path, &remaining) {
        warn!(error = %e, "Failed to rewrite worker registry after shutdown");
    } else {
        info!(
            killed,
            remaining = remaining.len(),
            "Retired owned TCP daemon workers"
        );
    }
}

/// Discover pre-started workers from the registry file.
///
/// Reads `workers.json`, connects to each entry, runs a health check, and
/// removes stale entries (workers that crashed without cleanup). Returns
/// only healthy, connectable workers plus one capability snapshot probed on the
/// same TCP connection used during discovery.
pub async fn discover_workers(
    registry_path: &Path,
    audio_task_timeout_s: u64,
    analysis_task_timeout_s: u64,
    current_server_instance_id: &str,
) -> RegistryDiscovery {
    let entries = read_registry(registry_path);
    if entries.is_empty() {
        return RegistryDiscovery::default();
    }

    info!(
        count = entries.len(),
        path = %registry_path.display(),
        "Checking worker registry"
    );

    let mut discovered = Vec::new();
    let mut detected_capabilities = None;
    let mut stale_indices = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        match discovery_disposition(entry, current_server_instance_id) {
            DiscoveryDisposition::Accept => {}
            DiscoveryDisposition::SkipForeignOwner => {
                info!(
                    pid = entry.pid,
                    profile = %entry.profile,
                    owner_server_instance_id = ?entry.owner_server_instance_id(),
                    owner_server_pid = ?entry.owner_server_pid,
                    "Skipping registry worker owned by another live server"
                );
                continue;
            }
            DiscoveryDisposition::ReapStaleOwned => {
                warn!(
                    pid = entry.pid,
                    profile = %entry.profile,
                    owner_server_instance_id = ?entry.owner_server_instance_id(),
                    owner_server_pid = ?entry.owner_server_pid,
                    "Reaping stale server-owned registry worker"
                );
                terminate_registered_daemon(entry.pid, &entry.profile);
                stale_indices.push(i);
                continue;
            }
        }

        let Some(profile) = entry.worker_profile() else {
            warn!(
                profile = %entry.profile,
                pid = entry.pid,
                "Unknown worker profile in registry, skipping"
            );
            stale_indices.push(i);
            continue;
        };

        let lang = match WorkerLanguage::parse_untrusted(&entry.lang) {
            Ok(code) => code,
            Err(e) => {
                warn!(
                    lang = %entry.lang,
                    pid = entry.pid,
                    error = %e,
                    "Registry entry has invalid worker language, skipping"
                );
                stale_indices.push(i);
                continue;
            }
        };
        // Health-check connect uses TcpWorkerHandle (one-request-at-a-time),
        // not SharedGpuTcpWorker. The dispatch semaphore is unused on this
        // path, so any non-zero default suffices; the pool will rebuild with
        // the correct value at integration time (pool/discovery.rs).
        let info = TcpWorkerInfo {
            host: entry.host.clone(),
            port: entry.port,
            profile,
            lang: lang.clone(),
            engine_overrides: entry.engine_overrides.clone(),
            pid: WorkerPid(entry.pid),
            audio_task_timeout_s,
            analysis_task_timeout_s,
            // Placeholder per the comment above — the registry walker
            // does not own the host-facts pipeline; the discovery /
            // pool integration step replaces this with the real
            // `EffectiveConfig.gpu_thread_pool_size`. The literal
            // matches the legacy static default so any code path
            // that *does* read it before integration sees the same
            // value as before the host-facts migration.
            gpu_thread_pool_size: 4,
        };

        match TcpWorkerHandle::connect(info).await {
            Ok(mut handle) => {
                match handle.health_check().await {
                    Ok(_) => {
                        info!(
                            profile = %entry.profile,
                            lang = %entry.lang,
                            host = %entry.host,
                            port = entry.port,
                            pid = entry.pid,
                            "Discovered healthy TCP worker"
                        );
                        if detected_capabilities.is_none() {
                            match handle.capabilities().await {
                                Ok(caps) => {
                                    info!(
                                        infer_tasks = ?caps.infer_tasks,
                                        engine_versions = ?caps.engine_versions,
                                        "Detected worker capabilities during registry discovery"
                                    );
                                    detected_capabilities = Some(caps);
                                }
                                Err(e) => {
                                    warn!(
                                        host = %entry.host,
                                        port = entry.port,
                                        pid = entry.pid,
                                        error = %e,
                                        "Failed to detect capabilities from discovered TCP worker"
                                    );
                                }
                            }
                        }
                        discovered.push(DiscoveredWorker {
                            entry: entry.clone(),
                            profile,
                            lang: lang.clone(),
                        });
                    }
                    Err(e) => {
                        warn!(
                            host = %entry.host,
                            port = entry.port,
                            pid = entry.pid,
                            error = %e,
                            "TCP worker health check failed, marking stale"
                        );
                        if entry.is_server_owned() {
                            terminate_registered_daemon(entry.pid, &entry.profile);
                        }
                        stale_indices.push(i);
                    }
                }
                // Drop the handle — the pool will create its own connection.
                drop(handle);
            }
            Err(e) => {
                debug!(
                    host = %entry.host,
                    port = entry.port,
                    pid = entry.pid,
                    error = %e,
                    "Cannot connect to registered worker, marking stale"
                );
                if entry.is_server_owned() {
                    terminate_registered_daemon(entry.pid, &entry.profile);
                }
                stale_indices.push(i);
            }
        }
    }

    // Remove stale entries from the registry file.
    if !stale_indices.is_empty() {
        let remaining: Vec<RegistryEntry> = entries
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !stale_indices.contains(i))
            .map(|(_, e)| e)
            .collect();

        info!(
            removed = stale_indices.len(),
            remaining = remaining.len(),
            "Removed stale entries from worker registry"
        );

        if let Err(e) = write_registry(registry_path, &remaining) {
            warn!(error = %e, "Failed to update worker registry after stale removal");
        }
    }

    RegistryDiscovery {
        workers: discovered,
        detected_capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiscoveryDisposition, RegistryEntry, RegistryOwnership, discovery_disposition,
        should_shutdown_entry,
    };

    fn external_entry() -> RegistryEntry {
        RegistryEntry {
            pid: 10,
            host: "127.0.0.1".to_string(),
            port: 1234,
            profile: "stanza".to_string(),
            lang: "eng".to_string(),
            engine_overrides: String::new(),
            ownership: RegistryOwnership::External,
            owner_server_instance_id: None,
            owner_server_pid: None,
            started_at: String::new(),
        }
    }

    #[test]
    fn discovery_accepts_external_entry() {
        let entry = external_entry();
        assert_eq!(
            discovery_disposition(&entry, "current-server"),
            DiscoveryDisposition::Accept
        );
    }

    #[test]
    fn discovery_accepts_current_server_owned_entry() {
        let mut entry = external_entry();
        entry.ownership = RegistryOwnership::ServerOwned;
        entry.owner_server_instance_id = Some("current-server".to_string());
        entry.owner_server_pid = Some(std::process::id());

        assert_eq!(
            discovery_disposition(&entry, "current-server"),
            DiscoveryDisposition::Accept
        );
    }

    #[test]
    fn discovery_skips_foreign_live_server_owned_entry() {
        let mut entry = external_entry();
        entry.ownership = RegistryOwnership::ServerOwned;
        entry.owner_server_instance_id = Some("other-server".to_string());
        entry.owner_server_pid = Some(std::process::id());

        assert_eq!(
            discovery_disposition(&entry, "current-server"),
            DiscoveryDisposition::SkipForeignOwner
        );
    }

    #[test]
    fn shutdown_only_targets_current_server_owned_entries() {
        let mut owned = external_entry();
        owned.ownership = RegistryOwnership::ServerOwned;
        owned.owner_server_instance_id = Some("current-server".to_string());
        owned.owner_server_pid = Some(std::process::id());

        let mut foreign = external_entry();
        foreign.ownership = RegistryOwnership::ServerOwned;
        foreign.owner_server_instance_id = Some("other-server".to_string());
        foreign.owner_server_pid = Some(std::process::id());

        assert!(should_shutdown_entry(&owned, "current-server"));
        assert!(!should_shutdown_entry(&foreign, "current-server"));
        assert!(!should_shutdown_entry(&external_entry(), "current-server"));
    }
}
