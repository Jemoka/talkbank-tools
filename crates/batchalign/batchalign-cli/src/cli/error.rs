//! Typed CLI errors with stable exit codes for scripting.
//!
//! Every error the CLI can encounter is represented as a [`CliError`] variant.
//! Each variant maps to one of six exit code categories (2--6) via
//! [`CliError::exit_code()`], providing a stable contract that shell scripts
//! and CI pipelines can match on without parsing stderr text.
//!
//! Exit code 1 (`EXIT_GENERAL`) is reserved but currently unused -- it exists
//! so that an unexpected panic or catch-all still produces a nonzero exit
//! distinguishable from the structured categories.

use std::path::PathBuf;

use crate::api::{JobId, ReleasedCommand};

/// All errors that the CLI can encounter.
///
/// Each variant maps to a stable exit code category via [`CliError::exit_code()`].
/// Scripts should match on exit codes, not error messages.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// No input files or directories were provided on the command line or via
    /// `--file-list`. Triggers before any server communication.
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("no input paths provided")]
    NoInputPaths,

    /// A path explicitly named on the command line does not exist on disk.
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("input path does not exist: {0}")]
    InputMissing(PathBuf),

    /// The file given to `--file-list` does not exist.
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("file list does not exist: {0}")]
    FileListMissing(PathBuf),

    /// The `--file-list` file exists but contains no usable paths.
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("file list is empty")]
    FileListEmpty,

    /// A CLI argument value failed validation (e.g. invalid language code).
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("{0}")]
    InvalidArgument(String),

    /// The server at `url` did not respond to a health check or connection
    /// attempt. This covers DNS failures, refused connections, and TLS errors.
    /// Exit code: [`EXIT_NETWORK`](Self::EXIT_NETWORK) (4).
    #[error("cannot reach server at {url}: {source}")]
    ServerUnreachable {
        /// Base URL of the unreachable server.
        url: String,
        /// Underlying connection error.
        #[source]
        source: reqwest::Error,
    },

    /// The server is reachable but does not advertise support for the
    /// requested command (e.g. `transcribe` on a text-only server).
    /// Detected via the capabilities probe before job submission.
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("server {server} does not support command '{command}'")]
    UnsupportedCommand {
        /// Requested processing command.
        command: ReleasedCommand,
        /// Server URL that was queried.
        server: String,
    },

    /// The server returned a non-2xx HTTP status during job submission or
    /// result retrieval. `detail` contains the response body or status text.
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("server returned {status}: {detail}")]
    ServerHttp {
        /// HTTP status code.
        status: u16,
        /// Response body or status text.
        detail: String,
    },

    /// The local auto-daemon could not be spawned or failed its health check
    /// after startup. Check `~/.batchalign3/daemon.log` for details.
    /// Exit code: [`EXIT_CONFIG`](Self::EXIT_CONFIG) (3).
    #[error("daemon failed to start")]
    DaemonStartFailed,

    /// The CLI exhausted its retry budget while polling for job completion.
    /// This usually indicates the server crashed or became unresponsive
    /// mid-job rather than a transient network blip.
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("poll exhausted after {attempts} failures")]
    PollExhausted {
        /// Number of consecutive poll failures before giving up.
        attempts: u32,
    },

    /// The server acknowledged the job but later returned 404 when polled.
    /// Indicates the server restarted and lost in-memory state, or the job
    /// was garbage-collected before the client retrieved results.
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("server lost job {job_id}")]
    JobLost {
        /// ID of the job that disappeared from the server.
        job_id: JobId,
    },

    /// The server kept the job record, but it reached a terminal non-success
    /// state (`failed`, `cancelled`, or `interrupted`) or surfaced file-level
    /// errors while polling.
    ///
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("job {job_id} finished with status {status}: {detail}")]
    JobFailed {
        /// ID of the terminal job.
        job_id: JobId,
        /// Terminal server-reported status string.
        status: String,
        /// Best available human-readable failure detail.
        detail: String,
    },

    /// A server-returned filename would resolve to a path outside the output
    /// directory (e.g. `../../../etc/passwd`). The write is blocked to prevent
    /// a malicious or misconfigured server from overwriting arbitrary files.
    /// Exit code: [`EXIT_USAGE`](Self::EXIT_USAGE) (2).
    #[error("path escapes output directory: {0}")]
    PathTraversal(String),

    /// An error propagated from the embedded server crate (e.g. when
    /// running `batchalign3 serve start --foreground`).
    /// Exit code: [`EXIT_SERVER`](Self::EXIT_SERVER) (5).
    #[error("server error: {0}")]
    Server(#[from] crate::error::ServerError),

    /// Database error from the local utterance cache database.
    /// Exit code: [`EXIT_LOCAL_RUNTIME`](Self::EXIT_LOCAL_RUNTIME) (6).
    #[error("cache database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Filesystem I/O error. Exit code depends on the [`ErrorKind`](std::io::ErrorKind):
    /// `InvalidInput`/`InvalidData` map to [`EXIT_USAGE`](Self::EXIT_USAGE) (2);
    /// everything else maps to [`EXIT_LOCAL_RUNTIME`](Self::EXIT_LOCAL_RUNTIME) (6).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// HTTP client error not associated with a specific server URL (e.g.
    /// timeout during result download, TLS handshake failure).
    /// Exit code: [`EXIT_NETWORK`](Self::EXIT_NETWORK) (4).
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failure, typically when parsing a
    /// server response or writing the job submission payload.
    /// Exit code: [`EXIT_LOCAL_RUNTIME`](Self::EXIT_LOCAL_RUNTIME) (6).
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Configuration file error (malformed `server.yaml` or
    /// `~/.batchalign.ini`). Propagated from the config loader in
    /// `batchalign_types`.
    /// Exit code: [`EXIT_CONFIG`](Self::EXIT_CONFIG) (3).
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
}

impl CliError {
    /// Catch-all for unexpected failures (panics, unclassified errors).
    /// Value 1 follows the Unix convention where any nonzero exit means failure;
    /// keeping it distinct from the structured codes (2--6) lets scripts detect
    /// "something truly unexpected happened" vs. a known failure category.
    pub const EXIT_GENERAL: i32 = 1;

    /// The user supplied invalid arguments or paths that could be corrected
    /// without changing the environment. Mirrors the BSD `sysexits.h` EX_USAGE
    /// convention (value 64 there, but we use a compact 2--6 range so that
    /// callers can test with simple numeric comparisons).
    pub const EXIT_USAGE: i32 = 2;

    /// A required configuration file is missing, malformed, or contains
    /// invalid values. The user needs to fix config before retrying.
    /// Separated from USAGE because the fix is editing a file, not changing
    /// CLI flags.
    pub const EXIT_CONFIG: i32 = 3;

    /// The CLI could not reach any server (connection refused, DNS failure,
    /// TLS error). Retrying after a network change
    /// or server restart may help.
    pub const EXIT_NETWORK: i32 = 4;

    /// The server was reachable but the job failed on the server side:
    /// HTTP errors, unsupported commands, poll exhaustion, lost jobs.
    /// Distinguished from NETWORK because the connection succeeded --
    /// the problem is in the server's processing or state.
    pub const EXIT_SERVER: i32 = 5;

    /// A local runtime dependency failed: SQLite, JSON parsing, GUI launch,
    /// or general I/O errors (disk full, permission denied). These are not
    /// user-input or network problems -- something on the local machine is
    /// broken or misconfigured.
    pub const EXIT_LOCAL_RUNTIME: i32 = 6;

    /// Stable process exit code for this error category.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoInputPaths
            | Self::InputMissing(_)
            | Self::FileListMissing(_)
            | Self::FileListEmpty
            | Self::InvalidArgument(_)
            | Self::PathTraversal(_) => Self::EXIT_USAGE,
            Self::Config(_) | Self::DaemonStartFailed => Self::EXIT_CONFIG,
            Self::ServerUnreachable { .. } | Self::Http(_) => Self::EXIT_NETWORK,
            Self::UnsupportedCommand { .. }
            | Self::ServerHttp { .. }
            | Self::PollExhausted { .. }
            | Self::JobLost { .. }
            | Self::JobFailed { .. }
            | Self::Server(_) => Self::EXIT_SERVER,
            Self::Database(_) | Self::Json(_) => Self::EXIT_LOCAL_RUNTIME,
            Self::Io(err) => match err.kind() {
                std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                    Self::EXIT_USAGE
                }
                _ => Self::EXIT_LOCAL_RUNTIME,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliError;
    use std::path::PathBuf;

    use crate::config::ConfigError;
    use crate::error::ServerError;

    #[test]
    fn usage_errors_map_to_exit_usage() {
        let errs = [
            CliError::NoInputPaths,
            CliError::InputMissing(PathBuf::from("/tmp/missing.cha")),
            CliError::FileListMissing(PathBuf::from("/tmp/list.txt")),
            CliError::FileListEmpty,
            CliError::PathTraversal("../escape".into()),
            CliError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "bad flag",
            )),
        ];
        for err in &errs {
            assert_eq!(err.exit_code(), CliError::EXIT_USAGE);
        }
    }

    #[test]
    fn config_errors_map_to_exit_config() {
        let cfg_err = ConfigError::Parse(PathBuf::from("/tmp/server.yaml"), "bad yaml".into());
        assert_eq!(CliError::Config(cfg_err).exit_code(), CliError::EXIT_CONFIG);
        assert_eq!(
            CliError::DaemonStartFailed.exit_code(),
            CliError::EXIT_CONFIG
        );
    }

    #[test]
    fn server_errors_map_to_exit_server() {
        let errs = [
            CliError::ServerHttp {
                status: 500,
                detail: "boom".into(),
            },
            CliError::PollExhausted { attempts: 10 },
            CliError::JobLost {
                job_id: "job123".into(),
            },
            CliError::JobFailed {
                job_id: "job123".into(),
                status: "failed".into(),
                detail: "boom".into(),
            },
            CliError::Server(ServerError::Validation("bad".into())),
        ];
        for err in &errs {
            assert_eq!(err.exit_code(), CliError::EXIT_SERVER);
        }
    }

    #[test]
    fn local_runtime_errors_map_to_exit_local_runtime() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let io_err = std::io::Error::other("disk full");
        let errs = [CliError::Json(json_err), CliError::Io(io_err)];
        for err in &errs {
            assert_eq!(err.exit_code(), CliError::EXIT_LOCAL_RUNTIME);
        }
    }
}
