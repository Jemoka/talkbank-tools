//! HTTP route handlers — composing all sub-routers with middleware.

pub mod bug_reports;
pub mod dashboard;
pub mod health;
pub mod jobs;
pub mod media_list;
pub mod traces;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Build the main router with all routes and middleware.
///
/// Layer order (bottom-up execution, outermost runs first):
/// 1. CORS — outermost, runs first on every request
/// 2. Body limit — reject oversized requests early (from config `max_body_bytes_mb`)
/// 3. Catch panic — convert panics to 500 instead of connection reset
/// 4. Timeout — 5-minute request timeout
/// 5. Trace — structured request/response logging with latency
/// 6. Compression — gzip/brotli response compression (innermost)
pub fn router(state: Arc<AppState>) -> Router {
    let max_body_bytes = state.environment.config.max_body_bytes_mb.0 as usize * 1024 * 1024;

    Router::new()
        .merge(health::router())
        .merge(jobs::router())
        .merge(media_list::router())
        .merge(bug_reports::router())
        .merge(dashboard::router_with_dashboard_dir(
            state.environment.paths.dashboard_dir.clone(),
        ))
        .merge(traces::router())
        .merge(crate::ws_route(state.clone()))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(CatchPanicLayer::new())
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
