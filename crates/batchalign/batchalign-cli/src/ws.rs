//! WebSocket — broadcast channel for real-time dashboard updates.
//!
//! Port of `batchalign/serve/websocket.py`. Uses `tokio::sync::broadcast`
//! instead of maintaining a list of connections — each axum WebSocket handler
//! subscribes to the broadcast channel independently.

use serde::{Deserialize, Serialize};

/// Events broadcast to all connected dashboard clients and SSE subscribers.
///
/// The runner and store publish these via `tokio::sync::broadcast`. Each
/// WebSocket/SSE consumer subscribes independently and filters by job ID.
/// The JSON wire format uses `{"type": "snake_case_variant", ...}` so
/// JavaScript clients can switch on the `type` field directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// Full state snapshot sent on initial WebSocket connection so the
    /// dashboard can render without a separate REST call.
    Snapshot {
        /// All known jobs as opaque JSON objects.
        jobs: Vec<serde_json::Value>,
        /// Server health/status as opaque JSON.
        health: serde_json::Value,
    },
    /// A job transitioned to a new status (e.g., queued -> running,
    /// running -> completed). Carries the full `JobInfo` as opaque JSON
    /// because the dashboard renders arbitrary fields.
    JobUpdate {
        /// Full `JobInfo` as opaque JSON.
        job: serde_json::Value,
    },
    /// A single file within a job changed status. Emitted by the runner
    /// after each file completes or fails, enabling per-file progress bars.
    /// `completed_files` is the running total so the client does not need
    /// to track it locally.
    FileUpdate {
        /// ID of the parent job.
        job_id: crate::api::JobId,
        /// Updated file status as opaque JSON.
        file: serde_json::Value,
        /// Running total of completed files in this job.
        completed_files: i64,
    },
    /// A job was permanently removed via `DELETE /jobs/{id}`. The dashboard
    /// uses this to remove the job row without re-fetching the full list.
    JobDeleted {
        /// ID of the deleted job.
        job_id: crate::api::JobId,
    },
}

/// Broadcast capacity — must be large enough to hold bursts of file updates
/// for large jobs without dropping messages for slow clients.
pub const BROADCAST_CAPACITY: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_event_job_update_json() {
        let event = WsEvent::JobUpdate {
            job: serde_json::json!({"job_id": "abc", "status": "running"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"job_update\""));
        assert!(json.contains("\"job_id\":\"abc\""));
    }

    #[test]
    fn ws_event_file_update_json() {
        let event = WsEvent::FileUpdate {
            job_id: "abc".into(),
            file: serde_json::json!({"filename": "test.cha", "status": "done"}),
            completed_files: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"file_update\""));
        assert!(json.contains("\"completed_files\":3"));
    }

    #[test]
    fn ws_event_snapshot_json() {
        let event = WsEvent::Snapshot {
            jobs: vec![serde_json::json!({"job_id": "abc"})],
            health: serde_json::json!({"status": "ok"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"snapshot\""));
    }

    #[test]
    fn ws_event_job_deleted_json() {
        let event = WsEvent::JobDeleted {
            job_id: "abc".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"job_deleted\""));
    }
}
