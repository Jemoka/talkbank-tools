//! Worker IPC types — FROZEN legacy V1 protocol.
//!
//! ⚠️ **This module is frozen and no longer the active protocol.**
//!
//! - **Status:** Legacy compatibility surface (JSON-lines over stdio).
//! - **Active alternative:** [`crate::worker_v2`] (typed MessagePack envelopes).
//! - **Deprecation:** V1 support will be removed after all production tasks are migrated
//!   to [`crate::worker_v2`]. Currently used only by `dispatch_batch_infer()` for
//!   backward-compatibility paths that do not route through the task orchestration layer.
//!
//! ## What Changed in V2
//!
//! The original worker protocol in this module defined the request/response payloads
//! exchanged for `infer`, `batch-infer`, `health`, and `capabilities` operations over
//! JSON-lines.
//!
//! V2 replaces this with:
//! - **Typed envelopes** (`ExecuteRequestV2`, `ExecuteResponseV2`, `ProgressEventV2`).
//! - **Explicit artifact references** (no large binary payloads embedded in JSON).
//! - **Task-discriminated union** (task kind is encoded in the envelope, enabling
//!   type-safe deserialization on both sides).
//! - **Backward-incompatible wire format** (different serialization, no V1→V2 bridge).
//!
//! ## Frozen Contract
//!
//! To preserve backward compatibility with any remaining direct consumers:
//! - Do NOT add new fields to existing structs in this module.
//! - Do NOT change the order of existing fields.
//! - Do NOT rename types or enum variants.
//! - If a new feature is needed, define it in [`crate::worker_v2`] instead.
//!
//! Any changes that would break the JSON serialization format are banned for this module.
//!
//! ## Migration Path
//!
//! 1. Identify remaining callers of `dispatch_batch_infer()`.
//! 2. Route them through `dispatch_execute_v2()` with V2 request types.
//! 3. Remove this module and all V1 paths once migration is complete.
//!
//! See also: [`crate::worker_v2`] for the source-of-truth protocol definition.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::{DurationSeconds, LanguageCode3, WorkerLanguage};

// ---------------------------------------------------------------------------
// Domain newtypes (worker-specific)
// ---------------------------------------------------------------------------

numeric_id!(
    /// OS process ID of a Python worker.
    pub WorkerPid(u32) [Eq]
);

/// Worker health status returned by the Python process health check.
///
/// The Python side sends `"ok"` as a string over the IPC channel.
/// Any unrecognized value deserializes to `Unknown` and triggers a
/// crash-restart cycle in the pool health loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerHealthStatus {
    /// Worker is responsive and its loaded pipeline is functioning.
    Ok,
    /// Unrecognized status value — treat as unhealthy.
    #[serde(other)]
    Unknown,
}

impl WorkerHealthStatus {
    /// Return `true` when the worker reported healthy status.
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

impl std::fmt::Display for WorkerHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Response from worker health operation.
///
/// Returned when the server sends `{"op":"health"}` over the worker's
/// stdio channel.  Used by the pool's health loop to detect stuck or
/// crashed workers before they affect job dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerHealthResponse {
    /// Worker health status — `Ok` when responsive, `Unknown` otherwise.
    pub status: WorkerHealthStatus,
    /// The logical bootstrap target this worker was spawned for (for example
    /// `"infer:morphosyntax"`). Workers are specialized at spawn time and cannot
    /// change target. This is a worker-internal label, not a released command.
    pub command: String,
    /// Worker-runtime language string this worker was spawned for.
    ///
    /// This is a routing/bootstrap value rather than a true domain language,
    /// so it may be `"auto"` or an empty string for runtime-only worker keys.
    pub lang: WorkerLanguage,
    /// OS process ID of the Python worker.  Used for crash diagnostics
    /// and force-kill during shutdown.
    pub pid: WorkerPid,
    /// Seconds since the worker process started (wall clock).  Useful for
    /// monitoring idle workers and debugging memory leaks over time.
    pub uptime_s: DurationSeconds,
}

/// Response from worker capabilities operation.
///
/// Returned when the server sends `{"op":"capabilities"}` during startup
/// probing.  Determines which commands the server advertises in its health
/// endpoint, which warmup workers to spawn, and whether to use thread-based
/// or process-based concurrency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerCapabilities {
    /// Compatibility-only command list reported by the worker.
    ///
    /// Rust no longer trusts this field for released command availability and
    /// instead derives the command surface from `infer_tasks +
    /// engine_versions`. Test-echo still uses it for direct worker CLI
    /// coverage.
    pub commands: Vec<String>,
    /// Whether the worker is running on free-threaded Python (3.14t+).
    /// When `true`, the server uses thread workers with shared models
    /// instead of process workers with model copies, dramatically reducing
    /// memory usage for CPU-bound commands.
    pub free_threaded: bool,
    /// Tasks supported by the `infer` op.
    pub infer_tasks: Vec<InferTask>,
    /// Engine version strings by task (e.g. `{"morphosyntax": "stanza-1.9.2"}`).
    /// Used by the server to match cache entries to the correct engine version.
    pub engine_versions: BTreeMap<String, String>,
    /// Per-language Stanza processor availability.
    ///
    /// Key is ISO-639-3 code (e.g. "eng", "nld"); value lists available
    /// processor names. Built from Stanza's `resources.json` at worker
    /// startup. Empty when the worker does not load Stanza (e.g. IO
    /// profile) or when the worker is an older version that does not
    /// report this field.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stanza_capabilities: BTreeMap<String, StanzaLanguageProcessors>,
}

/// Processor availability for one language in Stanza.
///
/// Reported by the Python worker from `resources.json`. Used by the
/// Rust server for submission validation and dispatch routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StanzaLanguageProcessors {
    /// Stanza alpha-2 code this ISO-639-3 code maps to.
    pub alpha2: String,
    /// Available processor names (e.g. `["tokenize", "pos", "lemma", "depparse", "mwt"]`).
    pub processors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Pure inference protocol (CHAT-divorced)
// ---------------------------------------------------------------------------

/// Supported inference tasks for the CHAT-divorced worker protocol.
///
/// This enum is serialized as snake_case strings on the wire.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InferTask {
    /// Stanza morphosyntax tagging (`morphotag` command path).
    Morphosyntax,
    /// Utterance segmentation.
    Utseg,
    /// Machine translation.
    Translate,
    /// Coreference annotation.
    Coref,
    /// Forced alignment.
    Fa,
    /// Automatic speech recognition.
    Asr,
    /// OpenSMILE feature extraction.
    Opensmile,
    /// AVQI (Acoustic Voice Quality Index).
    Avqi,
    /// Speaker diarization.
    Speaker,
}

/// Request for a single inference operation.
///
/// The server owns all CHAT operations (parse, cache, inject, validate,
/// serialize). Workers are stateless inference endpoints that receive
/// structured payloads and return results. CHAT text never crosses the
/// IPC boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferRequest {
    /// Inference task identifier.
    pub task: InferTask,
    /// 3-letter ISO language code.
    pub lang: LanguageCode3,
    /// Task-specific payload (structure depends on `task`).
    pub payload: serde_json::Value,
}

/// Response from a single inference operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferResponse {
    /// Inference result (structure depends on the task).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message if inference failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Processing time in seconds.
    #[serde(default)]
    pub elapsed_s: DurationSeconds,
}

/// Request for batched inference (multiple items, one model call).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchInferRequest {
    /// Inference task identifier.
    pub task: InferTask,
    /// 3-letter ISO language code.
    pub lang: LanguageCode3,
    /// Batch of payloads to process together.
    pub items: Vec<serde_json::Value>,
    /// Multi-word token lexicon: surface form → expansion tokens.
    /// Only used by the `Morphosyntax` task. Empty when no custom
    /// lexicon is supplied. Backward-compatible: absent in JSON when empty,
    /// defaults to empty on deserialization.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mwt: BTreeMap<String, Vec<String>>,
    /// Operator opt-in to the legacy Stanza constituency-parser
    /// fallback for utseg when no language-specific TalkBank BERT
    /// model is configured. Set by the `--utseg-fallback-stanza` CLI
    /// flag. Only consulted by the `Utseg` task; ignored by other
    /// inference tasks. Default `false` mirrors the
    /// `WhisperHubModelNotFoundError` "refuse silent substitution"
    /// pattern.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_stanza_fallback: bool,
}

/// Response from batched inference — one `InferResponse` per item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchInferResponse {
    /// Results in the same order as the request's `items` vec.
    pub results: Vec<InferResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_health_roundtrip() {
        let health = WorkerHealthResponse {
            status: WorkerHealthStatus::Ok,
            command: "infer:morphosyntax".into(),
            lang: WorkerLanguage::from(LanguageCode3::eng()),
            pid: WorkerPid(12345),
            uptime_s: DurationSeconds(120.5),
        };
        let json = serde_json::to_string(&health).unwrap();
        let back: WorkerHealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(health, back);
    }

    #[test]
    fn worker_capabilities_roundtrip() {
        let caps = WorkerCapabilities {
            commands: vec!["morphotag".into(), "align".into(), "opensmile".into()],
            free_threaded: true,
            infer_tasks: vec![],
            engine_versions: BTreeMap::new(),
            stanza_capabilities: BTreeMap::new(),
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: WorkerCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, back);
    }

    #[test]
    fn worker_capabilities_with_infer_fields() {
        let caps = WorkerCapabilities {
            commands: vec!["morphotag".into()],
            free_threaded: false,
            infer_tasks: vec![InferTask::Morphosyntax, InferTask::Utseg],
            engine_versions: BTreeMap::from([
                ("morphosyntax".into(), "stanza-1.9.2".into()),
                ("utseg".into(), "stanza-1.9.2".into()),
            ]),
            stanza_capabilities: BTreeMap::new(),
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: WorkerCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, back);
    }

    #[test]
    fn worker_capabilities_missing_infer_fields_is_rejected() {
        let json = r#"{"commands":["morphotag"],"free_threaded":false}"#;
        let err = serde_json::from_str::<WorkerCapabilities>(json).unwrap_err();
        assert!(err.to_string().contains("infer_tasks"));
    }

    #[test]
    fn infer_request_roundtrip() {
        let req = InferRequest {
            task: InferTask::Morphosyntax,
            lang: LanguageCode3::eng(),
            payload: serde_json::json!({
                "words": ["the", "dog", "runs"],
                "terminator": ".",
                "special_forms": []
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: InferRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn infer_response_success() {
        let resp = InferResponse {
            result: Some(
                serde_json::json!({"mor": "det|the n|dog v|run-3S", "gra": "1|2|DET 2|3|SUBJ 3|0|ROOT"}),
            ),
            error: None,
            elapsed_s: DurationSeconds(0.042),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("error")); // None fields skipped
        let back: InferResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn infer_response_error() {
        let resp = InferResponse {
            result: None,
            error: Some("model not loaded".into()),
            elapsed_s: DurationSeconds(0.001),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: InferResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn batch_infer_request_roundtrip() {
        let req = BatchInferRequest {
            task: InferTask::Morphosyntax,
            lang: LanguageCode3::eng(),
            items: vec![
                serde_json::json!({"words": ["hello"]}),
                serde_json::json!({"words": ["goodbye"]}),
            ],
            mwt: BTreeMap::new(),
            allow_stanza_fallback: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        // Empty mwt should be omitted from JSON
        assert!(!json.contains("\"mwt\""));
        let back: BatchInferRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn batch_infer_request_with_mwt_roundtrip() {
        let req = BatchInferRequest {
            task: InferTask::Morphosyntax,
            lang: LanguageCode3::eng(),
            items: vec![serde_json::json!({"words": ["gonna"]})],
            mwt: BTreeMap::from([("gonna".into(), vec!["going".into(), "to".into()])]),
            allow_stanza_fallback: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"mwt\""));
        let back: BatchInferRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn batch_infer_request_backward_compat_no_mwt_field() {
        // Old workers/servers that don't send "mwt" should still deserialize
        let json = r#"{"task":"morphosyntax","lang":"eng","items":[]}"#;
        let req: BatchInferRequest = serde_json::from_str(json).unwrap();
        assert!(req.mwt.is_empty());
    }

    #[test]
    fn infer_task_wire_format_is_snake_case_string() {
        let req = BatchInferRequest {
            task: InferTask::Translate,
            lang: LanguageCode3::eng(),
            items: vec![],
            mwt: BTreeMap::new(),
            allow_stanza_fallback: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"task\":\"translate\""));
    }

    #[test]
    fn batch_infer_response_roundtrip() {
        let resp = BatchInferResponse {
            results: vec![
                InferResponse {
                    result: Some(serde_json::json!({"mor": "co|hello"})),
                    error: None,
                    elapsed_s: DurationSeconds(0.01),
                },
                InferResponse {
                    result: None,
                    error: Some("empty input".into()),
                    elapsed_s: DurationSeconds(0.0),
                },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: BatchInferResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }
}
