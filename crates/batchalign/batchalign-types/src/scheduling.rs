//! Scheduling-specific shared domain types.
//!
//! These types belong to the control plane and should remain usable outside the
//! server crate so future fleet/orchestration layers do not need to depend on
//! `batchalign` just to name attempts or work units.

string_id!(
    /// Stable identifier for a single attempt to execute a work unit.
    pub AttemptId
);

string_id!(
    /// Opaque identifier for a work unit within a job.
    pub WorkUnitId
);
