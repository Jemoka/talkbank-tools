//! Rust-side request builder for live worker-protocol V2 AVQI.

use std::path::Path;

use thiserror::Error;

use crate::types::worker_v2::{
    ArtifactRefV2, AvqiRequestV2, ExecuteRequestV2, InferenceTaskV2, TaskRequestV2,
    WorkerArtifactIdV2, WorkerRequestIdV2,
};

use super::artifacts_v2::{PreparedArtifactErrorV2, PreparedArtifactStoreV2};

/// Stable ids for one V2 AVQI request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAvqiRequestIdsV2 {
    /// Top-level request id for the worker envelope.
    pub request_id: WorkerRequestIdV2,
    /// Artifact id for the prepared continuous-speech audio.
    pub cs_audio_ref_id: WorkerArtifactIdV2,
    /// Artifact id for the prepared sustained-vowel audio.
    pub sv_audio_ref_id: WorkerArtifactIdV2,
}

impl PreparedAvqiRequestIdsV2 {
    /// Construct the stable ids for one V2 AVQI request.
    pub fn new(
        request_id: impl Into<WorkerRequestIdV2>,
        cs_audio_ref_id: impl Into<WorkerArtifactIdV2>,
        sv_audio_ref_id: impl Into<WorkerArtifactIdV2>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            cs_audio_ref_id: cs_audio_ref_id.into(),
            sv_audio_ref_id: sv_audio_ref_id.into(),
        }
    }
}

/// Input bundle for the live AVQI V2 request builder.
#[derive(Debug, Clone)]
pub struct AvqiBuildInputV2<'a> {
    /// Stable ids for the request and prepared audio artifacts.
    pub ids: &'a PreparedAvqiRequestIdsV2,
    /// Continuous-speech audio file.
    pub cs_audio_path: &'a Path,
    /// Sustained-vowel audio file.
    pub sv_audio_path: &'a Path,
}

/// Errors produced while building a live V2 AVQI request.
#[derive(Debug, Error)]
pub enum AvqiRequestBuildErrorV2 {
    /// One of the paired audio paths was empty.
    #[error("worker protocol V2 AVQI request requires both cs and sv audio paths")]
    MissingAudioPath,

    /// Rust-owned prepared-artifact creation failed.
    #[error(transparent)]
    Artifact(#[from] PreparedArtifactErrorV2),
}

/// Build a live worker-protocol V2 AVQI request.
pub async fn build_avqi_request_v2(
    store: &PreparedArtifactStoreV2,
    input: AvqiBuildInputV2<'_>,
) -> Result<ExecuteRequestV2, AvqiRequestBuildErrorV2> {
    if input.cs_audio_path.as_os_str().is_empty() || input.sv_audio_path.as_os_str().is_empty() {
        return Err(AvqiRequestBuildErrorV2::MissingAudioPath);
    }

    let cs_attachment = store
        .prepare_audio_file_f32le(&input.ids.cs_audio_ref_id, input.cs_audio_path)
        .await?;
    let sv_attachment = store
        .prepare_audio_file_f32le(&input.ids.sv_audio_ref_id, input.sv_audio_path)
        .await?;

    Ok(ExecuteRequestV2 {
        request_id: input.ids.request_id.clone(),
        task: InferenceTaskV2::Avqi,
        payload: TaskRequestV2::Avqi(AvqiRequestV2 {
            cs_audio_ref_id: cs_attachment.id.clone(),
            sv_audio_ref_id: sv_attachment.id.clone(),
        }),
        attachments: vec![
            ArtifactRefV2::PreparedAudio(cs_attachment),
            ArtifactRefV2::PreparedAudio(sv_attachment),
        ],
    })
}
