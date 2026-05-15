//! Blocking Rev.AI HTTP client.
//!
//! The shared client stays blocking on purpose. The PyO3 binding can release
//! the Python GIL around an entire request, and the Rust server can move upload
//! work onto `spawn_blocking` threads. That keeps the client simple while still
//! fitting both host runtimes cleanly.

use std::path::Path;
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};

use super::types::{
    Job, JobStatus, LangIdJob, LangIdJobStatus, LangIdResult, SubmitOptions, TimedWord, Transcript,
};

/// Transcript plus optional detected language from Rev.AI auto-detection.
///
/// When `language: "auto"` is used in `SubmitOptions`, Rev.AI returns the
/// detected language as an ISO 639-1 code on the completed `Job`. This struct
/// bundles the transcript with that detection result so callers can propagate
/// the real language to downstream pipeline stages.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptResult {
    /// The full transcript payload.
    pub transcript: Transcript,
    /// ISO 639-1 language code detected by Rev.AI (e.g. `"es"`, `"en"`).
    /// `None` when a concrete language was specified (not auto-detected).
    pub detected_language: Option<String>,
}

const BASE_URL: &str = "https://api.rev.ai/speechtotext/v1";
const LANGID_BASE_URL: &str = "https://api.rev.ai/languageid/v1";
const TRANSCRIPT_ACCEPT: &str = "application/vnd.rev.transcript.v1.0+json";
const LANGID_ACCEPT: &str = "application/vnd.rev.languageid.v1.0+json";

/// Errors produced by Rev.AI client operations.
#[derive(Debug, thiserror::Error)]
pub enum RevAiError {
    /// The HTTP client failed before receiving a response.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Rev.AI returned a non-success HTTP status and response body.
    #[error("Rev.AI returned HTTP {status}: {body}")]
    ApiError {
        /// Numeric HTTP status code from Rev.AI.
        status: u16,
        /// Response body returned by Rev.AI.
        body: String,
    },

    /// A submitted job reached the failed terminal state.
    #[error("Rev.AI job failed: {0}")]
    JobFailed(String),

    /// All upload retry attempts were exhausted.
    #[error("Rev.AI upload retries exhausted: {0}")]
    Retry(String),

    /// JSON decoding failed for a Rev.AI response body.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Reading the local audio file failed before upload.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Standard result type for Rev.AI client operations.
pub type Result<T> = std::result::Result<T, RevAiError>;

/// Blocking Rev.AI HTTP client.
pub struct RevAiClient {
    api_key: String,
    client: Client,
}

impl RevAiClient {
    /// Create a new client bound to one API key.
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_owned(),
            client: Client::new(),
        }
    }

    /// Submit one local audio file for transcription.
    ///
    /// The client retries transient upload failures up to three times with
    /// exponential backoff before returning an error.
    pub fn submit_local_file(&self, path: &Path, opts: &SubmitOptions) -> Result<Job> {
        let file_bytes = std::fs::read(path)?;
        let file_name = upload_file_name(path);
        let options_json = serde_json::to_string(opts)?;

        let mut last_err: Option<RevAiError> = None;

        for attempt in 0..3u32 {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                eprintln!(
                    "talkbank-revai: retry {}/3 for upload of {} (waiting {}s)",
                    attempt + 1,
                    file_name,
                    delay.as_secs(),
                );
                thread::sleep(delay);
            }

            let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes.clone())
                .file_name(file_name.clone())
                .mime_str("audio/mpeg")?;
            let options_part = reqwest::blocking::multipart::Part::text(options_json.clone())
                .mime_str("application/json")?;
            let form = reqwest::blocking::multipart::Form::new()
                .part("media", file_part)
                .part("options", options_part);

            match self
                .client
                .post(format!("{BASE_URL}/jobs"))
                .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
                .multipart(form)
                .send()
            {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status().as_u16();
                        let body = read_error_body(resp);
                        if status >= 500 {
                            last_err = Some(RevAiError::ApiError { status, body });
                            continue;
                        }
                        return Err(RevAiError::ApiError { status, body });
                    }
                    return Ok(resp.json()?);
                }
                Err(err) => {
                    last_err = Some(RevAiError::Http(err));
                }
            }
        }

        Err(last_err.unwrap_or(RevAiError::Retry(
            "all upload retries exhausted without error".into(),
        )))
    }

    /// Fetch the current status for one previously submitted job.
    pub fn get_job_details(&self, job_id: &str) -> Result<Job> {
        let resp = self
            .client
            .get(format!("{BASE_URL}/jobs/{job_id}"))
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = read_error_body(resp);
            return Err(RevAiError::ApiError { status, body });
        }

        Ok(resp.json()?)
    }

    /// Download the transcript for a completed job.
    pub fn get_transcript(&self, job_id: &str) -> Result<Transcript> {
        let resp = self
            .client
            .get(format!("{BASE_URL}/jobs/{job_id}/transcript"))
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(ACCEPT, TRANSCRIPT_ACCEPT)
            .header(CONTENT_TYPE, "application/json")
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = read_error_body(resp);
            return Err(RevAiError::ApiError { status, body });
        }

        Ok(resp.json()?)
    }

    /// Submit audio, poll with exponential backoff, and download the transcript.
    pub fn transcribe_blocking(
        &self,
        path: &Path,
        opts: &SubmitOptions,
        max_poll_secs: u64,
    ) -> Result<TranscriptResult> {
        let job = self.submit_local_file(path, opts)?;
        self.poll_and_download(&job.id, 5, max_poll_secs)
    }

    /// Poll a previously submitted job until it completes, then download it.
    ///
    /// Returns a [`TranscriptResult`] that includes the detected language
    /// (when `language: "auto"` was used at submission time).
    pub fn poll_and_download(
        &self,
        job_id: &str,
        initial_interval_secs: u64,
        max_interval_secs: u64,
    ) -> Result<TranscriptResult> {
        let mut interval = initial_interval_secs;
        let mut attempts: u32 = 0;

        loop {
            let job = self.get_job_details(job_id)?;
            match job.status {
                JobStatus::InProgress => {
                    thread::sleep(Duration::from_secs(interval));
                    attempts += 1;
                    if attempts.is_multiple_of(3) {
                        interval = (interval * 2).min(max_interval_secs);
                    }
                }
                JobStatus::Transcribed => {
                    let transcript = self.get_transcript(job_id)?;
                    return Ok(TranscriptResult {
                        transcript,
                        detected_language: job.language,
                    });
                }
                JobStatus::Failed => {
                    let detail = job.failure_detail.unwrap_or_else(|| "unknown error".into());
                    return Err(RevAiError::JobFailed(detail));
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Language Identification API
    // -------------------------------------------------------------------

    /// Submit one local audio file for language identification.
    ///
    /// Returns the job ID for polling. The Language ID API is a separate
    /// Rev.AI endpoint from Speech-to-Text.
    pub fn submit_langid(&self, path: &Path) -> Result<LangIdJob> {
        let file_bytes = std::fs::read(path)?;
        let file_name = upload_file_name(path);

        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("audio/mpeg")?;
        let form = reqwest::blocking::multipart::Form::new().part("media", file_part);

        let resp = self
            .client
            .post(format!("{LANGID_BASE_URL}/jobs"))
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = read_error_body(resp);
            return Err(RevAiError::ApiError { status, body });
        }

        Ok(resp.json()?)
    }

    /// Poll a language identification job status.
    pub fn get_langid_job(&self, job_id: &str) -> Result<LangIdJob> {
        let resp = self
            .client
            .get(format!("{LANGID_BASE_URL}/jobs/{job_id}"))
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = read_error_body(resp);
            return Err(RevAiError::ApiError { status, body });
        }

        Ok(resp.json()?)
    }

    /// Download the language identification result for a completed job.
    pub fn get_langid_result(&self, job_id: &str) -> Result<LangIdResult> {
        let resp = self
            .client
            .get(format!("{LANGID_BASE_URL}/jobs/{job_id}/result"))
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(ACCEPT, LANGID_ACCEPT)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = read_error_body(resp);
            return Err(RevAiError::ApiError { status, body });
        }

        Ok(resp.json()?)
    }

    /// Submit audio for language identification, poll until complete, and
    /// return the result.
    ///
    /// Typically completes in 5–30 seconds. Uses exponential backoff polling.
    pub fn identify_language_blocking(
        &self,
        path: &Path,
        max_poll_secs: u64,
    ) -> Result<LangIdResult> {
        let job = self.submit_langid(path)?;
        let mut interval: u64 = 3;
        let mut attempts: u32 = 0;

        loop {
            let status = self.get_langid_job(&job.id)?;
            match status.status {
                LangIdJobStatus::InProgress => {
                    thread::sleep(Duration::from_secs(interval));
                    attempts += 1;
                    if attempts.is_multiple_of(3) {
                        interval = (interval * 2).min(max_poll_secs);
                    }
                }
                LangIdJobStatus::Completed => {
                    return self.get_langid_result(&job.id);
                }
                LangIdJobStatus::Failed => {
                    let detail = status
                        .failure_detail
                        .unwrap_or_else(|| "unknown error".into());
                    return Err(RevAiError::JobFailed(detail));
                }
            }
        }
    }
}

fn upload_file_name(path: &Path) -> String {
    path.file_name()
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn read_error_body(resp: reqwest::blocking::Response) -> String {
    match resp.text() {
        Ok(body) => body,
        Err(error) => format!("<failed to read response body: {error}>"),
    }
}

/// Project a full Rev.AI transcript into the simplified timed-word shape used
/// by the UTR path.
pub fn extract_timed_words(transcript: &Transcript) -> Vec<TimedWord> {
    struct TimedElement<'a> {
        value: &'a str,
        start_s: f64,
        end_s: f64,
    }

    let mut raw: Vec<TimedElement<'_>> = Vec::new();
    for monologue in &transcript.monologues {
        for elem in &monologue.elements {
            if let (Some(ts), Some(end_ts)) = (elem.ts, elem.end_ts) {
                raw.push(TimedElement {
                    value: &elem.value,
                    start_s: ts,
                    end_s: end_ts,
                });
            }
        }
    }

    raw.sort_by(|a, b| {
        a.start_s
            .partial_cmp(&b.start_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = Vec::with_capacity(raw.len());
    let mut prev_end_ms: f64 = 0.0;

    for elem in &raw {
        let cleaned = elem.value.trim();
        if cleaned.is_empty() {
            continue;
        }
        let start_ms = (elem.start_s * 1000.0).round() as u64;
        let end_ms = (elem.end_s * 1000.0).round() as u64;

        if (start_ms as f64) < prev_end_ms * 0.5 && prev_end_ms > 2000.0 {
            eprintln!(
                "talkbank-revai: timestamp regression at word {:?} (start={}ms after prev_end={}ms)",
                cleaned, start_ms, prev_end_ms as u64,
            );
        }

        result.push(TimedWord {
            word: cleaned.to_owned(),
            start_ms,
            end_ms,
        });
        prev_end_ms = end_ms as f64;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_job_in_progress() {
        let json = r#"{"id":"abc123","status":"in_progress"}"#;
        let job: Job = serde_json::from_str(json).unwrap();
        assert_eq!(job.id, "abc123");
        assert_eq!(job.status, JobStatus::InProgress);
        assert!(job.failure_detail.is_none());
    }

    #[test]
    fn parse_job_failed() {
        let json = r#"{"id":"ghi789","status":"failed","failure_detail":"Audio too short"}"#;
        let job: Job = serde_json::from_str(json).unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.failure_detail.as_deref(), Some("Audio too short"));
    }

    #[test]
    fn serialize_submit_options_full() {
        let opts = SubmitOptions {
            language: "en".into(),
            speakers_count: Some(2),
            skip_postprocessing: Some(true),
            metadata: Some("test_job".into()),
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains(r#""speakers_count":2"#));
        assert!(json.contains(r#""skip_postprocessing":true"#));
        assert!(json.contains(r#""metadata":"test_job""#));
    }

    #[test]
    fn extract_timed_words_basic() {
        let transcript: Transcript = serde_json::from_str(
            r#"{
            "monologues": [{
                "speaker": 0,
                "elements": [
                    {"type": "text", "value": "hello", "ts": 0.5, "end_ts": 0.9},
                    {"type": "text", "value": "world", "ts": 1.0, "end_ts": 1.5},
                    {"type": "punct", "value": "."}
                ]
            }]
        }"#,
        )
        .unwrap();

        let words = extract_timed_words(&transcript);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "hello");
        assert_eq!(words[0].start_ms, 500);
        assert_eq!(words[1].word, "world");
        assert_eq!(words[1].end_ms, 1500);
    }

    // -------------------------------------------------------------------
    // Language Identification type tests
    // -------------------------------------------------------------------

    #[test]
    fn parse_langid_job_in_progress() {
        let json = r#"{"id":"Umx5c6F7pH7r","status":"in_progress","type":"language_id","created_on":"2021-09-15T05:14:38.13"}"#;
        let job: crate::revai::types::LangIdJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.id, "Umx5c6F7pH7r");
        assert_eq!(job.status, crate::revai::types::LangIdJobStatus::InProgress);
    }

    #[test]
    fn parse_langid_job_completed() {
        let json = r#"{"id":"abc","status":"completed"}"#;
        let job: crate::revai::types::LangIdJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.status, crate::revai::types::LangIdJobStatus::Completed);
    }

    #[test]
    fn parse_langid_result() {
        let json = r#"{
            "top_language": "es",
            "language_confidences": [
                {"language": "es", "confidence": 0.907},
                {"language": "en", "confidence": 0.08},
                {"language": "nl", "confidence": 0.023}
            ]
        }"#;
        let result: crate::revai::types::LangIdResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.top_language, "es");
        assert_eq!(result.language_confidences.len(), 3);
        assert_eq!(result.language_confidences[0].language, "es");
        assert!(result.language_confidences[0].confidence > 0.9);
        assert_eq!(result.language_confidences[1].language, "en");
    }

    #[test]
    fn parse_langid_result_english_dominant() {
        let json = r#"{
            "top_language": "en",
            "language_confidences": [
                {"language": "en", "confidence": 0.95}
            ]
        }"#;
        let result: crate::revai::types::LangIdResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.top_language, "en");
        assert_eq!(result.language_confidences.len(), 1);
    }

    #[test]
    fn upload_file_name_prefers_basename() {
        assert_eq!(
            upload_file_name(Path::new("/tmp/example.wav")),
            "example.wav"
        );
    }

    #[test]
    fn upload_file_name_falls_back_to_full_path_when_needed() {
        assert_eq!(upload_file_name(Path::new("")), "");
        assert_eq!(upload_file_name(Path::new("/")), "/");
    }
}
