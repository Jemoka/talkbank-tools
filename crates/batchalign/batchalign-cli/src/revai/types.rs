//! Rev.AI API request and response types.
//!
//! These records mirror the subset of the Rev.AI Speech-to-Text v1 API used by
//! batchalign3. They intentionally model the wire format closely so that the
//! shared client can be reused by both the server control plane and the PyO3
//! bridge without a second translation layer.

use serde::{Deserialize, Serialize};

/// Status of a Rev.AI transcription job.
///
/// Rev.AI jobs move in one direction:
/// `InProgress` -> `Transcribed` | `Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// The upload was accepted and Rev.AI is still processing the audio.
    InProgress,
    /// The transcript is ready to download.
    Transcribed,
    /// The job failed permanently and must be resubmitted from scratch.
    Failed,
}

/// A Rev.AI job returned by submission or polling endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct Job {
    /// Rev.AI-assigned job identifier.
    pub id: String,
    /// Current lifecycle state for the job.
    pub status: JobStatus,
    /// Human-readable failure detail for terminal failures.
    #[serde(default)]
    pub failure_detail: Option<String>,
    /// Language detected by Rev.AI when `language: "auto"` was used.
    ///
    /// Rev.AI returns this field as an ISO 639-1 code (e.g. `"es"`, `"en"`)
    /// once a job completes with auto-detection enabled.
    #[serde(default)]
    pub language: Option<String>,
}

/// A single word or punctuation element inside one Rev.AI monologue.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Element {
    /// Rev.AI element type, typically `"text"` or `"punct"`.
    #[serde(rename = "type")]
    pub element_type: String,
    /// Element text as returned by Rev.AI.
    pub value: String,
    /// Start time in seconds for timed tokens.
    #[serde(default)]
    pub ts: Option<f64>,
    /// End time in seconds for timed tokens.
    #[serde(default)]
    pub end_ts: Option<f64>,
    /// Optional confidence score emitted for text elements.
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// A contiguous speaker turn in a Rev.AI transcript.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Monologue {
    /// Zero-based speaker index assigned by Rev.AI.
    pub speaker: i32,
    /// Sequence of timed words and punctuation for the turn.
    pub elements: Vec<Element>,
}

/// Full Rev.AI transcript payload returned by the transcript endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Transcript {
    /// Ordered monologues produced by Rev.AI.
    pub monologues: Vec<Monologue>,
}

/// Submission options for `POST /speechtotext/v1/jobs`.
#[derive(Debug, Clone, Serialize)]
pub struct SubmitOptions {
    /// ISO 639-1 language code expected by Rev.AI.
    pub language: String,
    /// Optional speaker-count hint for languages where Rev.AI supports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speakers_count: Option<u32>,
    /// Optional toggle for Rev.AI post-processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_postprocessing: Option<bool>,
    /// Optional metadata string attached to the submitted job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

// ---------------------------------------------------------------------------
// Language Identification API types
// ---------------------------------------------------------------------------

/// Status of a Rev.AI language identification job.
///
/// Language ID jobs share the same lifecycle as transcription jobs:
/// `InProgress` -> `Completed` | `Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LangIdJobStatus {
    /// The job is still being processed.
    InProgress,
    /// The result is ready to download.
    Completed,
    /// The job failed permanently.
    Failed,
}

/// A Rev.AI language identification job returned by submission or polling.
#[derive(Debug, Clone, Deserialize)]
pub struct LangIdJob {
    /// Rev.AI-assigned job identifier.
    pub id: String,
    /// Current lifecycle state.
    pub status: LangIdJobStatus,
    /// Human-readable failure detail for terminal failures.
    #[serde(default)]
    pub failure_detail: Option<String>,
}

/// One language with its confidence score from the Language ID result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanguageConfidence {
    /// ISO 639-1 language code (e.g., `"en"`, `"es"`).
    pub language: String,
    /// Confidence score (0.0 – 1.0).
    pub confidence: f64,
}

/// Result from `GET /languageid/v1/jobs/{id}/result`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LangIdResult {
    /// The most probable language (ISO 639-1 code, e.g., `"es"`).
    pub top_language: String,
    /// All detected languages ranked by confidence.
    pub language_confidences: Vec<LanguageConfidence>,
}

// ---------------------------------------------------------------------------
// Speech-to-Text supplementary types
// ---------------------------------------------------------------------------

/// Simplified timed-word projection used by the UTR path.
#[derive(Debug, Clone, Serialize)]
pub struct TimedWord {
    /// Trimmed token text. Empty/whitespace tokens are filtered out upstream.
    pub word: String,
    /// Absolute start time in milliseconds.
    pub start_ms: u64,
    /// Absolute end time in milliseconds.
    pub end_ms: u64,
}
