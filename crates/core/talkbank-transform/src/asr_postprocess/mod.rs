//! ASR post-processing: compound merging, number expansion, retokenization,
//! disfluency marking, and retrace detection.
//!
//! This module ports the Python ASR post-processing pipeline to Rust. After
//! the Python worker returns raw ASR tokens (via `batch_infer` with task
//! `"asr"`), the Rust server applies these transformations before utterance
//! segmentation and CHAT assembly.
//!
//! # Pipeline stages
//!
//! 1. **Compound merging** — merge adjacent words that form known compounds
//! 2. **Multi-word splitting** — split tokens containing spaces, interpolate timestamps
//! 3. **Number expansion** — convert digit strings to word form
//! 4. **Cantonese normalization** — simplified→HK traditional + domain replacements (lang=yue only)
//! 5. **Long turn splitting** — chunk monologues >300 words
//! 6. **Retokenization** — split into utterances by punctuation
//! 7. **Disfluency replacement** — mark filled pauses ("um" → "&-um") and orthographic
//!    replacements ("'cause" → "(be)cause") from per-language wordlists
//! 8. **N-gram retrace detection** — detect repeated n-grams and wrap in `<...> [/]`
//!
//! The implementation is split by stage so callers can find preparation,
//! number-expansion, chunking, and utterance-finalization logic quickly.

mod asr_types;
pub mod cantonese;
mod chunking;
mod cleanup;
mod compounds;
mod expand;
pub mod lang_detect;
mod num2chinese;
mod num2text;
mod ordinal_year_eng;
mod prepare;
pub mod registry;
mod snapshot;
#[cfg(test)]
mod tests;
mod utterance;

use serde::{Deserialize, Serialize};

pub use asr_types::{AsrNormalizedText, AsrRawText, AsrTimestampSecs, ChatWordText, SpeakerIndex};
pub use chunking::{
    finalize_words_to_chunks, finalize_words_to_chunks_with_snapshot,
    split_prepared_chunk_by_assignments,
};
pub use compounds::merge_compounds;
pub use expand::split_words_with_whitespace;
pub use num2text::{NumberExpansionMode, detect_expansion, expand_number};
pub use snapshot::AsrPipelineSnapshot;
pub use utterance::{
    finalize_utterances, prepare_asr_chunks, process_raw_asr, utterances_from_prepared_chunks,
};

use expand::expand_numbers_in_words;
use prepare::trim_word_boundaries;
pub use prepare::{prepare_words_pre_expansion, prepare_words_pre_expansion_with_snapshot};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// What role a word plays in the CHAT output.
///
/// The `build_chat` module reads this to decide how to represent the word
/// in the AST. Regular words become `UtteranceContent::Word`; retrace words
/// get wrapped in `<...> [/]` bracketed groups; filled pauses are already
/// encoded in the text as `&-um` etc. and parse normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WordKind {
    /// Normal content word (or filled pause already in `&-um` form).
    #[default]
    Regular,
    /// This word is part of a retrace group — a repeated n-gram that
    /// should be wrapped in `<...> [/]` annotation in the CHAT output.
    Retrace,
}

/// A single token from ASR output, with timing information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AsrWord {
    /// The word text (normalized through the ASR pipeline).
    pub text: AsrNormalizedText,
    /// Start time in milliseconds (None if unknown).
    pub start_ms: Option<i64>,
    /// End time in milliseconds (None if unknown).
    pub end_ms: Option<i64>,
    /// What kind of word this is (regular, retrace, etc.).
    #[serde(default)]
    pub kind: WordKind,
}

impl AsrWord {
    /// Create a regular (non-retrace) word with timing.
    pub fn new(text: impl Into<String>, start_ms: Option<i64>, end_ms: Option<i64>) -> Self {
        Self {
            text: AsrNormalizedText::new(text),
            start_ms,
            end_ms,
            kind: WordKind::default(),
        }
    }
}

/// A speaker-attributed utterance after retokenization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Utterance {
    /// Speaker index (0-based).
    pub speaker: SpeakerIndex,
    /// Words in the utterance (last word is a terminator like ".").
    pub words: Vec<AsrWord>,
    /// Detected language for this utterance (ISO 639-3), if different from
    /// the primary language. Used for `[- lang]` code-switching precodes in CHAT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// One prepared pre-CHAT chunk after ASR normalization but before utterance
/// segmentation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedMonologueChunk {
    /// Speaker index (0-based).
    pub speaker: SpeakerIndex,
    /// Normalized ASR words for this chunk.
    pub words: Vec<AsrWord>,
}

/// Raw monologue from ASR output (before post-processing).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsrMonologue {
    /// Speaker index (0-based).
    pub speaker: SpeakerIndex,
    /// Raw ASR elements.
    pub elements: Vec<AsrElement>,
}

/// What kind of raw ASR element this is.
///
/// Currently only `Text` and `Punctuation` are emitted by providers.
/// Defaults to `Text` when not specified (e.g. omitted from JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AsrElementKind {
    /// A word token.
    #[default]
    Text,
    /// A punctuation token (period, question mark, etc.).
    Punctuation,
}

/// A single element from raw ASR output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsrElement {
    /// Token text (raw from the ASR provider).
    pub value: AsrRawText,
    /// Start time in seconds.
    #[serde(default)]
    pub ts: AsrTimestampSecs,
    /// End time in seconds.
    #[serde(default)]
    pub end_ts: AsrTimestampSecs,
    /// Element kind: text or punctuation.
    #[serde(default)]
    pub kind: AsrElementKind,
}

/// Raw ASR output structure (matches Rev.AI format).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsrOutput {
    /// Speaker monologues.
    pub monologues: Vec<AsrMonologue>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// CHAT-legal sentence terminators.
pub(super) const ENDING_PUNCT: &[&str] = &[
    ".", "?", "!", "+...", "+/.", "+//.", "+/?", "+!?", "+\"/.", "+\".", "+//?", "+..?", "+.",
    "...", "(.)",
];

/// CHAT morphological punctuation markers.
///
/// Main-tier-legal separators that are NOT words — the tree-sitter
/// word fragment parser rejects them as such. `ChatWordText`'s
/// structural_check uses this list as a second short-circuit
/// alongside `Terminator::is_chat_terminator` so the ASR pipeline
/// can emit separator tokens (comma at clause boundaries, vocative
/// ‡, tag „) as regular `AsrWord` entries without tripping the
/// "must be a word" gate.
pub(super) const MOR_PUNCT: &[&str] = &["‡", "„", ","];

/// RTL punctuation that needs ASCII normalization.
pub(super) const RTL_PUNCT: &[(&str, &str)] = &[("؟", "?"), ("۔", "."), ("،", ","), ("؛", ";")];

/// Maximum words per turn before splitting.
pub(super) const MAX_TURN_LEN: usize = 300;

/// Long silence threshold used as a fallback boundary when ASR omits sentence
/// punctuation but timing gaps strongly suggest a new utterance.
pub(super) const LONG_PAUSE_SPLIT_MS: i64 = 800;

/// Common English sentence starters worth treating as utterance starts after a
/// long pause in otherwise unpunctuated ASR output.
pub(super) const LONG_PAUSE_SENTENCE_STARTERS: &[&str] = &[
    "and", "but", "did", "do", "does", "go", "have", "has", "had", "he", "how", "i", "is", "it",
    "no", "now", "okay", "so", "then", "they", "we", "well", "what", "when", "where", "who", "why",
    "yes", "you",
];

#[cfg(test)]
mod integration_tests {
    use super::{
        AsrElement, AsrElementKind, AsrMonologue, AsrOutput, AsrRawText, AsrTimestampSecs,
        SpeakerIndex, process_raw_asr,
    };

    #[test]
    fn pipeline_output_still_roundtrips_through_build_chat() {
        let parser = talkbank_parser::TreeSitterParser::new().unwrap();
        let output = AsrOutput {
            monologues: vec![AsrMonologue {
                speaker: SpeakerIndex(0),
                elements: vec![AsrElement {
                    value: AsrRawText::new(
                        "這麼搞笑?我還清了啊!我還覺得奇怪為什麼在一個三次頭的電話打工呢?",
                    ),
                    ts: AsrTimestampSecs(0.0),
                    end_ts: AsrTimestampSecs(0.0),
                    kind: AsrElementKind::Text,
                }],
            }],
        };

        let utterances = process_raw_asr(&output, "yue");
        let desc = crate::build_chat::transcript_from_asr_utterances(
            &utterances,
            &["PAR".to_string()],
            &["yue".to_string()],
            Some("05b_clip"),
            true,
        )
        .expect("test: transcript_from_asr_utterances should succeed");
        let chat = crate::build_chat::build_chat(&desc).expect("build chat");
        let serialized = crate::serialize::to_chat_string(&chat);
        let (_parsed, errors) = crate::parse::parse_lenient(&parser, &serialized);
        assert!(
            errors.is_empty(),
            "generated CHAT should reparse cleanly: {errors:?}"
        );
    }
}
