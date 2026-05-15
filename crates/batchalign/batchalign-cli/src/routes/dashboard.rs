//! SPA catch-all for `/dashboard` and `/dashboard/{rest}`.
//!
//! Serves pre-built dashboard SPA files (React static build). Resolution order:
//!
//! 1. `$BATCHALIGN_DASHBOARD_DIR` environment variable (development override)
//! 2. `~/.batchalign3/dashboard/` (user-installed, allows hot-updates)
//! 3. **Embedded in binary** — the `apps/batchalign/cli-web-statuspage/dist/` tree baked in at compile time,
//!    so every `batchalign3` binary ships a working dashboard with zero setup.
//!
//! Also redirects `/` -> `/dashboard`.

use std::path::PathBuf;

use axum::Router;
#[cfg(feature = "embed-dashboard")]
use axum::extract::Path;
#[cfg(feature = "embed-dashboard")]
use axum::http::{StatusCode, header};
use axum::response::{Html, Redirect};
#[cfg(feature = "embed-dashboard")]
use axum::response::{IntoResponse, Response};
use axum::routing::get;
#[cfg(feature = "embed-dashboard")]
use include_dir::{Dir, include_dir};
use tower_http::services::ServeDir;

use crate::config::RuntimeLayout;

/// Dashboard SPA files embedded at compile time from `apps/batchalign/cli-web-statuspage/dist/`.
///
/// Only available when built with `--features embed-dashboard` (requires
/// `bazel build //...-dashboard` first). Without this feature, the dashboard is
/// served from `$BATCHALIGN_DASHBOARD_DIR` or `~/.batchalign3/dashboard/`.
///
/// `DASHBOARD_DIST_DIR` is set by the build system:
///   - Cargo: `build.rs` sets it to `$CARGO_MANIFEST_DIR/../../../apps/.../dist`.
///   - Bazel: `rustc_env` in `BUILD.bazel` sets it to `$(execpath
///     //apps/batchalign/batchalign-cli-webdashboard:dist)`, which
///     resolves to the Vite TreeArtifact under `bazel-out/`.
/// Keeping the path indirect lets the same `include_dir!` literal work
/// against both layouts without diverging code paths.
#[cfg(feature = "embed-dashboard")]
static EMBEDDED_DASHBOARD: Dir<'_> = include_dir!("$DASHBOARD_DIST_DIR");

/// Find an on-disk dashboard directory, if one exists.
///
/// Checks the env-var override first, then the standard state directory.
/// Returns `None` if neither location has an `index.html`.
pub(crate) fn find_dashboard_dir_for(
    layout: &RuntimeLayout,
    env_override: Option<&str>,
) -> Option<PathBuf> {
    // Check env var first
    if let Some(dir) = env_override.map(str::trim).filter(|dir| !dir.is_empty()) {
        let path = PathBuf::from(dir);
        if path.join("index.html").exists() {
            return Some(path);
        }
    }

    // Check state dir (usually ~/.batchalign3/dashboard/)
    {
        let path = layout.dashboard_dir();
        if path.join("index.html").exists() {
            return Some(path);
        }
    }

    None
}

/// Infer MIME type from file extension.
#[cfg(feature = "embed-dashboard")]
fn mime_for_path(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else {
        "application/octet-stream"
    }
}

/// Serve a file from the embedded dashboard directory.
///
/// Falls back to `index.html` for SPA client-side routing (any path that
/// doesn't match a real embedded file gets the SPA shell).
#[cfg(feature = "embed-dashboard")]
fn serve_embedded(path: &str) -> Response {
    // Try exact file first
    if let Some(file) = EMBEDDED_DASHBOARD.get_file(path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime_for_path(path))],
            file.contents(),
        )
            .into_response();
    }

    // SPA fallback — serve index.html for client-side routing
    if let Some(index) = EMBEDDED_DASHBOARD.get_file("index.html") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            index.contents(),
        )
            .into_response();
    }

    // Should never happen — index.html is always embedded
    (
        StatusCode::NOT_FOUND,
        Html("Dashboard not found".to_string()),
    )
        .into_response()
}

/// Handler for `/dashboard` (no trailing path).
#[cfg(feature = "embed-dashboard")]
async fn embedded_dashboard_root() -> Response {
    serve_embedded("index.html")
}

/// Handler for `/dashboard/{rest}` — SPA catch-all with embedded files.
#[cfg(feature = "embed-dashboard")]
async fn embedded_dashboard_catchall(Path(rest): Path<String>) -> Response {
    serve_embedded(&rest)
}

/// Handler for `/assets/{rest}` — serves JS/CSS bundles from embedded files.
#[cfg(feature = "embed-dashboard")]
async fn embedded_assets(Path(rest): Path<String>) -> Response {
    let path = format!("assets/{rest}");
    if let Some(file) = EMBEDDED_DASHBOARD.get_file(&path) {
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime_for_path(&path)),
                // Asset filenames are content-hashed — safe to cache aggressively
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            file.contents(),
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

pub(crate) fn router_with_dashboard_dir<S>(dashboard_dir: Option<PathBuf>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if let Some(dashboard_dir) = dashboard_dir {
        // On-disk dashboard found — serve from filesystem (allows hot-updates)
        let serve_dir = ServeDir::new(&dashboard_dir).fallback(
            tower_http::services::ServeFile::new(dashboard_dir.join("index.html")),
        );
        // Dashboard bundles reference assets from absolute `/assets/...`.
        // Expose that mount so `/dashboard` can load JS/CSS successfully.
        let assets_dir = ServeDir::new(dashboard_dir.join("assets"));

        Router::new()
            .route("/", get(root_redirect))
            .nest_service("/assets", assets_dir)
            .nest_service("/dashboard", serve_dir)
    } else {
        router_without_disk_dashboard()
    }
}

/// Dashboard router when no on-disk directory is available.
///
/// With `embed-dashboard`: serves from the binary-embedded `apps/batchalign/cli-web-statuspage/dist/`.
/// Without: returns a simple HTML message directing the user to build or install it.
#[cfg(feature = "embed-dashboard")]
fn router_without_disk_dashboard<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(root_redirect))
        .route("/dashboard", get(embedded_dashboard_root))
        .route("/dashboard/{*rest}", get(embedded_dashboard_catchall))
        .route("/assets/{*rest}", get(embedded_assets))
}

#[cfg(not(feature = "embed-dashboard"))]
fn router_without_disk_dashboard<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    async fn no_dashboard() -> Html<&'static str> {
        Html(
            "<!doctype html><html><body>\
             <h1>Dashboard not available</h1>\
             <p>Build it with <code>bazel build //...-dashboard</code>, \
             or set <code>BATCHALIGN_DASHBOARD_DIR</code>.</p>\
             </body></html>",
        )
    }

    Router::new()
        .route("/", get(root_redirect))
        .route("/dashboard", get(no_dashboard))
        .route("/dashboard/{*rest}", get(no_dashboard))
}

/// Build the dashboard router, auto-detecting the SPA asset directory.
///
/// Resolution order:
/// 1. `$BATCHALIGN_DASHBOARD_DIR` env var (development override)
/// 2. `~/.batchalign3/dashboard/` (user-installed)
/// 3. Embedded in binary (always available)
///
/// The root path `/` always redirects to `/dashboard`.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let layout = RuntimeLayout::from_env();
    router_with_dashboard_dir(find_dashboard_dir_for(
        &layout,
        std::env::var("BATCHALIGN_DASHBOARD_DIR").ok().as_deref(),
    ))
}

async fn root_redirect() -> Redirect {
    Redirect::temporary("/dashboard")
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use axum::Router;

    async fn spawn_router(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener local_addr");
        let base = format!("http://{}", addr);
        let handle = tokio::spawn(async move {
            axum::serve(listener, router.into_make_service())
                .await
                .expect("serve test router");
        });
        (base, handle)
    }

    #[tokio::test]
    async fn root_redirects_to_dashboard() {
        let app = router_with_dashboard_dir::<()>(None);
        let (base, handle) = spawn_router(app).await;

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build reqwest client");
        let resp = client.get(format!("{base}/")).send().await.expect("GET /");

        assert_eq!(resp.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            resp.headers().get(reqwest::header::LOCATION),
            Some(&reqwest::header::HeaderValue::from_static("/dashboard"))
        );

        handle.abort();
    }

    #[tokio::test]
    async fn serves_dashboard_and_assets_routes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        fs::create_dir_all(root.join("assets")).expect("mkdir assets");
        fs::write(
            root.join("index.html"),
            "<!doctype html><script src=\"/./assets/app.js\"></script>",
        )
        .expect("write index");
        fs::write(root.join("assets/app.js"), "console.log('ok');").expect("write app.js");

        let app = router_with_dashboard_dir::<()>(Some(root.to_path_buf()));
        let (base, handle) = spawn_router(app).await;
        let client = reqwest::Client::new();

        let dashboard = client
            .get(format!("{base}/dashboard"))
            .send()
            .await
            .expect("GET /dashboard");
        assert_eq!(dashboard.status(), reqwest::StatusCode::OK);

        let asset = client
            .get(format!("{base}/assets/app.js"))
            .send()
            .await
            .expect("GET /assets/app.js");
        assert_eq!(asset.status(), reqwest::StatusCode::OK);

        handle.abort();
    }

    #[cfg(feature = "embed-dashboard")]
    #[tokio::test]
    async fn embedded_dashboard_serves_index() {
        // Force embedded mode by passing None (no on-disk dir)
        let app = router_with_dashboard_dir::<()>(None);
        let (base, handle) = spawn_router(app).await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{base}/dashboard"))
            .send()
            .await
            .expect("GET /dashboard (embedded)");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.expect("body text");
        assert!(
            body.contains("<!"),
            "expected HTML content from embedded index.html"
        );

        handle.abort();
    }

    #[cfg(feature = "embed-dashboard")]
    #[tokio::test]
    async fn embedded_dashboard_spa_fallback() {
        let app = router_with_dashboard_dir::<()>(None);
        let (base, handle) = spawn_router(app).await;
        let client = reqwest::Client::new();

        // Unknown SPA route should still return index.html (client-side routing)
        let resp = client
            .get(format!("{base}/dashboard/jobs/some-id"))
            .send()
            .await
            .expect("GET /dashboard/jobs/some-id (embedded SPA fallback)");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.expect("body text");
        assert!(body.contains("<!"), "SPA fallback should serve index.html");

        handle.abort();
    }

    #[cfg(feature = "embed-dashboard")]
    #[tokio::test]
    async fn embedded_assets_served_with_cache_headers() {
        let app = router_with_dashboard_dir::<()>(None);
        let (base, handle) = spawn_router(app).await;
        let client = reqwest::Client::new();

        // Find an actual embedded asset filename from the dir
        let asset_file = EMBEDDED_DASHBOARD
            .get_dir("assets")
            .and_then(|d| d.files().next())
            .map(|f| f.path().to_string_lossy().to_string());

        if let Some(asset_path) = asset_file {
            let filename = asset_path.strip_prefix("assets/").unwrap_or(&asset_path);
            let resp = client
                .get(format!("{base}/assets/{filename}"))
                .send()
                .await
                .expect("GET /assets/... (embedded)");
            assert_eq!(resp.status(), reqwest::StatusCode::OK);

            let cache = resp
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert!(
                cache.contains("immutable"),
                "content-hashed assets should have immutable cache headers"
            );
        }

        handle.abort();
    }

    #[test]
    fn runtime_layout_prefers_state_dashboard_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state_dir = tmp.path().join("state");
        let dashboard_dir = state_dir.join("dashboard");
        fs::create_dir_all(&dashboard_dir).expect("mkdir dashboard");
        fs::write(dashboard_dir.join("index.html"), "<!doctype html>").expect("write index");

        let layout = RuntimeLayout::from_state_dir(state_dir);
        assert_eq!(
            find_dashboard_dir_for(&layout, None),
            Some(dashboard_dir.to_path_buf())
        );
    }
}
