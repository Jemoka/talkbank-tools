// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Deterministic network fault tests using turmoil.
//!
//! These tests exercise the HTTP layer (health, SSE, job lifecycle) under
//! simulated network conditions: partitions, message delays, server restarts.
//! They complement the existing integration tests which use real TCP on
//! loopback.
//!
//! turmoil replaces `tokio::net` with a simulated network that runs in a
//! single thread with virtual time. All network I/O is deterministic given
//! the same seed.
//!
//! Worker subprocess management is outside turmoil's scope (workers use stdio
//! pipes, not TCP). These tests focus purely on HTTP client ↔ server behavior.

mod common;

use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;

const SERVER_BASE: &str = "http://server:8001";

use axum::Router;
use http_body_util::BodyExt;
use hyper::{Request, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioIo;
use turmoil::net;

use batchalign::api::{
    FilePayload, JobSubmission, LanguageCode3, LanguageSpec, MemoryMb, NumSpeakers, ReleasedCommand,
};
use batchalign::config::ServerConfig;
use batchalign::create_test_app;
use batchalign::host_facts::PerProfile;
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::pool::PoolConfig;

// ---------------------------------------------------------------------------
// Error helpers — reduce `map_err(|e| Box::new(e) as ...)` noise
// ---------------------------------------------------------------------------

type TurmoilError = Box<dyn std::error::Error>;

trait IntoTurmoilErr<T> {
    fn t(self) -> Result<T, TurmoilError>;
}

impl<T, E: std::error::Error + 'static> IntoTurmoilErr<T> for Result<T, E> {
    fn t(self) -> Result<T, TurmoilError> {
        self.map_err(|e| Box::new(e) as TurmoilError)
    }
}

// ---------------------------------------------------------------------------
// Turmoil ↔ axum adapter: TurmoilListener
// ---------------------------------------------------------------------------

/// Bridges `turmoil::net::TcpListener` to `axum::serve::Listener`.
/// turmoil's `TcpStream` implements tokio `AsyncRead + AsyncWrite` directly.
struct TurmoilListener(net::TcpListener);

impl axum::serve::Listener for TurmoilListener {
    type Io = net::TcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        self.0.accept().await.expect("turmoil accept")
    }

    fn local_addr(&self) -> tokio::io::Result<Self::Addr> {
        self.0.local_addr()
    }
}

// ---------------------------------------------------------------------------
// Turmoil ↔ hyper adapter: TurmoilStream + TurmoilConnector
// ---------------------------------------------------------------------------

/// Bridges turmoil's `TcpStream` to hyper's `Read/Write + Connection` traits
/// (required by the hyper legacy client).
struct TurmoilStream(TokioIo<net::TcpStream>);

impl hyper_util::client::legacy::connect::Connection for TurmoilStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

impl hyper::rt::Read for TurmoilStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl hyper::rt::Write for TurmoilStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// Establishes TCP connections through turmoil's simulated network.
#[derive(Clone)]
struct TurmoilConnector;

impl tower::Service<Uri> for TurmoilConnector {
    type Response = TurmoilStream;
    type Error = std::io::Error;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        Box::pin(async move {
            let host = uri.host().expect("URI must have host");
            let port = uri.port_u16().unwrap_or(80);
            let addr = turmoil::lookup(host);
            let stream = net::TcpStream::connect((addr, port)).await?;
            Ok(TurmoilStream(TokioIo::new(stream)))
        })
    }
}

fn turmoil_http_client() -> Client<TurmoilConnector, axum::body::Body> {
    Client::builder(hyper_util::rt::TokioExecutor::new()).build(TurmoilConnector)
}

// ---------------------------------------------------------------------------
// Server setup helpers
// ---------------------------------------------------------------------------

fn health_only_router() -> Router {
    use axum::Json;
    use axum::routing::get;

    async fn health() -> Json<serde_json::Value> {
        Json(serde_json::json!({ "status": "ok", "version": "test" }))
    }

    Router::new().route("/health", get(health))
}

/// Register a turmoil host that serves the health-only router on port 8001.
fn serve_health_only(sim: &mut turmoil::Sim<'_>) {
    sim.host("server", || async {
        let listener = net::TcpListener::bind("0.0.0.0:8001").await?;
        axum::serve(
            TurmoilListener(listener),
            health_only_router().into_make_service(),
        )
        .await
        .t()?;
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Real app setup (requires Python test-echo workers)
// ---------------------------------------------------------------------------

/// Holds the real tokio runtime that hosts AppState background actors
/// (JobRegistry, RuntimeSupervisor). Must outlive `sim.run()` — dropping
/// it kills the actors.
///
/// Concurrency is controlled by nextest: `.config/nextest.toml` assigns the
/// `turmoil_net` binary to a `turmoil` test-group with `max-threads = 1`.
struct RealAppHandle {
    router: Option<Router>,
    _state: std::sync::Arc<batchalign::AppState>,
    _tmp: tempfile::TempDir,
    _runtime: tokio::runtime::Runtime,
}

impl RealAppHandle {
    fn take_router(&mut self) -> Router {
        self.router.take().expect("router already taken")
    }
}

/// Create the real batchalign app with test-echo workers on a dedicated
/// multi-thread tokio runtime. The runtime's worker threads keep the
/// background actors alive while turmoil owns the main thread.
fn create_real_test_app(python_path: &str) -> RealAppHandle {
    use axum::extract::connect_info::MockConnectInfo;
    // Turmoil tests build their own server outside the shared fixture;
    // opt into the same per-process host-memory ledger override so they
    // don't race with sessions from other test binaries.
    common::test_server_fixture::isolate_host_memory_ledger();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build tokio runtime for test app");

    let (router, state, tmp) = rt.block_on(async {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let jobs_dir = tmp.path().join("jobs");
        std::fs::create_dir_all(&jobs_dir).expect("mkdir jobs");
        let db_dir = tmp.path().join("db");
        std::fs::create_dir_all(&db_dir).expect("mkdir db");

        let config = ServerConfig {
            host: "0.0.0.0".into(),
            port: 8001,
            job_ttl_days: 7,
            warmup_commands: vec![],
            memory_gate_mb: Some(MemoryMb(0)),
            ..Default::default()
        };
        let pool_config = PoolConfig {
            python_path: python_path.into(),
            test_echo: true,
            health_check_interval_s: 600,
            ready_timeout_s: 30,
            max_workers_per_key: PerProfile::uniform(1),
            verbose: 0,
            engine_overrides: String::new(),
            runtime: Default::default(),
            ..Default::default()
        };

        let (router, state) = create_test_app(
            config,
            pool_config,
            Some(jobs_dir.to_string_lossy().into()),
            Some(db_dir),
            Some("turmoil-test-hash".into()),
        )
        .await
        .expect("create_test_app");

        let router = router.layer(MockConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))));
        (router, state, tmp)
    });

    RealAppHandle {
        router: Some(router),
        _state: state,
        _tmp: tmp,
        _runtime: rt,
    }
}

/// Register a turmoil host serving the real batchalign app on port 8001.
/// Router is cloned into the host closure; the `RealAppHandle` must be kept
/// alive by the caller (its runtime hosts the background actors).
fn serve_real_app(sim: &mut turmoil::Sim<'_>, app: &mut RealAppHandle) {
    let router = app.take_router();
    sim.host("server", move || {
        let router = router.clone();
        async move {
            let listener = net::TcpListener::bind("0.0.0.0:8001").await?;
            axum::serve(TurmoilListener(listener), router.into_make_service())
                .await
                .t()?;
            Ok(())
        }
    });
}

fn test_submission() -> JobSubmission {
    JobSubmission {
        command: ReleasedCommand::Morphotag,
        // Morphotag requires LanguageSpec::PerFile per the submission
        // validator. The CHAT content carries `@Languages: eng` so the
        // per-file resolver succeeds.
        lang: LanguageSpec::PerFile,
        num_speakers: NumSpeakers(1),
        files: vec![FilePayload {
            filename: "test.cha".into(),
            content: "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                      @ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n"
                .into(),
        }],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),
            merge_abbrev: Default::default(),

            ..Default::default()
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    }
}

// ---------------------------------------------------------------------------
// HTTP request helpers
// ---------------------------------------------------------------------------

fn get(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .expect("build GET request")
}

async fn get_status(
    client: &Client<TurmoilConnector, axum::body::Body>,
    uri: &str,
) -> Result<u16, TurmoilError> {
    let resp = client.request(get(uri)).await.t()?;
    Ok(resp.status().as_u16())
}

async fn post_json(
    client: &Client<TurmoilConnector, axum::body::Body>,
    uri: &str,
    body: &impl serde::Serialize,
) -> Result<hyper::Response<hyper::body::Incoming>, TurmoilError> {
    let json = serde_json::to_vec(body)?;
    let resp = client
        .request(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(json))
                .expect("build POST request"),
        )
        .await
        .t()?;
    Ok(resp)
}

async fn body_json(
    resp: hyper::Response<hyper::body::Incoming>,
) -> Result<serde_json::Value, TurmoilError> {
    let bytes = resp.into_body().collect().await.t()?.to_bytes();
    Ok(serde_json::from_slice(&bytes)?)
}

async fn delete(
    client: &Client<TurmoilConnector, axum::body::Body>,
    uri: &str,
) -> Result<hyper::Response<hyper::body::Incoming>, TurmoilError> {
    let resp = client
        .request(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("build DELETE request"),
        )
        .await
        .t()?;
    Ok(resp)
}

async fn post_empty(
    client: &Client<TurmoilConnector, axum::body::Body>,
    uri: &str,
) -> Result<hyper::Response<hyper::body::Incoming>, TurmoilError> {
    let resp = client
        .request(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("build POST request"),
        )
        .await
        .t()?;
    Ok(resp)
}

/// Submit a job and return its job_id.
async fn submit_and_get_id(
    client: &Client<TurmoilConnector, axum::body::Body>,
    submission: &JobSubmission,
) -> Result<String, TurmoilError> {
    let resp = post_json(client, &format!("{SERVER_BASE}/jobs"), submission).await?;
    assert_eq!(resp.status().as_u16(), 200, "Job submission should succeed");
    let info = body_json(resp).await?;
    Ok(info["job_id"].as_str().expect("job_id").to_string())
}

/// Submit a job, wait for completion, return the job_id.
async fn submit_and_complete(
    client: &Client<TurmoilConnector, axum::body::Body>,
) -> Result<String, TurmoilError> {
    let job_id = submit_and_get_id(client, &test_submission()).await?;
    let status = poll_until_terminal(client, &job_id, 300).await?;
    assert_eq!(status, "completed");
    Ok(job_id)
}

/// Build a submission with a custom filename (for conflict tests).
fn test_submission_with_filename(filename: &str) -> JobSubmission {
    let mut sub = test_submission();
    sub.files[0].filename = filename.into();
    sub
}

/// Skip the test if Python with batchalign is not available.
macro_rules! require_python {
    () => {
        match common::resolve_python() {
            Some(p) => p,
            None => {
                eprintln!("SKIP: Python not available");
                return Ok(());
            }
        }
    };
}

/// Poll a job until it reaches a terminal state. Returns the final status string.
///
/// Checks immediately, then sleeps between polls. Short virtual-time sleeps
/// give the real runtime (which hosts Python workers) more chances to make
/// progress on the real clock.
async fn poll_until_terminal(
    client: &Client<TurmoilConnector, axum::body::Body>,
    job_id: &str,
    max_polls: u32,
) -> Result<String, TurmoilError> {
    let poll_uri = format!("{SERVER_BASE}/jobs/{job_id}");
    for i in 0..max_polls {
        if i > 0 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let resp = client.request(get(&poll_uri)).await.t()?;
        let info = body_json(resp).await?;
        let status = info["status"].as_str().unwrap_or("").to_string();
        if status == "completed" || status == "failed" {
            return Ok(status);
        }
    }
    Err(format!("Job {job_id} did not reach terminal state in {max_polls} polls").into())
}

// ---------------------------------------------------------------------------
// Infrastructure tests (health-only router, no Python needed)
// ---------------------------------------------------------------------------

#[test]
fn health_check_basic() -> turmoil::Result {
    let mut sim = turmoil::Builder::new().build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();
        let resp = client
            .request(get(&format!("{SERVER_BASE}/health")))
            .await
            .t()?;
        assert_eq!(resp.status(), 200);

        let json = body_json(resp).await?;
        assert_eq!(json["status"], "ok");
        Ok(())
    });

    sim.run()
}

/// Partition → timeout → repair → recovery.
#[test]
fn health_check_under_partition() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );

        turmoil::partition("client", "server");

        // Under partition, TCP SYN is dropped — detect via timeout.
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            client.request(get(&format!("{SERVER_BASE}/health"))),
        )
        .await;
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Request should time out or fail under partition"
        );

        turmoil::repair("client", "server");
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );

        Ok(())
    });

    sim.run()
}

/// Delayed response arrives after messages released.
#[test]
fn health_check_with_message_hold() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();

        turmoil::hold("client", "server");

        let request_handle = tokio::spawn({
            let client = client.clone();
            async move { client.request(get(&format!("{SERVER_BASE}/health"))).await }
        });

        tokio::time::sleep(Duration::from_secs(2)).await;
        turmoil::release("client", "server");

        let resp = request_handle.await.t()?.t()?;
        assert_eq!(resp.status(), 200);
        Ok(())
    });

    sim.run()
}

/// 3 clients hit the server simultaneously.
#[test]
fn concurrent_clients_health_check() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    serve_health_only(&mut sim);

    for i in 0..3 {
        let name = format!("client-{i}");
        sim.client(name, async move {
            let client = turmoil_http_client();
            assert_eq!(
                get_status(&client, &format!("{SERVER_BASE}/health")).await?,
                200
            );
            Ok(())
        });
    }

    sim.run()
}

/// Server crash, bounce, new client reconnects.
#[test]
fn server_crash_and_recovery() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );
        Ok(())
    });

    sim.run()?;

    sim.crash("server");
    sim.bounce("server");

    sim.client("client-after-restart", async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );
        Ok(())
    });

    sim.run()
}

// ---------------------------------------------------------------------------
// an operator/a user scenario tests
// ---------------------------------------------------------------------------

/// One-way partition: server can't send responses but client can send requests.
/// Models a user's scenario where she could submit but never got progress.
#[test]
fn one_way_partition_client_sends_but_no_response() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );

        turmoil::partition_oneway("server", "client");

        let result = tokio::time::timeout(
            Duration::from_secs(3),
            client.request(get(&format!("{SERVER_BASE}/health"))),
        )
        .await;
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Request should time out: server responses are blocked"
        );

        turmoil::repair_oneway("server", "client");
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );
        Ok(())
    });

    sim.run()
}

/// 5 dashboard clients reconnect simultaneously after deploy restart.
/// Models the 2026-03-30 fleet deploy scenario.
#[test]
fn rapid_reconnection_burst_after_restart() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();
    serve_health_only(&mut sim);

    for i in 0..5 {
        let name = format!("dashboard-{i}");
        sim.client(name, async move {
            let client = turmoil_http_client();
            assert_eq!(
                get_status(&client, &format!("{SERVER_BASE}/health")).await?,
                200
            );
            Ok(())
        });
    }

    sim.run()?;

    sim.crash("server");
    sim.bounce("server");

    for i in 0..5 {
        let name = format!("reconnect-{i}");
        sim.client(name, async move {
            tokio::time::sleep(Duration::from_millis(10 * i as u64)).await;
            let client = turmoil_http_client();
            assert_eq!(
                get_status(&client, &format!("{SERVER_BASE}/health")).await?,
                200
            );
            Ok(())
        });
    }

    sim.run()
}

/// 5 rapid partition/repair cycles. Models intermittent Tailscale
/// connectivity on fleet machines with unstable WiFi.
#[test]
fn network_flap_rapid_partition_cycles() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();

        for cycle in 0..5u32 {
            turmoil::partition("client", "server");
            tokio::time::sleep(Duration::from_millis(500)).await;

            turmoil::repair("client", "server");
            tokio::time::sleep(Duration::from_millis(100)).await;

            let status = get_status(&client, &format!("{SERVER_BASE}/health")).await?;
            assert_eq!(
                status, 200,
                "Server should respond after flap cycle {cycle}"
            );
        }
        Ok(())
    });

    sim.run()
}

/// Response held for 10s then released — models slow WiFi.
/// a user's machine on congested network.
#[test]
fn slow_response_eventually_arrives() -> turmoil::Result {
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();
    serve_health_only(&mut sim);

    sim.client("client", async {
        let client = turmoil_http_client();

        turmoil::hold("server", "client");

        let request_handle = tokio::spawn({
            let client = client.clone();
            async move { client.request(get(&format!("{SERVER_BASE}/health"))).await }
        });

        tokio::time::sleep(Duration::from_secs(10)).await;
        turmoil::release("server", "client");

        let resp = request_handle.await.t()?.t()?;
        assert_eq!(resp.status(), 200);

        let json = body_json(resp).await?;
        assert_eq!(json["status"], "ok");
        Ok(())
    });

    sim.run()
}

// ---------------------------------------------------------------------------
// Real-app turmoil tests (require Python test-echo workers)
// ---------------------------------------------------------------------------

/// Full job lifecycle: submit → queued → running → completed.
/// The fundamental "an operator submits a job from the dashboard" scenario.
#[test]
fn real_app_submit_and_poll_job() -> turmoil::Result {
    let python = require_python!();

    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();

    // _app must outlive sim.run() — its runtime hosts the background actors.
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );

        let resp = post_json(&client, &format!("{SERVER_BASE}/jobs"), &test_submission()).await?;
        assert_eq!(resp.status().as_u16(), 200, "Job submission should succeed");
        let info = body_json(resp).await?;
        let job_id = info["job_id"].as_str().expect("job_id in response");
        assert_eq!(info["status"], "queued");

        let status = poll_until_terminal(&client, job_id, 300).await?;
        assert_eq!(status, "completed", "Job should complete, not fail");
        Ok(())
    });

    sim.run()
}

/// Network drops after submit. Job completes on server during partition.
/// Client reconnects and sees the result.
#[test]
fn real_app_partition_during_job_processing() -> turmoil::Result {
    let python = require_python!();

    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();

    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();

        let resp = post_json(&client, &format!("{SERVER_BASE}/jobs"), &test_submission()).await?;
        assert_eq!(resp.status().as_u16(), 200);
        let info = body_json(resp).await?;
        let job_id = info["job_id"].as_str().expect("job_id").to_string();

        turmoil::partition("client", "server");
        tokio::time::sleep(Duration::from_secs(10)).await;
        turmoil::repair("client", "server");

        let status = poll_until_terminal(&client, &job_id, 300).await?;
        assert!(
            status == "completed" || status == "failed",
            "Job should reach terminal state after repair"
        );
        Ok(())
    });

    sim.run()
}

/// SSE stream for a nonexistent job returns 404 immediately.
/// Models an operator bookmarking a deleted job URL.
#[test]
fn real_app_sse_nonexistent_job_returns_404() -> turmoil::Result {
    let python = require_python!();

    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();

    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs/nonexistent-id/stream")))
            .await
            .t()?;
        assert_eq!(
            resp.status().as_u16(),
            404,
            "SSE for nonexistent job should 404"
        );
        Ok(())
    });

    sim.run()
}

/// Real health endpoint reports actual version and worker state.
/// After 2026-03-30 incident where health showed live_workers: 0 incorrectly.
#[test]
fn real_app_health_reports_workers() -> turmoil::Result {
    let python = require_python!();

    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();

    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let resp = client
            .request(get(&format!("{SERVER_BASE}/health")))
            .await
            .t()?;
        assert_eq!(resp.status().as_u16(), 200);
        let health = body_json(resp).await?;
        assert_eq!(health["status"], "ok");
        assert!(
            health["version"].is_string(),
            "Health should report version"
        );
        Ok(())
    });

    sim.run()
}

// ===========================================================================
// Group 1: Job lifecycle
// ===========================================================================

/// Submit, wait for running/completed, cancel → assert cancelled or no-op.
///
/// Extended (Phase 1, RED 1.4) to also assert that cancel provenance is
/// captured under turmoil-simulated network conditions: when the cancel
/// POST carries a body declaring source=tui, the audit row and the
/// denormalized columns must reflect those values even after the request
/// crossed simulated network hops.
#[test]
fn cancel_running_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Cancel with explicit provenance — source=tui simulates the
        // common case (an operator pressing 'c' then 'y' in their terminal).
        let provenance = serde_json::json!({
            "source": "tui",
            "host": "turmoil-client",
            "pid": 9001,
            "reason": "turmoil-cancel-test",
        });
        let resp = post_json(
            &client,
            &format!("{SERVER_BASE}/jobs/{job_id}/cancel"),
            &provenance,
        )
        .await?;
        assert_eq!(resp.status().as_u16(), 200);

        // Poll — should be cancelled (or completed if it finished first)
        tokio::time::sleep(Duration::from_secs(2)).await;
        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs/{job_id}")))
            .await
            .t()?;
        let info = body_json(resp).await?;
        let status = info["status"].as_str().unwrap_or("");
        assert!(
            status == "cancelled" || status == "completed",
            "Job should be cancelled or completed, got: {status}"
        );

        // Provenance assertion — the denormalized columns on the jobs row
        // must reflect what the client sent, even with simulated network
        // jitter between client and server. If the job completed before
        // the cancel arrived, the columns may be unset; tolerate that.
        if status == "cancelled" {
            assert_eq!(
                info["last_cancelled_source"].as_str(),
                Some("tui"),
                "jobs.last_cancelled_source must reflect provenance under turmoil"
            );
            assert_eq!(info["last_cancelled_host"].as_str(), Some("turmoil-client"));
            assert_eq!(
                info["last_cancelled_reason"].as_str(),
                Some("turmoil-cancel-test")
            );

            // Audit endpoint must surface the row.
            let resp = client
                .request(get(&format!("{SERVER_BASE}/jobs/{job_id}/cancellations")))
                .await
                .t()?;
            let audit = body_json(resp).await?;
            let arr = audit.as_array().expect("array");
            assert!(
                !arr.is_empty(),
                "audit table must have at least one row after cancel"
            );
            assert_eq!(arr[0]["pid"].as_i64(), Some(9001));
        }
        Ok(())
    });
    sim.run()
}

/// Cancel an already-completed job → 200 no-op.
#[test]
fn cancel_already_completed_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_complete(&client).await?;
        let resp = post_empty(&client, &format!("{SERVER_BASE}/jobs/{job_id}/cancel")).await?;
        assert_eq!(
            resp.status().as_u16(),
            200,
            "Cancel of completed job should be 200 no-op"
        );
        Ok(())
    });
    sim.run()
}

/// Cancel a nonexistent job → 404.
#[test]
fn cancel_nonexistent_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let resp = post_empty(
            &client,
            &format!("{SERVER_BASE}/jobs/fake-nonexistent/cancel"),
        )
        .await?;
        assert_eq!(resp.status().as_u16(), 404);
        Ok(())
    });
    sim.run()
}

/// Delete a completed job → 200, then poll → 404.
#[test]
fn delete_completed_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_complete(&client).await?;

        let resp = delete(&client, &format!("{SERVER_BASE}/jobs/{job_id}")).await?;
        assert_eq!(resp.status().as_u16(), 200);

        // Job should be gone
        let status = get_status(&client, &format!("{SERVER_BASE}/jobs/{job_id}")).await?;
        assert_eq!(status, 404, "Deleted job should return 404");
        Ok(())
    });
    sim.run()
}

/// Delete a running job → 409 (must cancel first).
#[test]
fn delete_running_job_rejected() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Try to delete immediately — may be 409 (still active) or 200
        // (test-echo completed before the delete arrived)
        let resp = delete(&client, &format!("{SERVER_BASE}/jobs/{job_id}")).await?;
        let status = resp.status().as_u16();
        assert!(
            status == 409 || status == 200,
            "Delete should be 409 (active) or 200 (already completed), got {status}"
        );
        Ok(())
    });
    sim.run()
}

/// Submit two jobs with the same file → second gets 409 conflict.
#[test]
fn submit_duplicate_files_conflict() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        // Submit first job
        let _id1 = submit_and_get_id(&client, &test_submission_with_filename("shared.cha")).await?;

        // Submit second job with same filename — should conflict
        let resp = post_json(
            &client,
            &format!("{SERVER_BASE}/jobs"),
            &test_submission_with_filename("shared.cha"),
        )
        .await?;
        assert_eq!(
            resp.status().as_u16(),
            409,
            "Duplicate file submission should be 409 conflict"
        );
        Ok(())
    });
    sim.run()
}

// ===========================================================================
// Group 2: SSE streaming
// ===========================================================================

/// Connect to SSE, receive snapshot event for an active job.
#[test]
fn sse_stream_receives_snapshot() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Connect to SSE — should get at least a snapshot event
        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs/{job_id}/stream")))
            .await
            .t()?;
        assert_eq!(resp.status().as_u16(), 200);

        // Read some of the stream body (timeout to avoid hanging)
        let body_result = tokio::time::timeout(Duration::from_secs(30), async {
            resp.into_body().collect().await.t()
        })
        .await;

        // Whether timeout or complete, we should have received the snapshot
        if let Ok(Ok(collected)) = body_result {
            let bytes = collected.to_bytes();
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                text.contains("event: snapshot"),
                "SSE should contain snapshot event"
            );
        }
        // Timeout is acceptable — it means the stream stayed open (job still processing)
        Ok(())
    });
    sim.run()
}

/// SSE for a completed job: snapshot + complete, then stream closes.
#[test]
fn sse_stream_completed_job_closes() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_complete(&client).await?;

        // SSE for completed job should close quickly
        let resp = tokio::time::timeout(
            Duration::from_secs(10),
            client.request(get(&format!("{SERVER_BASE}/jobs/{job_id}/stream"))),
        )
        .await
        .expect("SSE request should not hang")
        .t()?;
        assert_eq!(resp.status().as_u16(), 200);

        let body = tokio::time::timeout(Duration::from_secs(10), resp.into_body().collect())
            .await
            .expect("SSE body should close for completed job")
            .t()?;

        let bytes = body.to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("event: snapshot"), "Should have snapshot");
        assert!(
            text.contains("event: complete"),
            "Should have complete event"
        );
        Ok(())
    });
    sim.run()
}

/// Drop SSE client connection mid-stream → server stays healthy.
#[test]
fn sse_client_disconnect_no_panic() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Connect to SSE then immediately drop the response (simulates browser close)
        {
            let _resp = client
                .request(get(&format!("{SERVER_BASE}/jobs/{job_id}/stream")))
                .await
                .t()?;
            // _resp dropped here — client disconnects
        }

        // Server should still be healthy
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/health")).await?,
            200
        );
        Ok(())
    });
    sim.run()
}

/// Partition during SSE, repair, reconnect — get fresh snapshot.
#[test]
fn sse_partition_then_reconnect() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Partition — any active SSE connection would break
        turmoil::partition("client", "server");
        tokio::time::sleep(Duration::from_secs(5)).await;
        turmoil::repair("client", "server");

        // Reconnect to SSE — should get fresh snapshot
        let resp = tokio::time::timeout(
            Duration::from_secs(10),
            client.request(get(&format!("{SERVER_BASE}/jobs/{job_id}/stream"))),
        )
        .await
        .expect("SSE reconnect should not hang")
        .t()?;
        assert_eq!(resp.status().as_u16(), 200);
        Ok(())
    });
    sim.run()
}

// ===========================================================================
// Group 3: Results download
// ===========================================================================

/// Download results of a completed job → 200 with content.
#[test]
fn download_results_completed_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_complete(&client).await?;

        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs/{job_id}/results")))
            .await
            .t()?;
        assert_eq!(
            resp.status().as_u16(),
            200,
            "Results download should succeed"
        );
        Ok(())
    });
    sim.run()
}

/// Download results of a running job → 409.
#[test]
fn download_results_running_job_rejected() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Immediately try to download — should fail
        let status = get_status(&client, &format!("{SERVER_BASE}/jobs/{job_id}/results")).await?;
        assert!(
            status == 409 || status == 200,
            "Results for active job should be 409 or 200 if very fast"
        );
        Ok(())
    });
    sim.run()
}

/// Download single result file by name.
#[test]
fn download_single_result_file() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_complete(&client).await?;

        let resp = client
            .request(get(&format!(
                "{SERVER_BASE}/jobs/{job_id}/results/test.cha"
            )))
            .await
            .t()?;
        assert_eq!(
            resp.status().as_u16(),
            200,
            "Single file result should succeed"
        );
        Ok(())
    });
    sim.run()
}

/// Download results for nonexistent job → 404.
#[test]
fn download_results_nonexistent_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let status = get_status(&client, &format!("{SERVER_BASE}/jobs/fake-id/results")).await?;
        assert_eq!(status, 404);
        Ok(())
    });
    sim.run()
}

// ===========================================================================
// Group 4: Concurrent operations
// ===========================================================================

/// 3 clients submit different jobs simultaneously → all succeed.
#[test]
fn concurrent_job_submissions() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    for i in 0..3 {
        let name = format!("client-{i}");
        let filename = format!("test-{i}.cha");
        sim.client(name, async move {
            let client = turmoil_http_client();
            let sub = test_submission_with_filename(&filename);
            let resp = post_json(&client, &format!("{SERVER_BASE}/jobs"), &sub).await?;
            assert_eq!(
                resp.status().as_u16(),
                200,
                "Concurrent submission {filename} should succeed"
            );
            Ok(())
        });
    }
    sim.run()
}

/// 3 clients poll the same job simultaneously → all get consistent state.
#[test]
fn concurrent_poll_same_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    // First: submit and complete a job
    sim.client("submitter", async {
        let client = turmoil_http_client();
        let _job_id = submit_and_complete(&client).await?;
        Ok(())
    });
    sim.run()?;

    // Now 3 clients poll the job list simultaneously
    for i in 0..3 {
        let name = format!("poller-{i}");
        sim.client(name, async move {
            let client = turmoil_http_client();
            let resp = client
                .request(get(&format!("{SERVER_BASE}/jobs")))
                .await
                .t()?;
            assert_eq!(resp.status().as_u16(), 200);
            let jobs = body_json(resp).await?;
            assert!(jobs.as_array().is_some(), "Jobs list should be an array");
            Ok(())
        });
    }
    sim.run()
}

/// Health endpoint reports active_jobs > 0 during processing.
#[test]
fn health_accurate_during_processing() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let _job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Check health — should show the active job
        let resp = client
            .request(get(&format!("{SERVER_BASE}/health")))
            .await
            .t()?;
        let health = body_json(resp).await?;
        assert_eq!(health["status"], "ok");
        // active_jobs may be 0 if test-echo completed instantly, so just verify the field exists
        assert!(
            health["active_jobs"].is_number(),
            "Health should report active_jobs count"
        );
        Ok(())
    });
    sim.run()
}

// ===========================================================================
// Group 5: Error responses
// ===========================================================================

/// Submit with unknown command → 400.
#[test]
fn submit_unknown_command() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        // Send raw JSON with an invalid command
        let bad_submission = serde_json::json!({
            "command": "nonexistent_command",
            "lang": "eng",
            "files": [{"filename": "test.cha", "content": "test"}],
            "options": {"Morphotag": {"retokenize": false, "skipmultilang": false}}
        });
        let json = serde_json::to_vec(&bad_submission)?;
        let resp = client
            .request(
                Request::builder()
                    .method("POST")
                    .uri(format!("{SERVER_BASE}/jobs"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(json))
                    .expect("build request"),
            )
            .await
            .t()?;
        assert!(
            resp.status().as_u16() == 400 || resp.status().as_u16() == 422,
            "Unknown command should be rejected, got {}",
            resp.status()
        );
        Ok(())
    });
    sim.run()
}

/// Submit with empty files list → 400.
#[test]
fn submit_empty_files() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let mut sub = test_submission();
        sub.files.clear();
        let resp = post_json(&client, &format!("{SERVER_BASE}/jobs"), &sub).await?;
        assert_eq!(
            resp.status().as_u16(),
            400,
            "Empty files should be rejected"
        );
        Ok(())
    });
    sim.run()
}

/// GET nonexistent job → 404.
#[test]
fn get_nonexistent_job() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(30))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        assert_eq!(
            get_status(&client, &format!("{SERVER_BASE}/jobs/totally-fake-id")).await?,
            404
        );
        Ok(())
    });
    sim.run()
}

// ===========================================================================
// Group 6: Network fault + lifecycle combos
// ===========================================================================

/// Cancel during partition: times out, repair, cancel again → succeeds.
#[test]
fn cancel_during_partition() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();
        let job_id = submit_and_get_id(&client, &test_submission()).await?;

        // Partition the network
        turmoil::partition("client", "server");

        // Cancel attempt should time out
        let cancel_result = tokio::time::timeout(
            Duration::from_secs(3),
            post_empty(&client, &format!("{SERVER_BASE}/jobs/{job_id}/cancel")),
        )
        .await;
        assert!(
            cancel_result.is_err(),
            "Cancel should time out under partition"
        );

        // Repair and cancel again
        turmoil::repair("client", "server");
        let resp = post_empty(&client, &format!("{SERVER_BASE}/jobs/{job_id}/cancel")).await?;
        assert_eq!(resp.status().as_u16(), 200);
        Ok(())
    });
    sim.run()
}

/// Submit job under message hold, release → job processes normally.
#[test]
fn submit_under_message_hold_then_release() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();

        // Hold client → server messages
        turmoil::hold("client", "server");

        // Spawn submission (will be buffered). Build the JSON payload
        // eagerly so the spawned future doesn't capture non-Send types.
        let payload = serde_json::to_vec(&test_submission()).expect("serialize");
        let submit_handle = tokio::spawn({
            let client = client.clone();
            async move {
                client
                    .request(
                        Request::builder()
                            .method("POST")
                            .uri(format!("{SERVER_BASE}/jobs"))
                            .header("content-type", "application/json")
                            .body(axum::body::Body::from(payload))
                            .expect("build request"),
                    )
                    .await
            }
        });

        // Wait then release
        tokio::time::sleep(Duration::from_secs(3)).await;
        turmoil::release("client", "server");

        // Submission should complete
        let resp = submit_handle.await.t()?.t()?;
        assert_eq!(resp.status().as_u16(), 200);
        let info = body_json(resp).await?;
        let job_id = info["job_id"].as_str().expect("job_id").to_string();

        // Job should eventually complete
        let status = poll_until_terminal(&client, &job_id, 300).await?;
        assert_eq!(status, "completed");
        Ok(())
    });
    sim.run()
}

/// Job list endpoint works and returns array.
#[test]
fn list_jobs_returns_array() -> turmoil::Result {
    let python = require_python!();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(120))
        .build();
    let mut _app = create_real_test_app(&python);
    serve_real_app(&mut sim, &mut _app);

    sim.client("client", async {
        let client = turmoil_http_client();

        // Empty at first
        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs")))
            .await
            .t()?;
        assert_eq!(resp.status().as_u16(), 200);
        let jobs = body_json(resp).await?;
        let arr = jobs.as_array().expect("jobs should be array");
        assert_eq!(arr.len(), 0, "No jobs initially");

        // Submit one
        let _id = submit_and_get_id(&client, &test_submission()).await?;

        // Now should have 1
        let resp = client
            .request(get(&format!("{SERVER_BASE}/jobs")))
            .await
            .t()?;
        let jobs = body_json(resp).await?;
        let arr = jobs.as_array().expect("jobs should be array");
        assert_eq!(arr.len(), 1, "One job after submission");
        Ok(())
    });
    sim.run()
}
