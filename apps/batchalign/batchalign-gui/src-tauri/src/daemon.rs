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
    events, DaemonFailedPayload, DaemonProgressPayload, DaemonReadyPayload,
};
use crate::state::{AppState, DaemonHandle};

const SIDECAR: &str = "sidecar";

/// Stamped at compile time by build.rs from the BATCHALIGN_BUILD_HASH
/// env var, which the wrapper scripts (bazel/batchalign-tauri/{dev,
/// bundle}.sh) populate from bazel/stamp.sh. Format mirrors stamp.sh:
/// `<git-sha>[-dirty]`. When this differs from the marker file inside
/// the PyApp install dir, the install was populated by a different
/// build and we wipe it.
const BUILD_HASH: &str = env!("BATCHALIGN_BUILD_HASH");

/// Marker file dropped inside each PyApp install dir on successful
/// boot, containing the embedded `BUILD_HASH`. The next launch reads
/// it and decides whether the install is from this build.
const PYAPP_BUILD_MARKER: &str = ".batchalign-build-hash";

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
    // PyApp's cache key is derived from the wheel filename + Python
    // version; it does NOT include PYAPP_PROJECT_FEATURES. Bumping the
    // bundled extras (api → api,all) silently reuses the old install
    // at ~/Library/Application Support/pyapp/batchalign/<hash>/<ver>/
    // and the daemon then crashes with `ModuleNotFoundError: stanza`
    // on the first non-trivial recipe run.
    //
    // Self-heal: the Tauri binary is stamped at build time with
    // BATCHALIGN_BUILD_HASH (see bazel/stamp.sh + build.rs). We drop
    // a marker file inside the PyApp install dir on every successful
    // boot. If on next boot the marker is missing or differs from the
    // currently-embedded hash, the install is from a stale build and
    // we wipe it — PyApp then repopulates with the current features.
    if let Err(e) = invalidate_stale_pyapp_install(BUILD_HASH) {
        eprintln!("[daemon  setup] pyapp cache check failed: {e}");
    }

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
    let port = wait_for_port(app, &mut rx).await?;
    let (shutdown_tx, _shutdown_rx) = watch::channel(false);

    // PyApp finished unpacking + installing extras by the time it
    // bound a port — tag the install with our build hash so the next
    // launch knows this install is current and skips the wipe.
    if let Err(e) = write_pyapp_build_marker(BUILD_HASH) {
        eprintln!("[daemon  setup] failed to write pyapp build marker: {e}");
    }

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
    app: &AppHandle,
    rx: &mut tauri::async_runtime::Receiver<CommandEvent>,
) -> Result<u16, DaemonError> {
    // Cold-start budget. First-ever launch downloads ~several GB worth
    // of wheels (torch, transformers, stanza, pyannote, openai-whisper
    // …) and pip-installs them all. On a quiet machine with a good
    // connection that's 3–6 minutes; on a slow one it can be 10+. We
    // pick 15 minutes — long enough to cover almost any cold install
    // without hiding a genuine hang forever.
    //
    // To keep the user from thinking the app froze during that window,
    // every stderr line is forwarded to the frontend as a
    // `daemon-progress` event; the overlay surfaces the latest line
    // (e.g. "Collecting torch", "Installing collected packages …") so
    // there's visible motion the whole time.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let mut tail: Vec<String> = Vec::new();
    while std::time::Instant::now() < deadline {
        let evt = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            rx.recv(),
        )
        .await
        {
            Ok(Some(evt)) => evt,
            Ok(None) => {
                return Err(DaemonError::EarlyExit(format!(
                    "stdout closed before announcing port. last output:\n{}",
                    tail.join("\n"),
                )));
            }
            Err(_) => continue, // poll again
        };
        match evt {
            CommandEvent::Stdout(line) => {
                let text = String::from_utf8_lossy(&line).into_owned();
                let trimmed = text.trim();
                eprintln!("[daemon stdout] {trimmed}");
                push_tail(&mut tail, &text);
                if !trimmed.is_empty() {
                    let _ = app.emit(
                        events::DAEMON_PROGRESS,
                        DaemonProgressPayload { line: trimmed.to_owned() },
                    );
                }
                if let Some(port) = parse_port(trimmed) {
                    return Ok(port);
                }
            }
            CommandEvent::Stderr(line) => {
                let text = String::from_utf8_lossy(&line).into_owned();
                let trimmed = text.trim_end();
                eprintln!("[daemon stderr] {trimmed}");
                push_tail(&mut tail, &text);
                if !trimmed.is_empty() {
                    let _ = app.emit(
                        events::DAEMON_PROGRESS,
                        DaemonProgressPayload { line: trimmed.to_owned() },
                    );
                }
                // Fallback: newer uvicorn versions moved `Server.servers`,
                // breaking `_PortAnnouncingServer.startup`'s stdout
                // announcement (the override swallows AttributeError and
                // silently returns). Recognize uvicorn's stable startup
                // log line "Uvicorn running on http://127.0.0.1:<port>"
                // as a secondary port announcement.
                if let Some(port) = parse_port(trimmed) {
                    eprintln!("[daemon  match] port={port} via uvicorn log");
                    return Ok(port);
                }
            }
            CommandEvent::Error(e) => {
                eprintln!("[daemon  error] {e}");
                return Err(DaemonError::EarlyExit(format!(
                    "{e}. last output:\n{}",
                    tail.join("\n"),
                )));
            }
            CommandEvent::Terminated(payload) => {
                eprintln!(
                    "[daemon   exit] code={:?} signal={:?}",
                    payload.code, payload.signal,
                );
                return Err(DaemonError::EarlyExit(format!(
                    "daemon exited (code={:?}) before announcing port. last output:\n{}",
                    payload.code,
                    tail.join("\n"),
                )));
            }
            _ => {}
        }
    }
    Err(DaemonError::PortTimeout)
}

/// Recognize a port announcement from one daemon log line. Two forms:
///   - `DAEMON_PORT=<n>`               — the Python-side explicit signal
///   - `Uvicorn running on http(s)://<host>:<n>` — uvicorn's own startup log
/// The uvicorn fallback exists because newer uvicorn versions moved
/// `Server.servers`, breaking `_PortAnnouncingServer.startup`'s introspection
/// — it silently fails and never prints DAEMON_PORT, leaving the GUI stuck
/// at "starting daemon…". The uvicorn log itself is the stable contract.
fn parse_port(line: &str) -> Option<u16> {
    if let Some(rest) = line.strip_prefix("DAEMON_PORT=") {
        return rest.trim().parse().ok();
    }
    // Find " http://" or " https://" then the colon, then the port. uvicorn's
    // INFO logs prepend "INFO:" so we substring-search rather than expect a
    // line start.
    let scheme = ["http://", "https://"].into_iter().find_map(|s| {
        line.find(s).map(|i| i + s.len())
    })?;
    let rest = &line[scheme..];
    let colon = rest.find(':')?;
    let after_colon = &rest[colon + 1..];
    // Port runs until the first non-digit (space, comma, '/', etc.)
    let end = after_colon
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_colon.len());
    if end == 0 {
        return None;
    }
    after_colon[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_port;

    #[test]
    fn parse_port_explicit() {
        assert_eq!(parse_port("DAEMON_PORT=52064"), Some(52064));
        assert_eq!(parse_port("DAEMON_PORT=  52064 "), Some(52064));
        assert_eq!(parse_port("DAEMON_PORT=not-a-number"), None);
    }

    #[test]
    fn parse_port_uvicorn_log() {
        assert_eq!(
            parse_port("INFO:     Uvicorn running on http://127.0.0.1:52064 (Press CTRL+C to quit)"),
            Some(52064),
        );
        assert_eq!(
            parse_port("Uvicorn running on https://localhost:8000/"),
            Some(8000),
        );
    }

    #[test]
    fn parse_port_unrelated() {
        assert_eq!(parse_port("INFO:     Application startup complete."), None);
        assert_eq!(parse_port(""), None);
    }
}

/// Bounded ring of recent daemon log lines. Capped so the GUI doesn't
/// pin tens of MB of stdout in the daemon-failed reason string.
fn push_tail(tail: &mut Vec<String>, text: &str) {
    const MAX_LINES: usize = 40;
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return;
    }
    tail.push(trimmed.to_owned());
    if tail.len() > MAX_LINES {
        let drain_to = tail.len() - MAX_LINES;
        tail.drain(0..drain_to);
    }
}

/// Path to the PyApp install root: `<data_dir>/pyapp/batchalign`. Each
/// (project-hash, version) pair lives in a subdir below this.
fn pyapp_project_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("pyapp").join("batchalign"))
}

/// Walk every existing PyApp install for the batchalign project and
/// wipe any whose `.batchalign-build-hash` marker is missing or
/// doesn't match `expected`. PyApp will re-pip-install the wheel + the
/// configured extras on next launch.
///
/// Called before the sidecar spawns. Returning `Ok(())` with no wipe
/// is the happy path — the install is from this build, no work needed.
fn invalidate_stale_pyapp_install(expected: &str) -> std::io::Result<()> {
    let Some(root) = pyapp_project_dir() else {
        return Ok(());
    };
    if !root.is_dir() {
        return Ok(());
    }
    let mut wiped_any = false;
    for hash_entry in std::fs::read_dir(&root)? {
        let hash_path = hash_entry?.path();
        if !hash_path.is_dir() {
            continue;
        }
        // Each `<hash>/` directory holds a `<version>/` directory per
        // wheel version PyApp has unpacked. Check the marker inside
        // each version dir.
        for ver_entry in std::fs::read_dir(&hash_path)? {
            let ver_path = ver_entry?.path();
            if !ver_path.is_dir() {
                continue;
            }
            let marker = ver_path.join(PYAPP_BUILD_MARKER);
            let stale = match std::fs::read_to_string(&marker) {
                Ok(content) => content.trim() != expected,
                Err(_) => true, // missing marker → unknown provenance
            };
            if stale {
                eprintln!(
                    "[daemon  setup] wiping stale pyapp install {} (expected {expected})",
                    ver_path.display(),
                );
                std::fs::remove_dir_all(&ver_path)?;
                wiped_any = true;
            }
        }
        // If the version dir went away leave the hash dir alone —
        // PyApp recreates the version dir under the same hash on
        // re-install, no need to churn the parent.
    }
    if wiped_any {
        eprintln!("[daemon  setup] pyapp re-install will run on next sidecar boot");
    }
    Ok(())
}

/// Drop the build-hash marker inside the PyApp install dir that
/// matches our currently-running sidecar's wheel version. Called
/// after the sidecar successfully announces its port — at that point
/// we know the install is good and can be safely tagged with this
/// build's hash so future launches don't wipe it.
fn write_pyapp_build_marker(expected: &str) -> std::io::Result<()> {
    let Some(root) = pyapp_project_dir() else {
        return Ok(());
    };
    if !root.is_dir() {
        return Ok(());
    }
    for hash_entry in std::fs::read_dir(&root)? {
        let hash_path = hash_entry?.path();
        if !hash_path.is_dir() {
            continue;
        }
        for ver_entry in std::fs::read_dir(&hash_path)? {
            let ver_path = ver_entry?.path();
            if !ver_path.is_dir() {
                continue;
            }
            let marker = ver_path.join(PYAPP_BUILD_MARKER);
            std::fs::write(&marker, expected)?;
        }
    }
    Ok(())
}

async fn drain_child(
    app: &AppHandle,
    _child: CommandChild,
    mut rx: tauri::async_runtime::Receiver<CommandEvent>,
) {
    while let Some(evt) = rx.recv().await {
        match evt {
            CommandEvent::Stdout(line) => {
                eprintln!(
                    "[daemon stdout] {}",
                    String::from_utf8_lossy(&line).trim_end(),
                );
            }
            CommandEvent::Stderr(line) => {
                eprintln!(
                    "[daemon stderr] {}",
                    String::from_utf8_lossy(&line).trim_end(),
                );
            }
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
                eprintln!("[daemon  error] {e}");
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
