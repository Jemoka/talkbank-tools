//! Runtime filesystem layout resolution.
//!
//! Resolves the batchalign3 state directory and derived paths (jobs, logs,
//! config, PID file, etc.) from environment variables or explicit sources.
//! The layout is determined once at startup and threaded through the server
//! and CLI.

use std::path::{Path, PathBuf};

/// Runtime-owned filesystem layout resolved from env/home defaults at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayout {
    state_dir: PathBuf,
    config_path: PathBuf,
}

impl RuntimeLayout {
    /// Resolve the runtime layout from ambient environment variables.
    pub fn from_env() -> Self {
        Self::from_sources(
            std::env::var("BATCHALIGN_STATE_DIR").ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        )
    }

    /// Resolve the runtime layout from explicit state-dir and home-dir sources.
    pub fn from_sources(state_dir_env: Option<&str>, home_env: Option<&str>) -> Self {
        let state_dir = state_dir_env
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Documented default: fall back to home dir or platform temp dir.
                let base = home_env
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir);
                base.join(".batchalign3")
            });
        Self::from_state_dir(state_dir)
    }

    /// Build the runtime layout from an explicit state directory.
    pub fn from_state_dir(state_dir: PathBuf) -> Self {
        let config_path = state_dir.join("server.yaml");
        Self {
            state_dir,
            config_path,
        }
    }

    /// Runtime state directory (jobs, DB, daemon metadata, logs, config).
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Default server config path under the runtime state directory.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Runtime jobs directory under the owned state root.
    pub fn jobs_dir(&self) -> PathBuf {
        self.state_dir.join("jobs")
    }

    /// Runtime logs directory under the owned state root.
    pub fn logs_dir(&self) -> PathBuf {
        self.state_dir.join("logs")
    }

    /// Runtime bug-report directory under the owned state root.
    pub fn bug_reports_dir(&self) -> PathBuf {
        self.state_dir.join("bug-reports")
    }

    /// Runtime dashboard asset directory under the owned state root.
    pub fn dashboard_dir(&self) -> PathBuf {
        self.state_dir.join("dashboard")
    }

    /// Server PID file under the owned state root.
    pub fn server_pid_path(&self) -> PathBuf {
        self.state_dir.join("server.pid")
    }

    /// Server stderr log file under the owned state root.
    pub fn server_log_path(&self) -> PathBuf {
        self.state_dir.join("server.log")
    }
}

/// Default config file path.
pub fn default_config_path() -> PathBuf {
    RuntimeLayout::from_env().config_path().to_path_buf()
}

/// State directory: `$BATCHALIGN_STATE_DIR` if set, else `$HOME/.batchalign3`.
pub fn ba_state_dir() -> PathBuf {
    RuntimeLayout::from_env().state_dir().to_path_buf()
}
