//! App-managed Tauri state: the daemon handle.
//!
//! Lock-free via `arc-swap` (per chatter-gui convention). The handle is
//! populated by `daemon::spawn()` once the sidecar prints
//! `DAEMON_PORT=<n>` to stdout, and consumed by every HTTP-proxy command.

use std::sync::Arc;

use arc_swap::ArcSwapOption;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct DaemonHandle {
    pub port: u16,
    /// Set true once the daemon has shut down (clean or crashed) so
    /// further requests fail fast instead of hanging on a dead socket.
    pub shutdown: tokio::sync::watch::Sender<bool>,
}

#[derive(Default)]
pub struct AppState {
    pub daemon: ArcSwapOption<DaemonHandle>,
    /// Per-batch SSE pump cancellers. A new batch start replaces the
    /// previous canceller for that batch (rare; the GUI runs at most
    /// one job per tab).
    pub pumps: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn daemon_port(&self) -> Option<u16> {
        self.daemon.load().as_deref().map(|h| h.port)
    }

    pub fn set_daemon(&self, handle: DaemonHandle) {
        self.daemon.store(Some(Arc::new(handle)));
    }
}
