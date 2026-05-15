//! Rust-side request builders for live worker-protocol V2 speaker diarization.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::api::NumSpeakers;
use crate::types::worker_v2::{
    ArtifactRefV2, ExecuteRequestV2, InferenceTaskV2, SpeakerBackendV2,
    SpeakerPreparedAudioInputV2, SpeakerRequestV2, TaskRequestV2, WorkerArtifactIdV2,
    WorkerRequestIdV2,
};

use super::artifacts_v2::{PreparedArtifactErrorV2, PreparedArtifactStoreV2};

/// Stable request ids for one V2 speaker execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSpeakerRequestIdsV2 {
    /// Top-level request id for the worker envelope.
    pub request_id: WorkerRequestIdV2,
    /// Artifact id for the prepared speaker audio payload.
    pub audio_ref_id: WorkerArtifactIdV2,
}

/// Monotonic sequence for process-unique speaker request ids. See the
/// matching [`ASR_REQUEST_SEQUENCE_V2`][crate::worker::asr_request_v2] rationale:
/// shared-GPU-worker pending-request routing is keyed by `request_id`, so
/// duplicate ids across concurrent callers cause response orphaning.
static SPEAKER_REQUEST_SEQUENCE_V2: AtomicU64 = AtomicU64::new(1);

impl PreparedSpeakerRequestIdsV2 {
    /// Construct explicit stable ids for one V2 speaker request.
    ///
    /// Intended for tests and fixtures. Concurrent production code must use
    /// [`Self::fresh`] so every request carries a process-unique id.
    pub fn new(
        request_id: impl Into<WorkerRequestIdV2>,
        audio_ref_id: impl Into<WorkerArtifactIdV2>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            audio_ref_id: audio_ref_id.into(),
        }
    }

    /// Construct unique ids for one speaker V2 request via a process-global
    /// monotonic counter.
    pub fn fresh() -> Self {
        let sequence = SPEAKER_REQUEST_SEQUENCE_V2.fetch_add(1, Ordering::Relaxed);
        Self::new(
            format!("speaker-v2-request-{sequence}"),
            format!("speaker-v2-audio-{sequence}"),
        )
    }
}

/// Input bundle for the live speaker V2 request builder.
#[derive(Debug, Clone)]
pub struct SpeakerBuildInputV2<'a> {
    /// Stable ids for the request and prepared audio.
    pub ids: &'a PreparedSpeakerRequestIdsV2,
    /// Audio file to diarize.
    pub audio_path: &'a Path,
    /// Concrete V2 speaker backend selected by Rust.
    pub backend: SpeakerBackendV2,
    /// Expected number of speakers when known.
    pub expected_speakers: Option<NumSpeakers>,
}

/// Errors produced while building a live V2 speaker request.
#[derive(Debug, Error)]
pub enum SpeakerRequestBuildErrorV2 {
    /// The request referenced an empty audio path.
    #[error("worker protocol V2 speaker request is missing an audio path")]
    MissingAudioPath,

    /// Rust-owned prepared-artifact creation failed.
    #[error(transparent)]
    Artifact(#[from] PreparedArtifactErrorV2),
}

/// Build a live worker-protocol V2 speaker request from a typed input source.
pub async fn build_speaker_request_v2(
    store: &PreparedArtifactStoreV2,
    input: SpeakerBuildInputV2<'_>,
) -> Result<ExecuteRequestV2, SpeakerRequestBuildErrorV2> {
    if input.audio_path.as_os_str().is_empty() {
        return Err(SpeakerRequestBuildErrorV2::MissingAudioPath);
    }

    let audio_attachment = store
        .prepare_audio_file_f32le(&input.ids.audio_ref_id, input.audio_path)
        .await?;

    Ok(ExecuteRequestV2 {
        request_id: input.ids.request_id.clone(),
        task: InferenceTaskV2::Speaker,
        payload: TaskRequestV2::Speaker(SpeakerRequestV2 {
            backend: input.backend,
            input: crate::types::worker_v2::SpeakerInputV2::PreparedAudio(
                SpeakerPreparedAudioInputV2 {
                    audio_ref_id: audio_attachment.id.clone(),
                },
            ),
            expected_speakers: input.expected_speakers,
        }),
        attachments: vec![ArtifactRefV2::PreparedAudio(audio_attachment)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::worker_v2::{SpeakerInputV2, TaskRequestV2};

    #[test]
    fn builds_stable_speaker_request_ids() {
        let ids = PreparedSpeakerRequestIdsV2::new("req-speaker-v2-1", "audio-speaker-v2-1");
        assert_eq!(&*ids.request_id, "req-speaker-v2-1");
        assert_eq!(&*ids.audio_ref_id, "audio-speaker-v2-1");
    }

    /// Two calls to `fresh()` must produce distinct request and artifact ids,
    /// matching the ASR regression fix.
    #[test]
    fn fresh_produces_distinct_ids_across_calls() {
        let a = PreparedSpeakerRequestIdsV2::fresh();
        let b = PreparedSpeakerRequestIdsV2::fresh();
        assert_ne!(a.request_id, b.request_id);
        assert_ne!(a.audio_ref_id, b.audio_ref_id);
    }

    #[tokio::test]
    async fn builds_prepared_audio_speaker_request() {
        use std::path::Path;

        let tempdir = tempfile::tempdir().expect("tempdir should exist");
        let store = PreparedArtifactStoreV2::new(tempdir.path().join("artifacts"))
            .expect("artifact store should exist");
        let ids = PreparedSpeakerRequestIdsV2::new("req-speaker-v2-prepared", "audio-speaker-v2");
        let media_path = tempdir.path().join("speaker-input.wav");
        // ffmpeg is a runtime prereq for align/asr commands; tests
        // must skip gracefully when it isn't installed (e.g., CI
        // runners without ffmpeg). Treat NotFound as a skip; treat
        // any other launch error as a hard failure.
        let ffmpeg_out = match tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000",
                "-t",
                "0.25",
                media_path.to_string_lossy().as_ref(),
            ])
            .output()
            .await
        {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipping: ffmpeg not installed");
                return;
            }
            Err(e) => panic!("ffmpeg process: {e}"),
        };
        if !ffmpeg_out.status.success() {
            eprintln!("skipping: could not generate test wav");
            return;
        }

        let request = build_speaker_request_v2(
            &store,
            SpeakerBuildInputV2 {
                ids: &ids,
                audio_path: &media_path,
                backend: SpeakerBackendV2::Pyannote,
                expected_speakers: Some(NumSpeakers(2)),
            },
        )
        .await
        .expect("speaker request should build");

        let TaskRequestV2::Speaker(payload) = request.payload else {
            panic!("expected speaker payload");
        };
        let SpeakerInputV2::PreparedAudio(audio_input) = payload.input;
        let Some(crate::types::worker_v2::ArtifactRefV2::PreparedAudio(attachment)) =
            request.attachments.first()
        else {
            panic!("expected prepared-audio attachment");
        };

        assert_eq!(audio_input.audio_ref_id, ids.audio_ref_id);
        assert_eq!(attachment.id, ids.audio_ref_id);
        assert!(Path::new(attachment.path.as_ref()).exists());
        assert!(attachment.byte_len.0 > 0);
    }
}
