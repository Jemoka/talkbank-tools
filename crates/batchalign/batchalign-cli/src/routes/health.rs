//! `GET /health` — health check endpoint.
//!
//! Provides a point-in-time snapshot of server health for use by the CLI
//! (server discovery) and the dashboard (at-a-glance status). The response
//! includes worker availability, operational error counters, and loaded
//! pipeline summaries.

use std::sync::Arc;

use crate::api::{HealthResponse, HealthStatus, MemoryMb};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::AppState;
use crate::host_memory::{HostMemoryCoordinator, HostMemoryPressureLevel};

/// Build the health-check router (`GET /health`).
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health))
}

/// Return a point-in-time health snapshot used by the CLI for server discovery
/// and the dashboard for at-a-glance status.
///
/// The response includes worker availability, operational error counters
/// (crashes, forced-terminal errors, memory-gate aborts), and the list of
/// loaded pipelines so callers can decide whether this server is suitable
/// for a given command without probing individual workers.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server health snapshot", body = HealthResponse)
    )
)]
pub(crate) async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let control_plane = state.control.backend.control_plane_snapshot().await;
    let live_workers = state.workers.pool.worker_count().await as i64;
    let live_worker_keys = state.workers.pool.worker_keys().await;
    let worker_summary = state.workers.pool.worker_summary().await;

    // System memory snapshot for the dashboard memory panel.
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_mb = MemoryMb(sys.total_memory() / (1024 * 1024));
    let available_mb = MemoryMb(sys.available_memory() / (1024 * 1024));
    let used_mb = MemoryMb(total_mb.0.saturating_sub(available_mb.0));
    // `resolved_memory_gate_mb` applies the tier-derived fallback
    // when the operator has not set an explicit `Some(MemoryMb(_))`.
    let gate_mb = state.environment.config.resolved_memory_gate_mb();
    let (host_memory, host_memory_error) =
        match HostMemoryCoordinator::from_server_config(&state.environment.config).snapshot() {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => {
                tracing::warn!(error = %error, "Failed to read host-memory snapshot");
                (None, Some(error.to_string()))
            }
        };

    Json(HealthResponse {
        status: HealthStatus::Ok,
        version: state.build.version.clone(),
        node_id: control_plane.node_id,
        free_threaded: false, // Rust server dispatches to Python workers
        capabilities: state.workers.capabilities.clone(),
        loaded_pipelines: worker_summary,
        media_roots: state
            .environment
            .config
            .media_roots
            .iter()
            .map(|p| p.as_str().to_string())
            .collect(),
        media_mapping_keys: state
            .environment
            .config
            .media_mappings
            .keys()
            .map(|k| k.as_str().to_string())
            .collect(),
        workers_available: control_plane.workers_available,
        job_slots_available: control_plane.workers_available,
        live_workers,
        live_worker_keys,
        active_jobs: control_plane.active_jobs,
        cache_backend: "sqlite".into(),
        worker_crashes: control_plane.worker_crashes,
        attempts_started: control_plane.attempts_started,
        attempts_retried: control_plane.attempts_retried,
        deferred_work_units: control_plane.deferred_work_units,
        forced_terminal_errors: control_plane.forced_terminal_errors,
        memory_gate_aborts: control_plane.memory_gate_aborts,
        build_hash: state.build.build_hash.clone(),
        warmup_status: state.workers.pool.warmup_status(),
        system_memory_total_mb: total_mb,
        system_memory_available_mb: available_mb,
        system_memory_used_mb: used_mb,
        memory_gate_threshold_mb: gate_mb,
        host_memory_pressure: host_memory
            .as_ref()
            .map(|snapshot| snapshot.pressure_level)
            .unwrap_or(HostMemoryPressureLevel::Healthy),
        host_memory_reserved_mb: host_memory
            .as_ref()
            .map(|snapshot| snapshot.active_reserved_mb)
            .unwrap_or(MemoryMb(0)),
        host_memory_startup_leases: host_memory
            .as_ref()
            .map(|snapshot| snapshot.startup_leases as i64)
            .unwrap_or(0),
        host_memory_job_leases: host_memory
            .as_ref()
            .map(|snapshot| snapshot.job_execution_leases as i64)
            .unwrap_or(0),
        host_memory_ml_test_locks: host_memory
            .as_ref()
            .map(|snapshot| snapshot.ml_test_locks as i64)
            .unwrap_or(0),
        host_memory_active_leases: host_memory
            .as_ref()
            .map(|snapshot| snapshot.active_lease_labels.clone())
            .unwrap_or_default(),
        host_memory_error,
    })
}
