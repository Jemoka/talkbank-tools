use serde::Deserialize;

use crate::asr_postprocess;

/// Structured description of a transcript to be assembled into CHAT format.
///
/// Fields mirror the JSON format accepted by the PyO3 `build_chat()` function.
#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptDescription {
    /// ISO 639-3 language codes (e.g. `["eng"]`). Defaults to `["eng"]` if empty.
    #[serde(default)]
    pub langs: Vec<String>,
    /// Participant entries. At least one is required.
    pub participants: Vec<ParticipantDesc>,
    /// Optional media filename (e.g. `"recording.mp3"`).
    pub media_name: Option<String>,
    /// Optional media type (`"audio"` or `"video"`). Defaults to `"audio"`.
    pub media_type: Option<String>,
    /// Utterances to include in the transcript.
    #[serde(default)]
    pub utterances: Vec<UtteranceDesc>,
    /// Whether to generate `%wor` tiers when word-level timing is available.
    ///
    /// Defaults to `false` (BA2 parity: transcribe omits `%wor` unless
    /// explicitly requested via `--wor`). The JSON bridge (PyO3) defaults to
    /// `false` via serde; callers that want `%wor` must set this to `true`.
    #[serde(default)]
    pub write_wor: bool,
}

/// A participant in the transcript.
#[derive(Debug, Clone, Deserialize)]
pub struct ParticipantDesc {
    /// Speaker code (e.g. `"PAR"`, `"INV"`, `"CHI"`).
    pub id: String,
    /// Participant name for `@Participants` header. `None` omits the name
    /// field (output: `CODE Role`). `Some("...")` adds it (output: `CODE Name Role`).
    pub name: Option<String>,
    /// Participant role (e.g. `"Participant"`, `"Investigator"`, `"Target_Child"`).
    /// Callers should always set this — derive from speaker code via
    /// `role_for_speaker_code` if unknown. Defaults to `"Participant"` only
    /// for JSON backward compatibility.
    #[serde(default = "default_participant_role")]
    pub role: String,
    /// Corpus name for `@ID` header. Empty string if unknown.
    #[serde(default)]
    pub corpus: String,
}

/// An utterance in the transcript.
///
/// Either `words` (word-level with individual timings) or `text` (parse as
/// a single CHAT utterance line) should be provided. If both are present,
/// `words` takes precedence (when non-empty).
#[derive(Debug, Clone, Deserialize)]
pub struct UtteranceDesc {
    /// Speaker code for this utterance.
    pub speaker: String,
    /// Word-level tokens with optional per-word timing.
    pub words: Option<Vec<WordDesc>>,
    /// Full utterance text (alternative to word-level). Parsed via tree-sitter.
    ///
    /// This is a public API surface for callers who want to pass pre-formatted
    /// CHAT text rather than individual word tokens. The text is wrapped in a
    /// mini CHAT document and parsed by `build_text_utterance()`. Currently
    /// unused by the ASR pipeline (which always provides `words`), but
    /// preserved for external JSON API consumers.
    pub text: Option<String>,
    /// Utterance-level start time in ms (used with `text` mode).
    pub start_ms: Option<u64>,
    /// Utterance-level end time in ms (used with `text` mode).
    pub end_ms: Option<u64>,
    /// Detected language for this utterance (ISO 639-3). When set and different
    /// from the primary language (`langs[0]`), a `[- lang]` precode is prepended.
    #[serde(default)]
    pub lang: Option<String>,
}

/// A single word token with optional timing.
#[derive(Debug, Clone, Deserialize)]
pub struct WordDesc {
    /// Word text (ready for CHAT assembly via TreeSitterParser).
    pub text: asr_postprocess::ChatWordText,
    /// Start time in milliseconds.
    pub start_ms: Option<u64>,
    /// End time in milliseconds.
    pub end_ms: Option<u64>,
    /// What role this word plays (regular, retrace, etc.).
    #[serde(default)]
    pub kind: asr_postprocess::WordKind,
}

/// Derive the default participant role used by the JSON bridge.
pub(super) fn default_participant_role() -> String {
    "Participant".to_string()
}
