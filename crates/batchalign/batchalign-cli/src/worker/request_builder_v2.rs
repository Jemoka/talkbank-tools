//! Rust-side request builders for the staged worker protocol V2.
//!
//! The V2 schema already exists and is drift-tested across Rust and Python, but
//! production worker dispatch still uses the legacy request format. This module
//! is the next migration seam:
//!
//! - take an existing production-domain infer item
//! - materialize prepared artifacts owned by Rust
//! - build a typed V2 request without changing live worker dispatch yet
//!
//! Forced alignment is the first target because it has historically depended on
//! Python-side audio loading and window extraction. The builder below makes the
//! future contract explicit: Rust prepares the audio window and the token arrays
//! before Python sees the request.

use std::path::Path;

use crate::chat_ops::fa::{FaEngineType, FaInferItem, FaTimingMode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::worker_v2::{
    ArtifactRefV2, ExecuteRequestV2, FaBackendV2, FaTextModeV2, ForcedAlignmentRequestV2,
    InferenceTaskV2, TaskRequestV2, WorkerArtifactIdV2, WorkerRequestIdV2,
};

use super::artifacts_v2::{PreparedArtifactErrorV2, PreparedArtifactStoreV2};
use crate::api::DurationMs;

/// Prepared text payload that Rust writes for one V2 forced-alignment request.
///
/// The payload stays close to the existing `FaInferItem` arrays so the first
/// migration can stay behavior-preserving. The important architectural shift is
/// that these arrays now live in a Rust-owned artifact rather than being
/// reconstructed ad hoc inside Python worker logic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedFaPayloadV2 {
    /// Cleaned transcript words aligned to the model input.
    pub words: Vec<String>,
    /// Stable word identifiers aligned 1:1 with `words`.
    pub word_ids: Vec<String>,
    /// Utterance indices aligned 1:1 with `words`.
    pub word_utterance_indices: Vec<usize>,
    /// Word indices inside each utterance aligned 1:1 with `words`.
    pub word_utterance_word_indices: Vec<usize>,
}

impl PreparedFaPayloadV2 {
    /// Build the prepared payload from the existing production-domain infer
    /// item.
    pub fn from_infer_item(infer_item: &FaInferItem) -> Self {
        Self {
            words: infer_item.words.clone(),
            word_ids: infer_item.word_ids.clone(),
            word_utterance_indices: infer_item.word_utterance_indices.clone(),
            word_utterance_word_indices: infer_item.word_utterance_word_indices.clone(),
        }
    }
}

/// Stable ids for the prepared artifacts and request envelope of one V2 FA
/// request.
///
/// Keeping the ids grouped in one type prevents another primitive-heavy helper
/// signature and makes the staged builder easier to swap into live dispatch
/// later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFaRequestIdsV2 {
    /// Top-level request id for the future worker envelope.
    pub request_id: WorkerRequestIdV2,
    /// Artifact id for the prepared text payload.
    pub payload_ref_id: WorkerArtifactIdV2,
    /// Artifact id for the prepared audio window.
    pub audio_ref_id: WorkerArtifactIdV2,
}

impl PreparedFaRequestIdsV2 {
    /// Construct the stable ids for one staged V2 FA request.
    pub fn new(
        request_id: impl Into<WorkerRequestIdV2>,
        payload_ref_id: impl Into<WorkerArtifactIdV2>,
        audio_ref_id: impl Into<WorkerArtifactIdV2>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            payload_ref_id: payload_ref_id.into(),
            audio_ref_id: audio_ref_id.into(),
        }
    }
}

/// Input bundle for the staged forced-alignment V2 request builder.
#[derive(Debug, Clone)]
pub struct ForcedAlignmentBuildInputV2<'a> {
    /// Stable ids for the request and prepared artifacts.
    pub ids: &'a PreparedFaRequestIdsV2,
    /// Existing production infer item being migrated toward V2.
    pub infer_item: &'a FaInferItem,
    /// FA engine selected by the Rust control plane.
    pub engine: FaEngineType,
}

/// Errors produced while building a staged V2 forced-alignment request.
#[derive(Debug, Error)]
pub enum ForcedAlignmentRequestBuildErrorV2 {
    /// The infer item contained inconsistent word arrays.
    #[error("forced-alignment infer item field {field} has length {actual}, expected {expected}")]
    MismatchedWordArrayLength {
        /// Field name that did not match the canonical `words` length.
        field: &'static str,
        /// Canonical word count from `words`.
        expected: usize,
        /// Actual length observed in the mismatched field.
        actual: usize,
    },

    /// The infer item referenced an invalid or empty audio path.
    #[error("forced-alignment infer item is missing an audio path")]
    MissingAudioPath,

    /// Audio window bounds were invalid.
    #[error("forced-alignment infer item has invalid audio window start={start_ms} end={end_ms}")]
    InvalidAudioWindow {
        /// Inclusive start of the FA audio window.
        start_ms: DurationMs,
        /// Exclusive end of the FA audio window.
        end_ms: DurationMs,
    },

    /// The requested audio segment produced zero samples — the segment is
    /// entirely past the end of the source file.  Callers should skip the
    /// affected FA group rather than propagating a hard failure.
    #[error(
        "empty audio segment for [{start_ms}ms..{end_ms}ms) in {path}: \
         segment is past the end of the audio file"
    )]
    EmptyAudioSegment {
        /// Source media path.
        path: String,
        /// Requested segment start (milliseconds).
        start_ms: u64,
        /// Requested segment end (milliseconds).
        end_ms: u64,
    },

    /// Rust-owned prepared-artifact creation failed.
    #[error(transparent)]
    Artifact(PreparedArtifactErrorV2),
}

/// Build a staged worker-protocol V2 forced-alignment request from the
/// existing production infer item.
///
/// This function intentionally does not dispatch the request. Its only job is
/// to prove that Rust can already build the future V2 request shape:
///
/// - write the FA word arrays as a Rust-owned prepared text artifact
/// - extract the model-ready audio window as prepared PCM
/// - return a typed V2 `ExecuteRequest`
pub async fn build_forced_alignment_request_v2(
    store: &PreparedArtifactStoreV2,
    input: ForcedAlignmentBuildInputV2<'_>,
) -> Result<ExecuteRequestV2, ForcedAlignmentRequestBuildErrorV2> {
    validate_fa_infer_item(input.infer_item)?;

    let payload = PreparedFaPayloadV2::from_infer_item(input.infer_item);
    let payload_attachment = store
        .write_prepared_text_json(&input.ids.payload_ref_id, &payload)
        .map_err(PreparedArtifactErrorV2::Io)
        .map_err(ForcedAlignmentRequestBuildErrorV2::Artifact)?;
    let audio_attachment = store
        .extract_prepared_audio_segment_f32le(
            &input.ids.audio_ref_id,
            Path::new(&input.infer_item.audio_path),
            DurationMs(input.infer_item.audio_start_ms),
            DurationMs(input.infer_item.audio_end_ms),
        )
        .await
        .map_err(|err| match err {
            PreparedArtifactErrorV2::EmptyAudioSegment {
                path,
                start_ms,
                end_ms,
            } => ForcedAlignmentRequestBuildErrorV2::EmptyAudioSegment {
                path,
                start_ms,
                end_ms,
            },
            other => ForcedAlignmentRequestBuildErrorV2::Artifact(other),
        })?;

    let backend = fa_backend_for_engine(input.engine);
    let request = ExecuteRequestV2 {
        request_id: input.ids.request_id.clone(),
        task: InferenceTaskV2::ForcedAlignment,
        payload: TaskRequestV2::ForcedAlignment(ForcedAlignmentRequestV2 {
            backend,
            payload_ref_id: payload_attachment.id.clone(),
            audio_ref_id: audio_attachment.id.clone(),
            text_mode: text_mode_for_backend(backend),
            pauses: matches!(input.infer_item.timing_mode, FaTimingMode::WithPauses),
        }),
        attachments: vec![
            ArtifactRefV2::PreparedText(payload_attachment),
            ArtifactRefV2::PreparedAudio(audio_attachment),
        ],
    };

    Ok(request)
}

/// Map the current production FA engine selector onto the staged V2 backend
/// vocabulary.
pub(crate) fn fa_backend_for_engine(engine: FaEngineType) -> FaBackendV2 {
    match engine {
        FaEngineType::WhisperFa => FaBackendV2::Whisper,
        FaEngineType::Wave2Vec => FaBackendV2::Wave2vec,
    }
}

/// Return the required text-joining mode for one V2 FA backend.
pub(crate) fn text_mode_for_backend(backend: FaBackendV2) -> FaTextModeV2 {
    match backend {
        FaBackendV2::Whisper | FaBackendV2::Wave2vec => FaTextModeV2::SpaceJoined,
        FaBackendV2::Wav2vecCanto => FaTextModeV2::CharJoined,
    }
}

/// Validate that the existing infer item is coherent enough to be frozen into a
/// prepared-artifact request.
fn validate_fa_infer_item(
    infer_item: &FaInferItem,
) -> Result<(), ForcedAlignmentRequestBuildErrorV2> {
    let expected = infer_item.words.len();
    for (field, actual) in [
        ("word_ids", infer_item.word_ids.len()),
        (
            "word_utterance_indices",
            infer_item.word_utterance_indices.len(),
        ),
        (
            "word_utterance_word_indices",
            infer_item.word_utterance_word_indices.len(),
        ),
    ] {
        if actual != expected {
            return Err(
                ForcedAlignmentRequestBuildErrorV2::MismatchedWordArrayLength {
                    field,
                    expected,
                    actual,
                },
            );
        }
    }

    if infer_item.audio_path.trim().is_empty() {
        return Err(ForcedAlignmentRequestBuildErrorV2::MissingAudioPath);
    }

    if infer_item.audio_end_ms <= infer_item.audio_start_ms {
        return Err(ForcedAlignmentRequestBuildErrorV2::InvalidAudioWindow {
            start_ms: DurationMs(infer_item.audio_start_ms),
            end_ms: DurationMs(infer_item.audio_end_ms),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::chat_ops::fa::FaTimingMode;

    use super::*;
    use crate::types::worker_v2::{
        PreparedAudioEncodingV2, PreparedTextEncodingV2, PreparedTextRefV2, TaskRequestV2,
    };

    /// Create a temporary prepared-artifact store for builder tests.
    fn test_store() -> (PreparedArtifactStoreV2, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PreparedArtifactStoreV2::new(dir.path()).expect("prepared artifact store");
        (store, dir)
    }

    /// Return a small infer item for staged V2 builder tests.
    fn test_infer_item(audio_path: &Path) -> FaInferItem {
        FaInferItem {
            words: vec!["hello".into(), "world".into()],
            word_ids: vec!["u0:w0".into(), "u0:w1".into()],
            word_utterance_indices: vec![0, 0],
            word_utterance_word_indices: vec![0, 1],
            audio_path: audio_path.to_string_lossy().into_owned(),
            audio_start_ms: 0,
            audio_end_ms: 100,
            timing_mode: FaTimingMode::WithPauses,
        }
    }

    /// Return whether ffmpeg is available for staged V2 audio-preparation
    /// tests.
    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Generate a short tone WAV that the staged builder can extract from.
    async fn write_test_tone(path: &Path) {
        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=660:sample_rate=16000",
                "-t",
                "0.25",
                path.to_string_lossy().as_ref(),
            ])
            .output()
            .await
            .expect("ffmpeg process should run");
        assert!(
            output.status.success(),
            "ffmpeg should generate the FA builder tone fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn validates_infer_item_word_arrays() {
        let error = validate_fa_infer_item(&FaInferItem {
            words: vec!["one".into(), "two".into()],
            word_ids: vec!["u0:w0".into()],
            word_utterance_indices: vec![0, 0],
            word_utterance_word_indices: vec![0, 1],
            audio_path: "fixture.wav".into(),
            audio_start_ms: 0,
            audio_end_ms: 100,
            timing_mode: FaTimingMode::Continuous,
        })
        .expect_err("mismatched arrays should fail");

        assert!(matches!(
            error,
            ForcedAlignmentRequestBuildErrorV2::MismatchedWordArrayLength {
                field: "word_ids",
                expected: 2,
                actual: 1,
            }
        ));
    }

    #[test]
    fn maps_fa_engines_to_typed_v2_backend_contract() {
        assert_eq!(
            fa_backend_for_engine(FaEngineType::WhisperFa),
            FaBackendV2::Whisper
        );
        assert_eq!(
            fa_backend_for_engine(FaEngineType::Wave2Vec),
            FaBackendV2::Wave2vec
        );
        assert_eq!(
            text_mode_for_backend(FaBackendV2::Whisper),
            FaTextModeV2::SpaceJoined
        );
        assert_eq!(
            text_mode_for_backend(FaBackendV2::Wav2vecCanto),
            FaTextModeV2::CharJoined
        );
    }

    #[tokio::test]
    async fn builds_forced_alignment_execute_request_with_prepared_artifacts() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let (store, dir) = test_store();
        let wav_path = dir.path().join("tone.wav");
        write_test_tone(&wav_path).await;

        let request = build_forced_alignment_request_v2(
            &store,
            ForcedAlignmentBuildInputV2 {
                ids: &PreparedFaRequestIdsV2::new(
                    "req-fa-build-1",
                    "payload-fa-build-1",
                    "audio-fa-build-1",
                ),
                infer_item: &test_infer_item(&wav_path),
                engine: FaEngineType::WhisperFa,
            },
        )
        .await
        .expect("staged FA V2 request should build");

        assert_eq!(request.task, InferenceTaskV2::ForcedAlignment);
        let TaskRequestV2::ForcedAlignment(payload) = &request.payload else {
            panic!("expected forced-alignment payload");
        };
        assert_eq!(payload.backend, FaBackendV2::Whisper);
        assert_eq!(payload.text_mode, FaTextModeV2::SpaceJoined);
        assert!(payload.pauses);
        assert_eq!(request.attachments.len(), 2);

        let ArtifactRefV2::PreparedText(PreparedTextRefV2 {
            id, path, encoding, ..
        }) = &request.attachments[0]
        else {
            panic!("first attachment should be prepared text");
        };
        assert_eq!(id.as_ref(), "payload-fa-build-1");
        assert_eq!(*encoding, PreparedTextEncodingV2::Utf8Json);
        let payload_raw = fs::read_to_string(path.as_ref()).expect("read prepared payload");
        let payload_json: PreparedFaPayloadV2 =
            serde_json::from_str(&payload_raw).expect("parse prepared payload");
        assert_eq!(
            payload_json.words,
            vec!["hello".to_string(), "world".to_string()]
        );

        let ArtifactRefV2::PreparedAudio(audio) = &request.attachments[1] else {
            panic!("second attachment should be prepared audio");
        };
        assert_eq!(audio.id.as_ref(), "audio-fa-build-1");
        assert_eq!(audio.encoding, PreparedAudioEncodingV2::PcmF32le);
        assert_eq!(audio.channels, 1);
        assert_eq!(audio.sample_rate_hz, 16_000);
        assert!(audio.frame_count.0 > 0);
        assert!(Path::new(audio.path.as_ref()).exists());
    }
}
