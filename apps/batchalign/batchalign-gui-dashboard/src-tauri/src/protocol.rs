#![allow(dead_code)] // Constants used by tests and mirrored in TypeScript protocol.ts
//! Stable Tauri protocol identifiers and payload shapes for the desktop shell.
//!
//! The React frontend keeps a mirrored inventory in
//! `apps/batchalign/cli-web-statuspage/src/desktop/protocol.ts`. Keep command/event identifiers and
//! serialized field names aligned across both sides when this seam changes.

use serde::{Deserialize, Serialize};

// Command/event name constants are the single source of truth for test
// stability assertions. The Tauri `#[tauri::command]` macro uses the
// function name directly as the command identifier, so these constants
// are not referenced in non-test production code — only in the test
// module below and in the TypeScript mirror (apps/batchalign/cli-web-statuspage/src/desktop/protocol.ts).

/// Tauri command name for recursive file discovery.
pub const DISCOVER_FILES_COMMAND: &str = "discover_files";
/// Tauri command name for checking first-launch state.
pub const IS_FIRST_LAUNCH_COMMAND: &str = "is_first_launch";
/// Tauri command name for reading desktop config.
pub const READ_CONFIG_COMMAND: &str = "read_config";
/// Tauri command name for writing desktop config.
pub const WRITE_CONFIG_COMMAND: &str = "write_config";
/// Tauri command name for resolving the batchalign binary path.
pub const GET_BATCHALIGN_PATH_COMMAND: &str = "get_batchalign_path";
/// Tauri command name for querying server status.
pub const SERVER_STATUS_COMMAND: &str = "server_status";
/// Tauri command name for starting the managed server.
pub const START_SERVER_COMMAND: &str = "start_server";
/// Tauri command name for stopping the managed server.
pub const STOP_SERVER_COMMAND: &str = "stop_server";
/// Custom event emitted when the managed server status changes.
pub const SERVER_STATUS_CHANGED_EVENT: &str = "desktop://server-status-changed";

/// Subset of the INI config that the setup wizard cares about.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UserConfig {
    /// ASR engine: "rev" or "whisper".
    pub engine: String,
    /// Rev.AI API key, if engine is "rev".
    pub rev_key: Option<String>,
}

/// Small acknowledgement payload returned by shell-side mutations.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DesktopCommandAck {
    /// Human-readable message about the completed desktop action.
    pub message: String,
}

/// Status snapshot returned to the frontend by server lifecycle commands.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerStatusInfo {
    /// Whether the child process is currently alive.
    pub running: bool,
    /// The port the server is (or would be) listening on.
    pub port: u16,
    /// Path to the `batchalign3` binary, or null if not found.
    pub binary_path: Option<String>,
    /// PID of the server process, if running.
    pub pid: Option<u32>,
}

/// Event payload emitted when the managed server status changes.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerStatusChangedEvent {
    /// Latest status snapshot after one lifecycle transition.
    pub status: ServerStatusInfo,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn command_and_event_identifiers_stay_stable() {
        assert_eq!(DISCOVER_FILES_COMMAND, "discover_files");
        assert_eq!(IS_FIRST_LAUNCH_COMMAND, "is_first_launch");
        assert_eq!(READ_CONFIG_COMMAND, "read_config");
        assert_eq!(WRITE_CONFIG_COMMAND, "write_config");
        assert_eq!(GET_BATCHALIGN_PATH_COMMAND, "get_batchalign_path");
        assert_eq!(SERVER_STATUS_COMMAND, "server_status");
        assert_eq!(START_SERVER_COMMAND, "start_server");
        assert_eq!(STOP_SERVER_COMMAND, "stop_server");
        assert_eq!(
            SERVER_STATUS_CHANGED_EVENT,
            "desktop://server-status-changed"
        );
    }

    #[test]
    fn server_status_changed_event_serializes_expected_wrapper() {
        let payload = ServerStatusChangedEvent {
            status: ServerStatusInfo {
                running: true,
                port: 18_000,
                binary_path: Some("/usr/local/bin/batchalign3".into()),
                pid: Some(42),
            },
        };

        let json = serde_json::to_value(payload).expect("serialize status event");

        assert_eq!(
            json,
            json!({
                "status": {
                    "running": true,
                    "port": 18000,
                    "binary_path": "/usr/local/bin/batchalign3",
                    "pid": 42,
                }
            })
        );
    }
}
