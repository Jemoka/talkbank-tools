//! Cancel-request and cancel-record API models.
//!
//! These types ride the wire on `POST /jobs/{id}/cancel` (request) and
//! `GET /jobs/{id}/cancellations` (record/audit history). They surface
//! provenance — who cancelled, from where, why — so server logs and the
//! TUI can attribute cancels to a specific caller instead of treating
//! every "Job finished status=cancelled" as anonymous.
//!
//! Schema lives in
//! `crates/batchalign-app/migrations/20260426163000_cancellation_provenance.sql`.
//! The wire-side newtypes (`CancelSource`, `CallerHost`, `CallerPid`,
//! `CancelReason`) live in `batchalign-types::domain`.

use serde::{Deserialize, Serialize};

use super::domain::{
    CallerHost, CallerPid, CancelReason, CancelSource, CorrelationId, DisplayPath, JobId,
    UnixTimestamp,
};

/// Optional metadata accompanying a `POST /jobs/{id}/cancel` request body.
///
/// All fields are optional so existing clients (empty POST body) continue
/// working. The route handler fills missing fields with sensible defaults:
/// `source = CancelSource::Api`, `host = <peer addr>`, others left null.
///
/// **Why optional rather than required:** the same endpoint is hit by the
/// TUI (which knows everything), the dashboard (which knows source + host),
/// scripted curl (which knows nothing), and the staging orchestrator
/// (which forwards a cancel from another server). Forcing required fields
/// would break all but the TUI; the route handler does enrichment instead.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct CancellationRequest {
    /// Where the cancel originated. Defaults to `Api` if unspecified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<CancelSource>,
    /// Caller's hostname or IP. Server fills with peer-addr if unspecified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CallerHost>,
    /// Caller process ID, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<CallerPid>,
    /// Free-form reason ("user-pressed-cancel", "ctrl-c-shutdown", etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CancelReason>,
    /// Caller-side correlation ID for cross-system tracing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<CorrelationId>,
    /// Filename being processed at cancel time (TUI snapshots from
    /// `runtime.state` so we can show "cancel pending; finishing X" in
    /// the cancel-feedback render).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_flight_filename: Option<DisplayPath>,
}

/// One row from the `cancellations` audit table, returned by
/// `GET /jobs/{id}/cancellations`.
///
/// Multiple rows are possible per job (a user may press cancel several
/// times if visible feedback lags — the 2026-04-25 Malayalam run had
/// two cancels exactly an hour apart). The `accepted` flag distinguishes
/// "cancel actually changed job state" from "cancel arrived after the
/// job was already terminal."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct CancellationRecord {
    /// Audit-row primary key.
    pub id: i64,
    /// Job that was the target of this cancel.
    pub job_id: JobId,
    /// Server-side timestamp when the cancel POST hit the route handler.
    pub requested_at: UnixTimestamp,
    /// Caller-reported source (or `Api` when the body was empty).
    pub source: CancelSource,
    /// Caller-reported host (may be empty / null).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CallerHost>,
    /// Caller-reported process ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<CallerPid>,
    /// Caller-reported reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CancelReason>,
    /// Caller-reported correlation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<CorrelationId>,
    /// File being processed at cancel time, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_flight_filename: Option<DisplayPath>,
    /// `true` if this cancel actually changed job state, `false` if the
    /// job was already terminal when the cancel arrived (recorded for
    /// forensics regardless).
    pub accepted: bool,
}

#[cfg(test)]
impl CancellationRecord {
    /// Construct a minimal `CancellationRecord` for use in unit tests.
    ///
    /// All fields that tests typically vary (source, reason, requested_at)
    /// are left at stub values so individual tests can override them via
    /// struct-update syntax (`..CancellationRecord::test_default()`).
    pub fn test_default() -> Self {
        Self {
            id: 0,
            job_id: JobId::from("test-job"),
            requested_at: UnixTimestamp(0.0),
            source: CancelSource::Api,
            host: None,
            pid: None,
            reason: None,
            correlation_id: None,
            in_flight_filename: None,
            accepted: true,
        }
    }
}
