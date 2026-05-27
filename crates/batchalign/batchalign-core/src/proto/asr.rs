//! ASR proto types.
//!
//! !!!  THIS CONTRACT IS BRITTLE  !!!
//! Hand-mirrored with `python/batchalign/_core/proto.py::AsrInput, AsrOutput,
//! AsrSegment, AsrWord, AsrOptions, LanguageSpec, PreparedAudio`.
//! Edits MUST happen in both places. Tests in
//! `tests/proto_parity.rs` pin existence only.

use crate::register_proto_schema;
use crate::utils::{PreparedAudio, SpeakerLabel};
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Language specifier for an ASR / FA / translate task.
///
/// Tagged enum so Python's `LanguageSpec.auto()` and `LanguageSpec.code("eng")`
/// round-trip through serde.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LanguageSpec {
    /// Provider auto-detects.
    Auto,
    /// Pin to a specific 3-letter code (e.g. `"eng"`, `"yue"`).
    Code(SmolStr),
    /// Resolve per-file from the CHAT's `@Languages:` header.
    PerFile,
}

impl Default for LanguageSpec {
    fn default() -> Self {
        LanguageSpec::Auto
    }
}

/// Tunables passed to the ASR backend.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct AsrOptions {
    /// Hint for the diarizer; 0 means auto.
    #[serde(default)]
    pub num_speakers: u32,
    /// Optional initial prompt / biasing text.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Free-form backend-specific extras, opaque to the kernel.
    #[serde(default)]
    pub extras: serde_json::Value,
}

/// One time-aligned word emitted by ASR.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AsrWord {
    /// Surface text.
    pub text: String,
    /// Start in milliseconds since file start.
    pub start_ms: u64,
    /// End in milliseconds since file start.
    pub end_ms: u64,
    /// Optional model confidence in [0, 1].
    #[serde(default)]
    pub confidence: Option<f32>,
}

/// One ASR segment — speaker + word sequence + bounded time span.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AsrSegment {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Joined transcript text.
    pub text: String,
    /// Optional diarization label.
    #[serde(default)]
    pub speaker: Option<SpeakerLabel>,
    /// Word-level timings (may be empty if backend doesn't emit them).
    #[serde(default)]
    pub words: Vec<AsrWord>,
}

/// What the ASR runner ships to its backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AsrInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
    /// Language hint.
    pub language: LanguageSpec,
    /// Backend tunables.
    pub options: AsrOptions,
}

/// What an ASR backend returns.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AsrOutput {
    /// Echoes the input's `source_id` for routing.
    pub source_id: SourceId,
    /// Ordered segments, time-monotonic.
    pub segments: Vec<AsrSegment>,
}

register_proto_schema!(LanguageSpec);
register_proto_schema!(AsrOptions);
register_proto_schema!(AsrWord);
register_proto_schema!(AsrSegment);
register_proto_schema!(AsrInput);
register_proto_schema!(AsrOutput);
