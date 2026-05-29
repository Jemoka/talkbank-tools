//! Sidecar daemon manager.
//!
//! Spawns the bundled `sidecar` PyApp binary (`tauri.conf.json`
//! `bundle.externalBin = ["binaries/sidecar"]`) with `--port 0
//! --host 127.0.0.1`, reads its stdout for the `DAEMON_PORT=<n>` line
//! the daemon prints on startup (see `python/batchalign/cli/daemon.py
//! :_PortAnnouncingServer`), stores the resolved port in `AppState`,
//! and emits a `daemon-ready` Tauri event so the frontend's `bridge.ts`
//! can fetch `/capabilities`.
//!
//! On any failure during startup we emit `daemon-failed` instead with a
//! reason string the GUI surfaces to the user.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{Command, CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::watch;

use crate::protocol::{
    events, DaemonFailedPayload, DaemonReadyPayload,
};
use crate::state::{AppState, DaemonHandle};

const SIDECAR: &str = "sidecar";

/// Spawn the sidecar daemon at most once. The first caller to flip the
/// `daemon_spawning` latch wins; concurrent callers (e.g. setup() and
/// bridge.ts both racing on cold start) return immediately. If the
/// spawn fails the latch is reset so a retry can occur.
pub fn spawn(app: AppHandle) {
    let state = app.state::<AppState>();
    if state
        .daemon_spawning
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        match start(&app).await {
            Ok(_) => {}
            Err(e) => {
                app.state::<AppState>()
                    .daemon_spawning
                    .store(false, Ordering::Release);
                let _ = app.emit(
                    events::DAEMON_FAILED,
                    DaemonFailedPayload { reason: e.to_string() },
                );
            }
        }
    });
}

async fn start(app: &AppHandle) -> Result<(), DaemonError> {
    // Build the sidecar command. `Command::new_sidecar` resolves to the
    // platform-suffixed binary that the Tauri bundler placed in the
    // app resources (e.g. `binaries/sidecar-aarch64-apple-darwin`).
    // The PyApp-bundled sidecar IS the daemon — `run_pyapp_entry`
    // prepends "daemon" to argv internally (see
    // python/batchalign/cli/daemon.py:run_pyapp_entry). Passing "daemon"
    // here would duplicate the subcommand and Typer rejects it with
    // "Got unexpected extra argument(s) (daemon)".
    let cmd: Command = app
        .shell()
        .sidecar(SIDECAR)
        .map_err(|e| DaemonError::Spawn(e.to_string()))?
        // Trust local filesystem paths in `InputSpec`. The daemon's
        // `_paths_allowed()` (python/batchalign/api.py:235) requires
        // BATCHALIGN_API_ALLOW_PATHS=1 to honor `path` inputs. Because
        // the GUI's process boundary IS the user's machine — files
        // come from a Tauri folder picker, not arbitrary network
        // clients — there is no remote-trust concern; enabling paths
        // is the whole point of the desktop integration.
        .env("BATCHALIGN_API_ALLOW_PATHS", "1")
        .args([
            "--port",
            "0",
            "--host",
            "127.0.0.1",
            "--log-level",
            "info",
            "--no-access-log",
        ]);

    let (mut rx, child) = cmd
        .spawn()
        .map_err(|e| DaemonError::Spawn(e.to_string()))?;

    // Read the daemon's stdout until "DAEMON_PORT=<n>" appears or it dies.
    let port = wait_for_port(&mut rx).await?;
    let (shutdown_tx, _shutdown_rx) = watch::channel(false);

    // Store the handle BEFORE emitting daemon-ready so any frontend code
    // that immediately calls `daemon_port` (or fetches capabilities) sees
    // the populated state.
    let state = app.state::<AppState>();
    state.set_daemon(DaemonHandle { port, shutdown: shutdown_tx });

    let _ = app.emit(
        events::DAEMON_READY,
        DaemonReadyPayload { port },
    );

    // Keep draining stdout/stderr for the lifetime of the daemon so the
    // child's pipes never block. We surface unexpected exits as
    // `daemon-failed` so the GUI can show a recovery state.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        drain_child(&app_handle, child, rx).await;
    });

    Ok(())
}

async fn wait_for_port(
    rx: &mut tauri::async_runtime::Receiver<CommandEvent>,
) -> Result<u16, DaemonError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let evt = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            rx.recv(),
        )
        .await
        {
            Ok(Some(evt)) => evt,
            Ok(None) => return Err(DaemonError::EarlyExit("stdout closed".into())),
            Err(_) => continue, // poll again
        };
        match evt {
            CommandEvent::Stdout(line) => {
                let text = String::from_utf8_lossy(&line);
                if let Some(rest) = text.trim().strip_prefix("DAEMON_PORT=") {
                    if let Ok(port) = rest.parse::<u16>() {
                        return Ok(port);
                    }
                }
            }
            CommandEvent::Stderr(_) => { /* fine; daemon logs to stderr */ }
            CommandEvent::Error(e) => {
                return Err(DaemonError::EarlyExit(e));
            }
            CommandEvent::Terminated(payload) => {
                return Err(DaemonError::EarlyExit(format!(
                    "daemon exited (code={:?}) before announcing port",
                    payload.code,
                )));
            }
            _ => {}
        }
    }
    Err(DaemonError::PortTimeout)
}

async fn drain_child(
    app: &AppHandle,
    _child: CommandChild,
    mut rx: tauri::async_runtime::Receiver<CommandEvent>,
) {
    while let Some(evt) = rx.recv().await {
        match evt {
            CommandEvent::Stdout(_) | CommandEvent::Stderr(_) => {}
            CommandEvent::Terminated(payload) => {
                let _ = app.emit(
                    events::DAEMON_FAILED,
                    DaemonFailedPayload {
                        reason: format!("daemon exited (code={:?})", payload.code),
                    },
                );
                // Mark the handle as gone so callers know.
                let state = app.state::<AppState>();
                if let Some(h) = state.daemon.load_full() {
                    let _ = h.shutdown.send(true);
                }
                state.daemon.store(None);
                break;
            }
            CommandEvent::Error(e) => {
                let _ = app.emit(
                    events::DAEMON_FAILED,
                    DaemonFailedPayload { reason: e },
                );
            }
            _ => {}
        }
    }
}

/// Idempotent ensure-daemon: if the sidecar is already running, return
/// its port. If a spawn is already in flight (lib.rs's setup() always
/// fires one), wait for it. Only kick off a new spawn when no prior
/// attempt has been made — `spawn()`'s compare_exchange would no-op
/// the duplicate anyway, but checking here lets us return early without
/// the 5-second poll.
pub async fn ensure(app: AppHandle) -> Result<u16, String> {
    let state = app.state::<AppState>();
    if let Some(port) = state.daemon_port() {
        return Ok(port);
    }
    spawn(app.clone());
    // Best-effort: poll for up to a few seconds to surface the port
    // synchronously if it lands fast. Past the deadline the frontend
    // can still observe completion via the `daemon-ready` event.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Some(port) = state.daemon_port() {
            return Ok(port);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err("daemon still starting; listen for `daemon-ready`".into())
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("failed to spawn sidecar: {0}")]
    Spawn(String),
    #[error("daemon exited before announcing port: {0}")]
    EarlyExit(String),
    #[error("daemon did not announce port within timeout")]
    PortTimeout,
}

// Suppress dead-code on Arc import in some build configs.
#[allow(dead_code)]
fn _unused(_: Arc<()>) {}
