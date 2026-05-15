//! Server lifecycle management for the Tauri desktop shell.
//!
//! Spawns `batchalign3 serve start --foreground --port 18000` as a managed child
//! process. The server is auto-started on app launch via the Tauri `setup` hook
//! and auto-stopped on app exit. The frontend can also start/stop manually.
//!
//! Port 18000 is used instead of the default 8000 to avoid conflicts with any
//! manually started development server.

use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, State};

use crate::protocol::{ServerStatusChangedEvent, ServerStatusInfo, SERVER_STATUS_CHANGED_EVENT};

/// The desktop server port. Matches `DEFAULT_DESKTOP_SERVER` in `runtime.ts`.
const SERVER_PORT: u16 = 18000;

/// Managed state holding the server child process handle.
///
/// Wrapped in a `Mutex` so Tauri commands (which run on async threads) can
/// safely check and mutate the process. The `Option` is `None` when no server
/// has been started or after it has been stopped.
pub struct ServerProcess(Mutex<Option<Child>>);

impl ServerProcess {
    /// Create a new empty server-process manager.
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Option<Child>>, String> {
        self.0.lock().map_err(|e| format!("Lock error: {e}"))
    }

    fn start(&self) -> Result<(), String> {
        let mut guard = self.lock()?;

        if is_child_alive(&mut guard) {
            return Ok(());
        }

        let binary = find_batchalign().ok_or_else(|| {
            "batchalign3 not found on PATH. Install it with: uv tool install batchalign3"
                .to_string()
        })?;

        let child = Command::new(&binary)
            .args([
                "serve",
                "start",
                "--foreground",
                "--port",
                &SERVER_PORT.to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start server: {e}"))?;

        *guard = Some(child);
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        let mut guard = self.lock()?;

        if let Some(mut child) = guard.take() {
            // kill() sends SIGKILL on Unix, TerminateProcess on Windows.
            // We prefer this over a graceful SIGTERM because the server's
            // shutdown handler may take seconds and we want fast app exit.
            let _ = child.kill();
            let _ = child.wait(); // reap the zombie
        }
        Ok(())
    }

    fn status(&self) -> Result<ServerStatusInfo, String> {
        let mut guard = self.lock()?;
        let running = is_child_alive(&mut guard);
        let pid = guard.as_ref().map(|c| c.id());

        Ok(ServerStatusInfo {
            running,
            port: SERVER_PORT,
            binary_path: find_batchalign(),
            pid,
        })
    }

    fn shutdown(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn emit_status(app: &AppHandle, status: &ServerStatusInfo) {
    if let Err(error) = app.emit(
        SERVER_STATUS_CHANGED_EVENT,
        ServerStatusChangedEvent {
            status: status.clone(),
        },
    ) {
        eprintln!("[tauri] Failed to emit {SERVER_STATUS_CHANGED_EVENT}: {error}");
    }
}

/// Start the managed server and emit the resulting status snapshot.
pub(crate) fn start_managed_server(
    app: &AppHandle,
    process: &ServerProcess,
) -> Result<ServerStatusInfo, String> {
    process.start()?;
    let status = process.status()?;
    emit_status(app, &status);
    Ok(status)
}

/// Stop the managed server and emit the resulting status snapshot.
pub(crate) fn stop_managed_server(
    app: &AppHandle,
    process: &ServerProcess,
) -> Result<ServerStatusInfo, String> {
    process.stop()?;
    let status = process.status()?;
    emit_status(app, &status);
    Ok(status)
}

/// Locate the `batchalign3` binary on PATH.
///
/// Tries `batchalign3` first (installed via `uv tool install` or cargo),
/// then common locations on macOS/Linux.
fn find_batchalign() -> Option<String> {
    #[cfg(unix)]
    let cmd = "which";
    #[cfg(windows)]
    let cmd = "where";

    if let Ok(output) = Command::new(cmd).arg("batchalign3").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Check whether the managed child process is still alive.
///
/// Returns `true` if the process exists and hasn't exited. Also cleans up
/// the `Option` if the process has exited so subsequent calls see `None`.
fn is_child_alive(guard: &mut Option<Child>) -> bool {
    match guard {
        Some(child) => match child.try_wait() {
            Ok(None) => true, // still running
            Ok(Some(_)) => {
                // Process exited — clean up handle
                *guard = None;
                false
            }
            Err(_) => {
                *guard = None;
                false
            }
        },
        None => false,
    }
}

/// Start the batchalign3 server as a managed child process.
///
/// Returns a fresh shell status snapshot. The frontend should still poll
/// `/health` to detect when the server is ready to accept jobs.
#[tauri::command]
pub fn start_server(
    app: AppHandle,
    state: State<'_, ServerProcess>,
) -> Result<ServerStatusInfo, String> {
    start_managed_server(&app, state.inner())
}

/// Stop the managed server process and return the resulting shell status.
#[tauri::command]
pub fn stop_server(
    app: AppHandle,
    state: State<'_, ServerProcess>,
) -> Result<ServerStatusInfo, String> {
    stop_managed_server(&app, state.inner())
}

/// Return current server status for the frontend status bar.
#[tauri::command]
pub fn server_status(state: State<'_, ServerProcess>) -> Result<ServerStatusInfo, String> {
    state.inner().status()
}

/// Return the path to the batchalign3 binary, or null if not found.
///
/// The frontend keeps this command in the raw protocol inventory even though
/// the current capability surface relies on `server_status().binary_path`.
#[tauri::command]
pub fn get_batchalign_path() -> Option<String> {
    find_batchalign()
}

/// Auto-stop the server when the app exits.
pub fn shutdown_server(process: &ServerProcess) {
    process.shutdown();
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[cfg(unix)]
    fn spawn_running_child() -> Child {
        Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn long-running child")
    }

    #[cfg(windows)]
    fn spawn_running_child() -> Child {
        Command::new("cmd")
            .args(["/C", "ping 127.0.0.1 -n 30 > NUL"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn long-running child")
    }

    #[cfg(unix)]
    fn spawn_exited_child() -> Child {
        Command::new("sh")
            .args(["-c", "exit 0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn short-lived child")
    }

    #[cfg(windows)]
    fn spawn_exited_child() -> Child {
        Command::new("cmd")
            .args(["/C", "exit 0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn short-lived child")
    }

    #[test]
    fn fresh_status_reports_no_running_server() {
        let process = ServerProcess::new();

        let status = process.status().expect("query server status");

        assert!(!status.running);
        assert_eq!(status.port, SERVER_PORT);
        assert!(status.pid.is_none());
    }

    #[test]
    fn status_cleans_up_exited_children() {
        let process = ServerProcess::new();
        {
            let mut guard = process.lock().expect("lock server process");
            *guard = Some(spawn_exited_child());
        }
        std::thread::sleep(Duration::from_millis(50));

        let status = process.status().expect("query server status");

        assert!(!status.running);
        assert!(status.pid.is_none());
    }

    #[test]
    fn stop_reaps_running_children() {
        let process = ServerProcess::new();
        {
            let mut guard = process.lock().expect("lock server process");
            *guard = Some(spawn_running_child());
        }

        let before = process.status().expect("query running status");
        assert!(before.running);
        assert!(before.pid.is_some());

        process.stop().expect("stop running child");
        let after = process.status().expect("query stopped status");

        assert!(!after.running);
        assert!(after.pid.is_none());
    }
}
