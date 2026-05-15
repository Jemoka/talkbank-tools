//! HTTP client for communicating with batchalign servers.
//!
//! Wraps `reqwest::Client` with retry logic matching the Python implementation.

use std::time::Duration;

use crate::api::{
    CancellationRecord, CancellationRequest, DisplayPath, FileResult, HealthResponse, JobId,
    JobInfo, JobListItem, JobResultResponse, JobSubmission,
};
use reqwest::Client;
use tracing::debug;

use crate::cli::error::CliError;

// ---------------------------------------------------------------------------
// Constants (matching Python `dispatch_server.py`)
// ---------------------------------------------------------------------------

/// Minimum poll interval (seconds).
pub const POLL_MIN: f64 = 0.5;
/// Maximum poll interval (seconds).
pub const POLL_MAX: f64 = 5.0;
/// Poll interval step increase per idle poll.
pub const POLL_STEP: f64 = 0.5;
/// Maximum retry attempts for transient errors.
pub const RETRY_ATTEMPTS: u32 = 3;
/// Initial retry backoff (seconds), doubles each attempt.
pub const RETRY_BACKOFF: f64 = 2.0;
/// Consecutive poll failures before giving up.
pub const MAX_POLL_FAILURES: u32 = 10;

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// HTTP client for a batchalign server.
#[derive(Clone)]
pub struct BatchalignClient {
    http: Client,
}

impl BatchalignClient {
    /// Create a new client with default timeout settings.
    pub fn new() -> Result<Self, CliError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self { http })
    }

    /// `GET /health` — check server health and capabilities.
    pub async fn health_check(&self, url: &str) -> Result<HealthResponse, CliError> {
        let resp = self
            .request_with_retry(
                reqwest::Method::GET,
                &format!("{url}/health"),
                None::<&()>,
                Duration::from_secs(30),
            )
            .await?;
        let health: HealthResponse = resp.json().await?;
        Ok(health)
    }

    /// `POST /jobs` — submit a new job.
    ///
    /// Retries transient connect/timeout errors via `request_with_retry`.
    /// Does not retry 4xx/5xx HTTP rejections: the server's refusal is
    /// deterministic and re-sending the same payload will not fix it.
    pub async fn submit_job(&self, url: &str, sub: &JobSubmission) -> Result<JobInfo, CliError> {
        // 120s per attempt preserves the original single-attempt budget; the
        // retry wrapper multiplies total wallclock by up to RETRY_ATTEMPTS.
        let resp = self
            .request_with_retry(
                reqwest::Method::POST,
                &format!("{url}/jobs"),
                Some(sub),
                Duration::from_secs(120),
            )
            .await?;
        let info: JobInfo = resp.json().await?;
        Ok(info)
    }

    /// `GET /jobs/{id}` — get job status.
    pub async fn get_job(&self, url: &str, job_id: &JobId) -> Result<JobInfo, CliError> {
        let resp = self
            .http
            .get(format!("{url}/jobs/{job_id}"))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(CliError::JobLost {
                job_id: job_id.clone(),
            });
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let detail = read_http_error_detail(resp).await;
            return Err(CliError::ServerHttp { status, detail });
        }

        let info: JobInfo = resp.json().await?;
        Ok(info)
    }

    /// `GET /jobs/{id}/results/{*filename}` — fetch a single file result.
    ///
    /// The filename is embedded directly in the URL path (slashes included).
    /// The server route uses axum's `{*filename}` wildcard to capture the full
    /// remaining path, so slashes in `DisplayPath` values (e.g.
    /// `resources/corpus/subdir/file.cha`) are safe.
    pub async fn get_file_result(
        &self,
        url: &str,
        job_id: &JobId,
        filename: &DisplayPath,
    ) -> Result<FileResult, CliError> {
        let resp = self
            .request_with_retry(
                reqwest::Method::GET,
                &format!("{url}/jobs/{job_id}/results/{filename}"),
                None::<&()>,
                Duration::from_secs(30),
            )
            .await?;
        let result: FileResult = resp.json().await?;
        Ok(result)
    }

    /// `GET /jobs/{id}/results` — fetch all results for a job.
    pub async fn get_all_results(
        &self,
        url: &str,
        job_id: &JobId,
    ) -> Result<JobResultResponse, CliError> {
        let resp = self
            .request_with_retry(
                reqwest::Method::GET,
                &format!("{url}/jobs/{job_id}/results"),
                None::<&()>,
                Duration::from_secs(30),
            )
            .await?;
        let results: JobResultResponse = resp.json().await?;
        Ok(results)
    }

    /// `GET /jobs` — list all jobs.
    pub async fn list_jobs(&self, url: &str) -> Result<Vec<JobListItem>, CliError> {
        let resp = self
            .http
            .get(format!("{url}/jobs"))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let detail = read_http_error_detail(resp).await;
            return Err(CliError::ServerHttp { status, detail });
        }
        let jobs: Vec<JobListItem> = resp.json().await?;
        Ok(jobs)
    }

    /// `POST /jobs/{id}/cancel` — request cancellation of a running or
    /// queued job, recording caller provenance in the server's audit table.
    ///
    /// Bug history: an earlier revision of this method called
    /// `DELETE /jobs/{id}` (the *delete* endpoint, which returns 409 for
    /// running jobs) — TUI cancels appeared to silently do nothing because
    /// the error was swallowed at the call site. Fixed 2026-04-26 alongside
    /// the cancel-provenance work; see the cancellation-hygiene plan in
    /// the workspace for the full incident.
    /// `GET /jobs/{id}/cancellations` — fetch the audit history of
    /// every cancel attempt against a job. Used by the
    /// `batchalign3 cancellations` subcommand and by tests that
    /// assert provenance was recorded.
    pub async fn list_cancellations(
        &self,
        url: &str,
        job_id: &JobId,
    ) -> Result<Vec<CancellationRecord>, CliError> {
        let resp = self
            .request_with_retry(
                reqwest::Method::GET,
                &format!("{url}/jobs/{job_id}/cancellations"),
                None::<&()>,
                Duration::from_secs(10),
            )
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let detail = read_http_error_detail(resp).await;
            return Err(CliError::ServerHttp { status, detail });
        }
        let records: Vec<CancellationRecord> = resp.json().await?;
        Ok(records)
    }

    /// `POST /jobs/{id}/cancel` with provenance body. See the
    /// docstring above on `list_cancellations` for the audit-trail
    /// shape; this method is the writer side.
    pub async fn cancel_job(
        &self,
        url: &str,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), CliError> {
        // Route through `request_with_retry` so transient network errors
        // don't silently drop the cancel — the 2026-04-25 incident pattern
        // (user re-presses cancel because nothing happened) is exactly
        // what the retry wrapper exists to prevent.
        let resp = self
            .request_with_retry(
                reqwest::Method::POST,
                &format!("{url}/jobs/{job_id}/cancel"),
                Some(&provenance),
                Duration::from_secs(10),
            )
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let detail = read_http_error_detail(resp).await;
            return Err(CliError::ServerHttp { status, detail });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Retry logic
    // -----------------------------------------------------------------------

    /// HTTP request with exponential-backoff retry for transient errors.
    ///
    /// Retries on `ConnectionError`/`Timeout`, NOT on 4xx/5xx HTTP errors.
    /// Per-attempt timeout is configurable so callers with large request
    /// bodies (e.g. `POST /jobs`) can allot more than the default 30s.
    async fn request_with_retry<B: serde::Serialize>(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&B>,
        per_attempt_timeout: Duration,
    ) -> Result<reqwest::Response, CliError> {
        let mut delay = RETRY_BACKOFF;
        let mut last_err: Option<reqwest::Error> = None;

        for attempt in 0..RETRY_ATTEMPTS {
            let mut builder = self.http.request(method.clone(), url);
            if let Some(b) = body {
                builder = builder.json(b);
            }
            builder = builder.timeout(per_attempt_timeout);

            match builder.send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status().as_u16();
                        let detail = read_http_error_detail(resp).await;
                        return Err(CliError::ServerHttp { status, detail });
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    let is_transient = e.is_connect() || e.is_timeout();
                    if is_transient && attempt < RETRY_ATTEMPTS - 1 {
                        debug!(
                            attempt = attempt + 1,
                            max = RETRY_ATTEMPTS,
                            url,
                            error = %e,
                            "Retrying transient error"
                        );
                        let jitter = 0.5 + rand::random::<f64>() * 0.5;
                        tokio::time::sleep(Duration::from_secs_f64(delay * jitter)).await;
                        delay *= 2.0;
                        last_err = Some(e);
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }

        // SAFETY: the retry loop always sets last_err before `continue`.
        #[allow(clippy::expect_used)]
        Err(last_err
            .expect("retry loop exhausted without setting last_err")
            .into())
    }
}

async fn read_http_error_detail(resp: reqwest::Response) -> String {
    let status = resp.status();
    let status_text = status.canonical_reason().unwrap_or("").to_string();
    match resp.text().await {
        Ok(body) => summarize_http_error_body(&status_text, &body),
        Err(error) if status_text.is_empty() => format!("failed to read error body: {error}"),
        Err(error) => format!("{status_text} (failed to read error body: {error})"),
    }
}

fn summarize_http_error_body(status_text: &str, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return status_text.to_string();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(detail) = value.get("detail").and_then(|d| d.as_str())
        && !detail.trim().is_empty()
    {
        return detail.to_string();
    }

    trimmed.to_string()
}

/// Extract a short hostname label from a server URL (e.g. "myhost").
pub fn server_label(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme
        .split(':')
        .next()
        .unwrap_or(without_scheme)
        .split('/')
        .next()
        .unwrap_or(without_scheme);
    host.split('.').next().unwrap_or(host).to_string()
}

/// Parse comma-separated server URLs, strip whitespace and trailing slashes.
pub fn parse_servers(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_servers_basic() {
        let urls = parse_servers("http://a:8000, http://b:8000/ ,");
        assert_eq!(urls, vec!["http://a:8000", "http://b:8000"]);
    }

    #[test]
    fn server_label_extracts_hostname() {
        assert_eq!(server_label("http://server.local:8000"), "server");
        assert_eq!(server_label("https://192.168.1.1:8000/path"), "192");
        assert_eq!(server_label("http://myhost:9000"), "myhost");
    }

    #[test]
    fn job_status_is_terminal() {
        use crate::api::JobStatus;
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(JobStatus::Interrupted.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(!JobStatus::Queued.is_terminal());
    }

    #[test]
    fn parse_servers_empty() {
        let urls = parse_servers("");
        assert!(urls.is_empty());
    }

    #[test]
    fn parse_servers_single() {
        let urls = parse_servers("http://a:8000");
        assert_eq!(urls, vec!["http://a:8000"]);
    }

    #[test]
    fn parse_servers_trailing_slashes() {
        let urls = parse_servers("http://a:8000///");
        assert_eq!(urls, vec!["http://a:8000"]);
    }

    #[test]
    fn summarize_http_error_body_prefers_json_detail() {
        let detail = summarize_http_error_body("Bad Request", r#"{"detail":"bad config"}"#);
        assert_eq!(detail, "bad config");
    }

    #[test]
    fn summarize_http_error_body_falls_back_to_raw_body() {
        let detail = summarize_http_error_body("Bad Request", "plain server failure");
        assert_eq!(detail, "plain server failure");
    }

    #[test]
    fn summarize_http_error_body_falls_back_to_status_text_when_empty() {
        let detail = summarize_http_error_body("Bad Request", "   ");
        assert_eq!(detail, "Bad Request");
    }

    #[test]
    fn server_label_no_scheme() {
        assert_eq!(server_label("myhost:9000"), "myhost");
        assert_eq!(server_label("bare-hostname"), "bare-hostname");
    }

    // -----------------------------------------------------------------------
    // Regression tests for silent submission-failure bug.
    //
    // During a large corpus run, `submit_job` bypassed the existing
    // `request_with_retry` helper. A transient connection-refused from the
    // daemon (e.g. while it finalized a previous job) surfaced to the caller
    // as an immediate error and the caller's script silently skipped the
    // chunk, losing work.  The tests below pin the invariant that
    // `submit_job` retries transient connect errors and does NOT retry 4xx
    // HTTP rejections.
    // -----------------------------------------------------------------------

    /// Minimal valid `JobSubmission` for HTTP-level tests. The server-side
    /// validation is bypassed here — we only care that the request
    /// actually reaches whatever endpoint the test is pointing at.
    fn minimal_submission() -> crate::api::JobSubmission {
        use crate::api::{
            JobSubmission, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
        };
        use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};
        JobSubmission {
            command: ReleasedCommand::Morphotag,
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            files: Vec::new(),
            media_files: Vec::new(),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            paths_mode: true,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
            display_names: Vec::new(),
            debug_traces: false,
            before_paths: Vec::new(),
        }
    }

    /// `submit_job` must retry transient connect errors. Port 1 is reserved
    /// and the OS consistently rejects connects with ECONNREFUSED, which is
    /// the exact `reqwest::Error::is_connect()` class we want the retry
    /// wrapper to handle.
    ///
    /// Before the fix: `submit_job` never retries, returns an error in
    /// well under one second.
    ///
    /// After the fix: `submit_job` retries `RETRY_ATTEMPTS=3` times with
    /// exponential backoff starting at `RETRY_BACKOFF=2.0s`, so the total
    /// elapsed time must be at least one backoff interval's worth of
    /// sleep (subject to jitter `0.5-1.5x`). We assert a lower bound of
    /// `RETRY_BACKOFF * 0.5` seconds, which is the smallest delay jitter
    /// can produce after exactly one retry.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn submit_job_retries_transient_connect_errors() {
        let client = BatchalignClient::new().expect("HTTP client build should not fail in tests");
        let sub = minimal_submission();
        let start = std::time::Instant::now();
        let result = client.submit_job("http://127.0.0.1:1", &sub).await;
        let elapsed = start.elapsed();
        assert!(result.is_err(), "expected submission to port 1 to fail");
        let min_elapsed = Duration::from_secs_f64(RETRY_BACKOFF * 0.5);
        assert!(
            elapsed >= min_elapsed,
            "submit_job must retry on transient connect errors; elapsed {:?}, \
             expected at least {:?} (one retry × backoff × min-jitter)",
            elapsed,
            min_elapsed
        );
    }

    /// `submit_job` must NOT retry HTTP 4xx responses (e.g. 413 payload
    /// too large). Those are rejections the server deliberately issued; a
    /// retry wastes work and cannot succeed. This test spins a minimal
    /// TCP mock that answers one 413 per connection and counts the total
    /// connections accepted.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn submit_job_does_not_retry_413_length_limit_exceeded() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let count_clone = connection_count.clone();
        tokio::spawn(async move {
            for _ in 0..5 {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                count_clone.fetch_add(1, Ordering::SeqCst);
                // Drain the incoming request bytes until we see the end of
                // the headers. hyper won't start parsing the response until
                // it's finished sending its request, so for a small payload
                // we can just read once and respond.
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let body = b"{\"detail\":\"length limit exceeded\"}";
                let response = format!(
                    "HTTP/1.1 413 Payload Too Large\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\
                     \r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.write_all(body).await;
                let _ = stream.flush().await;
                let _ = stream.shutdown().await;
            }
        });

        let client = BatchalignClient::new().expect("HTTP client build should not fail in tests");
        let sub = minimal_submission();
        let result = client
            .submit_job(&format!("http://127.0.0.1:{port}"), &sub)
            .await;
        assert!(result.is_err(), "expected 413 to surface as an error");
        match result.unwrap_err() {
            CliError::ServerHttp { status, .. } => assert_eq!(status, 413),
            other => panic!("expected ServerHttp(413), got {other:?}"),
        }
        // Give the listener a moment to record any trailing retry attempts.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let attempts = connection_count.load(Ordering::SeqCst);
        assert_eq!(
            attempts, 1,
            "submit_job must NOT retry 4xx HTTP rejections; got {attempts} attempts"
        );
    }
}
