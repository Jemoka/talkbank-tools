//! PID file reaper — garbage-collects orphaned Python worker processes.
//!
//! # Problem
//!
//! Worker processes are spawned with `setpgid(0, 0)` (their own process group)
//! so they survive the death of the parent Rust server. If the server is killed
//! (SIGKILL, OOM, test runner exit), `Drop` impls never fire and workers
//! accumulate indefinitely, consuming RAM.
//!
//! # Design
//!
//! Three atomic operations, no locking:
//!
//! 1. **On spawn:** Write `~/.batchalign3/worker-pids/{worker_pid}` containing
//!    the server PID and a timestamp.
//! 2. **On worker shutdown/drop:** Remove the PID file.
//! 3. **On pool startup:** Scan the directory. For each file where the worker
//!    is alive but the recorded server is dead → orphan → SIGTERM + SIGKILL.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tracing::{debug, info, warn};

/// Directory name under `~/.batchalign3/` for PID files.
const PID_DIR_NAME: &str = "worker-pids";

/// Return the PID file directory: `~/.batchalign3/worker-pids/`.
fn pid_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".batchalign3").join(PID_DIR_NAME))
}

/// Ensure the PID file directory exists.
fn ensure_pid_dir() -> Option<PathBuf> {
    let dir = pid_dir()?;
    if !dir.exists()
        && let Err(e) = fs::create_dir_all(&dir)
    {
        warn!(path = %dir.display(), error = %e, "Failed to create worker PID directory");
        return None;
    }
    Some(dir)
}

/// Record a spawned worker by writing its PID file.
///
/// File format: `server_pid={server_pid}\n` — one line, easy to parse.
pub(crate) fn record_worker_pid(worker_pid: u32) {
    let Some(dir) = ensure_pid_dir() else {
        return;
    };
    let path = dir.join(worker_pid.to_string());
    let server_pid = std::process::id();
    let content = format!("server_pid={server_pid}\n");
    match fs::File::create(&path).and_then(|mut f| f.write_all(content.as_bytes())) {
        Ok(()) => {
            debug!(worker_pid, server_pid, path = %path.display(), "Recorded worker PID file");
        }
        Err(e) => {
            warn!(worker_pid, path = %path.display(), error = %e, "Failed to write worker PID file");
        }
    }
}

/// Remove a worker's PID file (called on clean shutdown / Drop).
pub(crate) fn remove_worker_pid(worker_pid: u32) {
    let Some(dir) = pid_dir() else {
        return;
    };
    let path = dir.join(worker_pid.to_string());
    match fs::remove_file(&path) {
        Ok(()) => {
            debug!(worker_pid, path = %path.display(), "Removed worker PID file");
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Already cleaned up — fine.
        }
        Err(e) => {
            warn!(worker_pid, path = %path.display(), error = %e, "Failed to remove worker PID file");
        }
    }
}

/// Scan the PID directory and kill orphaned workers.
///
/// An orphan is a worker whose PID file exists, whose process is still alive,
/// but whose recorded server PID is dead (server crashed or exited).
///
/// Returns the number of orphans reaped.
pub(crate) fn reap_orphaned_workers() -> usize {
    let Some(dir) = pid_dir() else {
        return 0;
    };
    if !dir.exists() {
        return 0;
    }

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(path = %dir.display(), error = %e, "Failed to read worker PID directory");
            return 0;
        }
    };

    let mut reaped = 0;
    let current_server_pid = std::process::id();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(worker_pid) = filename.parse::<u32>() else {
            // Not a PID file — skip.
            continue;
        };

        match classify_pid_file(&path, worker_pid, current_server_pid) {
            PidFileStatus::Stale => {
                // Worker is dead — just clean up the file.
                debug!(worker_pid, "Removing stale PID file (worker already dead)");
                let _ = fs::remove_file(&path);
            }
            PidFileStatus::Orphan { server_pid } => {
                info!(
                    worker_pid,
                    dead_server_pid = server_pid,
                    "Reaping orphaned worker (parent server is dead)"
                );
                kill_orphan(worker_pid);
                let _ = fs::remove_file(&path);
                reaped += 1;
            }
            PidFileStatus::OwnedByUs => {
                // This worker belongs to our server — leave it alone.
                debug!(worker_pid, "PID file belongs to current server, skipping");
            }
            PidFileStatus::OwnedByOtherLiveServer { server_pid } => {
                // Another live server owns this worker — leave it alone.
                debug!(
                    worker_pid,
                    other_server_pid = server_pid,
                    "PID file belongs to another live server, skipping"
                );
            }
            PidFileStatus::Unreadable => {
                // Can't parse the file — remove it to avoid accumulation.
                warn!(worker_pid, "Removing unreadable PID file");
                let _ = fs::remove_file(&path);
            }
        }
    }

    if reaped > 0 {
        info!(reaped, "Reaped orphaned worker(s)");
    }
    reaped
}

/// Classification of a PID file.
enum PidFileStatus {
    /// Worker process is dead. File is stale.
    Stale,
    /// Worker is alive but its server is dead — orphan.
    Orphan { server_pid: u32 },
    /// Worker belongs to the current server process.
    OwnedByUs,
    /// Worker belongs to another server that is still alive.
    OwnedByOtherLiveServer { server_pid: u32 },
    /// PID file could not be parsed.
    Unreadable,
}

fn classify_pid_file(path: &Path, worker_pid: u32, current_server_pid: u32) -> PidFileStatus {
    // Read and parse the PID file.
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return PidFileStatus::Unreadable,
    };

    let server_pid = match parse_server_pid(&content) {
        Some(pid) => pid,
        None => return PidFileStatus::Unreadable,
    };

    // Is the worker still alive?
    if !process_alive(worker_pid) {
        return PidFileStatus::Stale;
    }

    // Worker is alive. Is its server still alive?
    if server_pid == current_server_pid {
        return PidFileStatus::OwnedByUs;
    }

    if process_alive(server_pid) {
        return PidFileStatus::OwnedByOtherLiveServer { server_pid };
    }

    PidFileStatus::Orphan { server_pid }
}

/// Parse `server_pid=12345` from the PID file content.
fn parse_server_pid(content: &str) -> Option<u32> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("server_pid=") {
            return value.trim().parse::<u32>().ok();
        }
    }
    None
}

/// Check if a process is alive via `kill(pid, 0)`. Single home for
/// the existence check used by the orphan reaper, the cancel-driven
/// worker shutdown (`job_tracker::signal_workers`), `WorkerHandle::Drop`,
/// and integration tests.
#[cfg(unix)]
pub(crate) fn process_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) sends no signal, just checks if the process exists
    // and we have permission to signal it.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
pub(crate) fn process_alive(_pid: u32) -> bool {
    // On non-Unix, conservatively assume alive (don't reap).
    true
}

/// Send SIGTERM to a worker's process group plus the PID directly.
/// Workers are spawned with `setpgid(0,0)` so PGID == PID; the
/// direct fallback covers TCP daemons whose PGID may differ.
/// Single primitive for `WorkerHandle::Drop`, `kill_orphan`, and
/// `job_tracker::send_terminate`.
#[cfg(unix)]
pub(crate) fn terminate_pgid(pid: u32) {
    let raw = pid as libc::pid_t;
    // SAFETY: SIGTERM to the worker's process group plus the PID.
    unsafe {
        libc::killpg(raw, libc::SIGTERM);
        libc::kill(raw, libc::SIGTERM);
    }
}

/// Send SIGKILL to a worker's process group plus the PID. Same
/// shape as `terminate_pgid` — the only operational difference
/// is the signal.
#[cfg(unix)]
pub(crate) fn kill_pgid(pid: u32) {
    let raw = pid as libc::pid_t;
    // SAFETY: SIGKILL to the worker's process group plus the PID.
    unsafe {
        libc::killpg(raw, libc::SIGKILL);
        libc::kill(raw, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
pub(crate) fn terminate_pgid(_pid: u32) {}

#[cfg(not(unix))]
pub(crate) fn kill_pgid(_pid: u32) {}

/// Kill an orphaned worker: SIGTERM, wait 2s, SIGKILL.
#[cfg(unix)]
fn kill_orphan(worker_pid: u32) {
    terminate_pgid(worker_pid);
    std::thread::sleep(Duration::from_secs(2));
    if process_alive(worker_pid) {
        info!(
            worker_pid,
            "Orphan didn't exit after SIGTERM, sending SIGKILL"
        );
        kill_pgid(worker_pid);
    }
}

#[cfg(not(unix))]
fn kill_orphan(_worker_pid: u32) {
    // No-op on non-Unix platforms. Workers on Windows don't use setpgid
    // and are tied to the parent process.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_server_pid_extracts_value() {
        assert_eq!(parse_server_pid("server_pid=12345\n"), Some(12345));
        assert_eq!(parse_server_pid("server_pid=1\n"), Some(1));
        assert_eq!(parse_server_pid("  server_pid=99999  \n"), Some(99999));
    }

    #[test]
    fn parse_server_pid_rejects_invalid() {
        assert_eq!(parse_server_pid(""), None);
        assert_eq!(parse_server_pid("garbage"), None);
        assert_eq!(parse_server_pid("server_pid=notanumber\n"), None);
    }

    #[test]
    fn pid_dir_is_under_home() {
        if let Some(dir) = pid_dir() {
            assert!(dir.to_string_lossy().contains(".batchalign3"));
            assert!(dir.to_string_lossy().contains("worker-pids"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn current_process_is_alive() {
        assert!(process_alive(std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn nonexistent_process_is_not_alive() {
        // PID 4_000_000 is almost certainly not in use.
        assert!(!process_alive(4_000_000));
    }
}
