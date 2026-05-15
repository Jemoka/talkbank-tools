//! App factory and server lifecycle (create, serve, shutdown).
//!
//! Server-specific code that depends on axum, sqlx (db), and temporal.
//! The shared worker-preparation logic lives in [`crate::worker_setup`].

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tracing::{info, warn};

use crate::cache::UtteranceCache;
use crate::config::{RuntimeLayout, ServerConfig};
use crate::db::JobDB;
use crate::error;
use crate::host_facts::HostFactsSource;
use crate::media::MediaResolver;
use crate::server_backend::{ServerBackendBootstrap, bootstrap_test_server_backend};
use crate::state::{
    AppBuildInfo, AppControlPlane, AppEnvironment, AppPaths, AppState, WorkerSubsystem,
};
use crate::temporal_backend::bootstrap_temporal_server_backend;
use crate::worker::pool::PoolConfig;
use crate::worker_setup::prepare_workers_background;

// Re-export engine-level types so existing `use crate::server::PreparedWorkers`
// paths continue to compile during migration. New code should import from
// `crate::worker_setup` directly.
pub use crate::worker_setup::{
    PreparedWorkers, WarmupTarget, prepare_direct_workers, prepare_workers,
};

/// Create the application: open DB, recover state, build router.
///
/// Returns `(Router, Arc<AppState>)` — the caller binds the router to a
/// TCP listener.
///
/// `db_dir` overrides the SQLite database directory (defaults to the runtime
/// state root, typically `~/.batchalign3/`). Useful for tests that need an
/// isolated DB.
pub async fn create_app(
    config: ServerConfig,
    pool_config: PoolConfig,
    jobs_dir: Option<String>,
    db_dir: Option<std::path::PathBuf>,
    build_hash: Option<String>,
) -> Result<(Router, Arc<AppState>), error::ServerError> {
    let layout = RuntimeLayout::from_env();
    create_app_with_runtime(config, pool_config, layout, jobs_dir, db_dir, build_hash).await
}

/// Create the application using an explicit runtime layout for state-owned
/// filesystem roots.
///
/// Warmup runs in the background so the HTTP port binds immediately.
pub async fn create_app_with_runtime(
    config: ServerConfig,
    pool_config: PoolConfig,
    layout: RuntimeLayout,
    jobs_dir: Option<String>,
    db_dir: Option<std::path::PathBuf>,
    build_hash: Option<String>,
) -> Result<(Router, Arc<AppState>), error::ServerError> {
    let workers = prepare_workers_background(&config, pool_config).await?;
    create_app_with_prepared_workers(config, layout, jobs_dir, db_dir, None, build_hash, workers)
        .await
}

/// Create the application with test-echo workers and the lightweight
/// [`TestServerBackend`](crate::server_backend::TestServerBackend).
///
/// This bypasses Temporal entirely. Use for integration tests that need a
/// real HTTP server with working job dispatch but no external dependencies.
pub async fn create_test_app(
    config: ServerConfig,
    pool_config: PoolConfig,
    jobs_dir: Option<String>,
    db_dir: Option<std::path::PathBuf>,
    build_hash: Option<String>,
) -> Result<(Router, Arc<AppState>), error::ServerError> {
    let layout = RuntimeLayout::from_env();
    let workers = prepare_workers_background(&config, pool_config).await?;
    create_test_app_with_prepared_workers(
        config, layout, jobs_dir, db_dir, None, build_hash, workers,
    )
    .await
}

/// Create the application with an already-prepared worker subsystem.
///
/// This keeps the expensive worker pool hot across repeated app lifecycles
/// while giving each app instance a fresh store, runtime supervisor, database,
/// and filesystem layout. `cache_dir` lets tests pin the utterance cache under
/// that owned runtime root instead of falling back to the ambient platform
/// cache directory.
pub async fn create_app_with_prepared_workers(
    config: ServerConfig,
    layout: RuntimeLayout,
    jobs_dir: Option<String>,
    db_dir: Option<std::path::PathBuf>,
    cache_dir: Option<std::path::PathBuf>,
    build_hash: Option<String>,
    workers: PreparedWorkers,
) -> Result<(Router, Arc<AppState>), error::ServerError> {
    let jobs_dir = jobs_dir.unwrap_or_else(|| layout.jobs_dir().to_string_lossy().into_owned());
    let _ = tokio::fs::create_dir_all(&jobs_dir).await;

    // Open database (includes schema migration)
    let db = JobDB::open_with_layout(&layout, db_dir.as_deref()).await?;

    // Recovery: mark interrupted, prune expired
    let interrupted = db.recover_interrupted().await?;
    if !interrupted.is_empty() {
        info!(count = interrupted.len(), "Recovered interrupted jobs");
    }
    let expired_dirs = db.prune_expired(config.job_ttl_days).await?;
    for d in &expired_dirs {
        let _ = tokio::fs::remove_dir_all(d).await;
    }

    // Initialize utterance cache (SQLite, shared with Python workers)
    // Must be before auto-resume so spawn_job can access it.
    let cache = Arc::new(
        UtteranceCache::tiered(cache_dir, None)
            .await
            .map_err(|e| error::ServerError::Validation(format!("cache init failed: {e}")))?,
    );
    let db = Arc::new(db);
    let execution_runtime = workers.resolve_execution_runtime(cache.clone())?;
    let backend_bootstrap: ServerBackendBootstrap = match config.temporal_backend() {
        crate::types::config::TemporalBackend::Server { .. } => {
            info!("Backend: temporal");
            bootstrap_temporal_server_backend(config.clone(), db, execution_runtime.engine).await?
        }
        crate::types::config::TemporalBackend::Disabled => {
            info!("Backend: local (Temporal disabled)");
            bootstrap_test_server_backend(
                config.clone(),
                db,
                execution_runtime.engine,
                std::path::PathBuf::from(&jobs_dir),
            )
            .await?
        }
    };
    if backend_bootstrap.loaded_jobs > 0 {
        info!(
            loaded = backend_bootstrap.loaded_jobs,
            "Jobs loaded from DB"
        );
    }
    let capability_snapshot = execution_runtime.capability_snapshot;
    let capabilities = capability_snapshot.capabilities;
    let infer_tasks = capability_snapshot.infer_tasks;
    let pool = workers.pool().clone();

    if backend_bootstrap.queued_jobs > 0 {
        info!(
            count = backend_bootstrap.queued_jobs,
            "Queued jobs recovered from DB — job_task runners spawned for each"
        );
    }

    let bug_reports_dir = layout.bug_reports_dir().to_string_lossy().into_owned();
    let dashboard_dir = crate::routes::dashboard::find_dashboard_dir_for(
        &layout,
        std::env::var("BATCHALIGN_DASHBOARD_DIR").ok().as_deref(),
    );

    let state = Arc::new(AppState {
        control: AppControlPlane {
            backend: backend_bootstrap.backend,
        },
        workers: WorkerSubsystem {
            pool,
            capabilities,
            infer_tasks,
        },
        environment: AppEnvironment {
            config,
            media: MediaResolver::new(),
            paths: AppPaths {
                jobs_dir,
                bug_reports_dir,
                dashboard_dir,
            },
        },
        build: AppBuildInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_hash: build_hash.unwrap_or_default(),
        },
    });

    let router = crate::routes::router(state.clone());
    Ok((router, state))
}

/// Create the application with prepared workers and the lightweight
/// [`TestServerBackend`](crate::server_backend::TestServerBackend).
///
/// Same lifecycle as [`create_app_with_prepared_workers`] but uses the
/// in-process test backend instead of Temporal.
pub async fn create_test_app_with_prepared_workers(
    config: ServerConfig,
    layout: RuntimeLayout,
    jobs_dir: Option<String>,
    db_dir: Option<std::path::PathBuf>,
    cache_dir: Option<std::path::PathBuf>,
    build_hash: Option<String>,
    workers: PreparedWorkers,
) -> Result<(Router, Arc<AppState>), error::ServerError> {
    let jobs_dir = jobs_dir.unwrap_or_else(|| layout.jobs_dir().to_string_lossy().into_owned());
    let _ = tokio::fs::create_dir_all(&jobs_dir).await;

    let db = JobDB::open_with_layout(&layout, db_dir.as_deref()).await?;
    let interrupted = db.recover_interrupted().await?;
    if !interrupted.is_empty() {
        info!(count = interrupted.len(), "Recovered interrupted jobs");
    }
    let expired_dirs = db.prune_expired(config.job_ttl_days).await?;
    for d in &expired_dirs {
        let _ = tokio::fs::remove_dir_all(d).await;
    }

    let cache = Arc::new(
        UtteranceCache::tiered(cache_dir, None)
            .await
            .map_err(|e| error::ServerError::Validation(format!("cache init failed: {e}")))?,
    );
    let db = Arc::new(db);
    let execution_runtime = workers.resolve_execution_runtime(cache.clone())?;
    let backend_bootstrap: ServerBackendBootstrap = bootstrap_test_server_backend(
        config.clone(),
        db,
        execution_runtime.engine,
        std::path::PathBuf::from(&jobs_dir),
    )
    .await?;

    if backend_bootstrap.loaded_jobs > 0 {
        info!(
            loaded = backend_bootstrap.loaded_jobs,
            "Jobs loaded from DB (test backend)"
        );
    }
    let capability_snapshot = execution_runtime.capability_snapshot;
    let capabilities = capability_snapshot.capabilities;
    let infer_tasks = capability_snapshot.infer_tasks;
    let pool = workers.pool().clone();

    let bug_reports_dir = layout.bug_reports_dir().to_string_lossy().into_owned();
    let dashboard_dir = crate::routes::dashboard::find_dashboard_dir_for(
        &layout,
        std::env::var("BATCHALIGN_DASHBOARD_DIR").ok().as_deref(),
    );

    let state = Arc::new(AppState {
        control: AppControlPlane {
            backend: backend_bootstrap.backend,
        },
        workers: WorkerSubsystem {
            pool,
            capabilities,
            infer_tasks,
        },
        environment: AppEnvironment {
            config,
            media: MediaResolver::new(),
            paths: AppPaths {
                jobs_dir,
                bug_reports_dir,
                dashboard_dir,
            },
        },
        build: AppBuildInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_hash: build_hash.unwrap_or_default(),
        },
    });

    let router = crate::routes::router(state.clone());
    Ok((router, state))
}

// ---------------------------------------------------------------------------
// Server lifecycle
// ---------------------------------------------------------------------------

/// Start serving on the configured host:port with graceful shutdown.
///
/// Listens for SIGINT and SIGTERM. On signal:
/// 1. Stops accepting new connections
/// 2. Waits for in-flight requests to complete
/// 3. Shuts down the worker pool (SIGTERM → wait → SIGKILL)
pub async fn serve(
    config: ServerConfig,
    pool_config: PoolConfig,
    build_hash: Option<String>,
) -> Result<(), error::ServerError> {
    let layout = RuntimeLayout::from_env();
    serve_with_runtime(config, pool_config, layout, build_hash).await
}

/// Start serving with an explicit runtime layout for state-owned paths.
///
/// Writes a PID file on startup so that `daemon.rs::detect_manual_server()`
/// can discover us, and removes it on exit (including after signal-driven
/// shutdown). This is the single lifecycle owner for foreground servers.
pub async fn serve_with_runtime(
    config: ServerConfig,
    pool_config: PoolConfig,
    layout: RuntimeLayout,
    build_hash: Option<String>,
) -> Result<(), error::ServerError> {
    let host = config.host.clone();
    let port = config.port;

    // Run config-vs-host-facts validation against the same live
    // facts the runtime resolved overrides against. Warnings surface
    // as `tracing::warn!` lines so operators see them in startup
    // logs; errors abort startup with a message that includes the
    // recommendation. The detect call is millisecond-scale and
    // happens once per startup.
    let host_facts = crate::host_facts::RealHostFactsSource.detect();
    let validation = crate::host_facts::validate(&config, &host_facts);
    for warning in &validation.warnings {
        warn!(
            target: "batchalign::host_facts::validate",
            "{warning}",
        );
    }
    if validation.has_errors() {
        let detail = validation
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(error::ServerError::Validation(format!(
            "host-facts validation refused startup: {detail}"
        )));
    }

    let (router, state) =
        create_app_with_runtime(config, pool_config, layout.clone(), None, None, build_hash)
            .await?;

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(error::ServerError::Io)?;
    info!(addr = %addr, "Server listening");

    // Write PID file so daemon.rs can discover this foreground server.
    // Best-effort: if the write fails (e.g. read-only filesystem), log
    // and continue -- the server still works, just won't be auto-discovered.
    let pid_path = layout.server_pid_path();
    if let Err(error) = write_pid_file(&pid_path) {
        warn!(path = %pid_path.display(), error = %error,
            "Failed to write server PID file; daemon auto-discovery may not work");
    }

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(error::ServerError::Io)?;

    info!("Server stopped, shutting down gracefully");

    // 1. Interrupt all active jobs for graceful shutdown.
    let interrupted = state.control.backend.interrupt_all_for_shutdown().await;
    if interrupted > 0 {
        info!(interrupted, "Interrupted active jobs for shutdown");
    }

    // 1b. Stop the queue dispatcher and await tracked job tasks.
    let shutdown_summary = state
        .control
        .backend
        .shutdown_runtime(tokio::time::Duration::from_secs(15))
        .await;
    match shutdown_summary {
        Ok(shutdown_summary) => {
            if shutdown_summary.timed_out {
                warn!(
                    remaining = shutdown_summary.remaining_jobs,
                    "Some job tasks did not finish in time"
                );
            } else if shutdown_summary.remaining_jobs > 0 {
                info!(
                    remaining = shutdown_summary.remaining_jobs,
                    "Job runtime shut down with remaining tracked jobs"
                );
            }
        }
        Err(error) => {
            warn!(error = %error, "Runtime supervisor failed to report shutdown status");
        }
    }

    // 3. Shut down the worker pool (gracefully shuts down all workers)
    state.workers.pool.shutdown().await;

    // 4. Remove PID file so stale detection works on next startup.
    remove_pid_file(&pid_path);
    info!("Shutdown complete");

    Ok(())
}

/// Wait for a shutdown signal (SIGINT or SIGTERM).
///
/// If signal handlers fail to install (rare, but possible in constrained
/// environments like containers without a proper init), the server logs the
/// failure and falls through to a pending future that never resolves --
/// meaning the server stays up until the process is killed externally. This
/// is safer than panicking the server on startup.
async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(error) => {
                warn!(error = %error, "Failed to install CTRL+C handler; \
                    server will not respond to SIGINT");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                warn!(error = %error, "Failed to install SIGTERM handler; \
                    server will not respond to SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("Received SIGINT, shutting down"),
        () = terminate => info!("Received SIGTERM, shutting down"),
    }
}

// ---------------------------------------------------------------------------
// PID file helpers
// ---------------------------------------------------------------------------

/// Write the current process PID to a file (atomic via temp + rename).
fn write_pid_file(path: &std::path::Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("pid.tmp");
    std::fs::write(&tmp, std::process::id().to_string())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Remove a PID file. Best-effort: missing file is not an error.
fn remove_pid_file(path: &std::path::Path) {
    match std::fs::remove_file(path) {
        Ok(()) => info!(path = %path.display(), "Removed server PID file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            warn!(path = %path.display(), error = %error,
                "Failed to remove server PID file");
        }
    }
}
