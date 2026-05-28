//! Tauri command + event name constants.
//!
//! These must stay in lockstep with `apps/batchalign/batchalign-gui/
//! src/protocol/events.ts`. The TypeScript side has matching string
//! literals; we keep both ends string-typed because Tauri's invoke/listen
//! APIs are inherently name-based.

pub mod events {
    pub const DAEMON_READY: &str = "daemon-ready";
    pub const DAEMON_FAILED: &str = "daemon-failed";
    pub const PROGRESS_V2: &str = "progress-v2";
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonReadyPayload {
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonFailedPayload {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressV2Payload {
    pub batch_id: String,
    pub job_id: String,
    pub event: serde_json::Value,
}
