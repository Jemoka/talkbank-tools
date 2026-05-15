//! Worker protocol V2 response and result types.

use serde::{Deserialize, Serialize};

use crate::api::{DurationMs, DurationSeconds, LanguageCode3};

use super::requests::{ProtocolErrorCodeV2, WhisperChunkSpanV2, WorkerRequestIdV2};

/// One ASR result built from raw Whisper chunks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct WhisperChunkResultV2 {
    /// Transcript language.
    pub lang: LanguageCode3,
    /// Concatenated transcript text.
    pub text: String,
    /// Raw chunk spans.
    pub chunks: Vec<WhisperChunkSpanV2>,
}

/// Stable vocabulary for one monologue element returned by an ASR provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AsrElementKindV2 {
    /// Lexical content that should become transcript tokens.
    Text,
    /// Punctuation emitted by the provider.
    Punctuation,
}

/// One raw ASR element inside a speaker monologue.
///
/// Timing fields validated upstream by Python Pydantic models (see module docs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct AsrElementV2 {
    /// Surface token or punctuation value.
    pub value: String,
    /// Start timestamp in seconds when the provider exposes one.
    #[serde(default)]
    pub start_s: Option<DurationSeconds>,
    /// End timestamp in seconds when the provider exposes one.
    #[serde(default)]
    pub end_s: Option<DurationSeconds>,
    /// Stable element kind selected by the worker adapter.
    pub kind: AsrElementKindV2,
    /// Optional model/provider confidence score.
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// One speaker-attributed monologue returned by a provider ASR backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct AsrMonologueV2 {
    /// Stable speaker label chosen by the worker adapter.
    pub speaker: String,
    /// Ordered elements inside the monologue.
    pub elements: Vec<AsrElementV2>,
}

/// One ASR result built from provider-shaped speaker monologues.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct MonologueAsrResultV2 {
    /// Transcript language reported by the provider adapter.
    pub lang: LanguageCode3,
    /// Speaker-grouped ASR output.
    pub monologues: Vec<AsrMonologueV2>,
}

/// One raw Whisper forced-alignment token span returned by Python.
///
/// Timing fields validated upstream by Python Pydantic models (see module docs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct WhisperTokenTimingV2 {
    /// Surface token text returned by the FA runtime.
    pub text: String,
    /// Token onset timestamp in seconds.
    pub time_s: DurationSeconds,
}

/// Forced-alignment token response returned before Rust token-to-word
/// reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct WhisperTokenTimingResultV2 {
    /// Raw token timings in model order.
    pub tokens: Vec<WhisperTokenTimingV2>,
}

/// One word-level timing result.
///
/// Timing fields validated upstream by Python Pydantic models (see module docs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct IndexedWordTimingV2 {
    /// Start time in milliseconds.
    pub start_ms: DurationMs,
    /// End time in milliseconds.
    pub end_ms: DurationMs,
    /// Optional model confidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Forced-alignment indexed response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct IndexedWordTimingResultV2 {
    /// Indexed timing results aligned to the request words.
    pub indexed_timings: Vec<Option<IndexedWordTimingV2>>,
}

/// One morphosyntax item result returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct MorphosyntaxItemResultV2 {
    /// Raw Stanza `doc.to_dict()` sentence arrays when inference succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_sentences: Option<Vec<serde_json::Value>>,
    /// Optional per-item runtime error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batched morphosyntax response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct MorphosyntaxResultV2 {
    /// Item results aligned to the prepared batch payload order.
    pub items: Vec<MorphosyntaxItemResultV2>,
}

/// One utseg item result returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct UtsegItemResultV2 {
    /// Direct word-group assignments when inference succeeded without raw trees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignments: Option<Vec<usize>>,
    /// Raw constituency trees when inference succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trees: Option<Vec<String>>,
    /// Optional per-item runtime error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batched utterance-segmentation response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct UtsegResultV2 {
    /// Item results aligned to the prepared batch payload order.
    pub items: Vec<UtsegItemResultV2>,
}

/// One translation item result returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct TranslationItemResultV2 {
    /// Raw model translation when inference succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_translation: Option<String>,
    /// Optional per-item runtime error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batched translation response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct TranslationResultV2 {
    /// Item results aligned to the prepared batch payload order.
    pub items: Vec<TranslationItemResultV2>,
}

/// One structured coreference chain reference returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct CorefChainRefV2 {
    /// Chain identifier assigned by the coreference runtime.
    pub chain_id: usize,
    /// Whether the current word starts this mention.
    pub is_start: bool,
    /// Whether the current word ends this mention.
    pub is_end: bool,
}

/// One per-sentence coreference annotation returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct CorefAnnotationV2 {
    /// Sentence index inside the corresponding document item.
    pub sentence_idx: usize,
    /// Per-word chain references parallel to the sentence words.
    pub words: Vec<Vec<CorefChainRefV2>>,
}

/// One coreference item result returned by Python.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct CorefItemResultV2 {
    /// Structured sparse annotations when inference succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<CorefAnnotationV2>>,
    /// Optional per-item runtime error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batched coreference response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct CorefResultV2 {
    /// Item results aligned to the prepared batch payload order.
    pub items: Vec<CorefItemResultV2>,
}

/// One raw speaker diarization segment returned by Python.
///
/// Timing fields validated upstream by Python Pydantic models (see module docs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SpeakerSegmentV2 {
    /// Segment start in milliseconds.
    pub start_ms: DurationMs,
    /// Segment end in milliseconds.
    pub end_ms: DurationMs,
    /// Stable speaker label chosen by the model adapter.
    pub speaker: String,
}

/// Raw speaker diarization output returned by the model host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SpeakerResultV2 {
    /// Ordered diarization segments.
    pub segments: Vec<SpeakerSegmentV2>,
}

/// Raw openSMILE output returned by the model host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct OpenSmileResultV2 {
    /// Requested feature-set name.
    pub feature_set: String,
    /// Requested feature-level name.
    pub feature_level: String,
    /// Number of extracted feature columns.
    pub num_features: u64,
    /// Number of result rows/segments.
    pub duration_segments: u64,
    /// Source audio identifier echoed by the worker.
    pub audio_file: String,
    /// Tabular feature rows keyed by feature name.
    pub rows: Vec<std::collections::BTreeMap<String, f64>>,
    /// Whether the underlying runtime succeeded.
    pub success: bool,
    /// Optional runtime error detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Raw AVQI output returned by the model host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct AvqiResultV2 {
    /// AVQI score.
    pub avqi: f64,
    /// Cepstral Peak Prominence Smoothed.
    pub cpps: f64,
    /// Harmonics-to-noise ratio.
    pub hnr: f64,
    /// Local shimmer percentage.
    pub shimmer_local: f64,
    /// Local shimmer in dB.
    pub shimmer_local_db: f64,
    /// LTAS slope.
    pub slope: f64,
    /// LTAS tilt.
    pub tilt: f64,
    /// Continuous-speech file label echoed by the worker.
    pub cs_file: String,
    /// Sustained-vowel file label echoed by the worker.
    pub sv_file: String,
    /// Whether the runtime succeeded.
    pub success: bool,
    /// Optional runtime error detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Typed execute result payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskResultV2 {
    /// Whisper chunk output.
    WhisperChunkResult(WhisperChunkResultV2),
    /// Provider-shaped speaker monologue output.
    MonologueAsrResult(MonologueAsrResultV2),
    /// Raw Whisper FA token timings.
    WhisperTokenTimingResult(WhisperTokenTimingResultV2),
    /// Forced-alignment indexed timings.
    IndexedWordTimingResult(IndexedWordTimingResultV2),
    /// Batched morphosyntax result.
    MorphosyntaxResult(MorphosyntaxResultV2),
    /// Batched utterance-segmentation result.
    UtsegResult(UtsegResultV2),
    /// Batched translation result.
    TranslationResult(TranslationResultV2),
    /// Batched coreference result.
    CorefResult(CorefResultV2),
    /// Raw speaker diarization result.
    SpeakerResult(SpeakerResultV2),
    /// Raw openSMILE result.
    OpensmileResult(OpenSmileResultV2),
    /// Raw AVQI result.
    AvqiResult(AvqiResultV2),
}

/// Top-level execute outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecuteOutcomeV2 {
    /// Request completed successfully.
    Success,
    /// Request failed at the protocol/runtime boundary.
    Error {
        /// Stable protocol error category.
        code: ProtocolErrorCodeV2,
        /// Human-readable detail for logs and tests.
        message: String,
    },
}

/// Top-level V2 execute response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct ExecuteResponseV2 {
    /// Correlation id for the request.
    pub request_id: WorkerRequestIdV2,
    /// Success or typed protocol/runtime error.
    pub outcome: ExecuteOutcomeV2,
    /// Typed task result when execution succeeded.
    #[serde(default)]
    pub result: Option<TaskResultV2>,
    /// Execution time in seconds.
    pub elapsed_s: DurationSeconds,
}

/// Progress event emitted by long-running V2 tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ProgressEventV2 {
    /// Correlation id of the request being updated.
    pub request_id: WorkerRequestIdV2,
    /// Completed units.
    pub completed: u32,
    /// Total units expected.
    pub total: u32,
    /// Stable progress stage label.
    pub stage: String,
}

/// Shutdown request sent to a V2 worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ShutdownRequestV2 {
    /// Correlation id for the shutdown message.
    pub request_id: WorkerRequestIdV2,
}
