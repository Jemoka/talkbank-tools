//! Forced-alignment proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::FaInput, FaOutput`. !!!

use crate::cache::{hash_serialized, CacheKey};
use crate::proto::asr::{AsrSegment, LanguageSpec};
use crate::register_proto_schema;
use crate::utils::PreparedAudio;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input to a forced-alignment backend: the audio plus the already-segmented
/// utterances (text-only; FA fills word-level timings).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FaInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
    /// Per-utterance text + (currently rough) bounds. FA refines word
    /// timings inside `words`.
    pub utterances: Vec<AsrSegment>,
    /// Language hint, frequently `LanguageSpec::PerFile`.
    pub language: LanguageSpec,
}

impl CacheKey for FaInput {
    /// Excludes `source_id` AND the utterance time bounds.
    ///
    /// The FA runner rewrites each utterance's main-tier bullet to span
    /// the aligned words after a successful run (see
    /// `taskrunners/fa.rs::inject_word_timings`), so a second run reads
    /// the refined bounds back into `AsrSegment.start_ms/end_ms`. If those
    /// bounds participated in the cache key, every successful run would
    /// permanently invalidate its own cache entry. They are soft hints to
    /// the backend's audio-slicer anyway — audio bytes and word texts
    /// fully determine the alignment.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct UttK<'a> {
            text: &'a str,
            speaker: Option<&'a str>,
            words: Vec<&'a str>,
        }
        #[derive(Serialize)]
        struct K<'a> {
            audio: &'a PreparedAudio,
            utterances: Vec<UttK<'a>>,
            language: &'a LanguageSpec,
        }
        let utterances: Vec<UttK<'_>> = self
            .utterances
            .iter()
            .map(|u| UttK {
                text: u.text.as_str(),
                speaker: u.speaker.as_ref().map(|s| s.as_str()),
                words: u.words.iter().map(|w| w.text.as_str()).collect(),
            })
            .collect();
        hash_serialized(
            &K {
                audio: &self.audio,
                utterances,
                language: &self.language,
            },
            hasher,
        );
    }
}

/// Output: same utterances, with refined `words[*].start_ms / end_ms`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FaOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Utterances with refined word timings.
    pub utterances: Vec<AsrSegment>,
}

register_proto_schema!(FaInput);
register_proto_schema!(FaOutput);
