//! Rust-side request builder for live worker-protocol V2 openSMILE.

use std::path::Path;

use thiserror::Error;

use crate::types::worker_v2::{
    ArtifactRefV2, ExecuteRequestV2, InferenceTaskV2, OpenSmileRequestV2, TaskRequestV2,
    WorkerArtifactIdV2, WorkerRequestIdV2,
};

use super::artifacts_v2::{PreparedArtifactErrorV2, PreparedArtifactStoreV2};

/// Stable ids for one V2 openSMILE request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOpenSmileRequestIdsV2 {
    /// Top-level request id for the worker envelope.
    pub request_id: WorkerRequestIdV2,
    /// Artifact id for the prepared audio payload.
    pub audio_ref_id: WorkerArtifactIdV2,
}

impl PreparedOpenSmileRequestIdsV2 {
    /// Construct the stable ids for one V2 openSMILE request.
    pub fn new(
        request_id: impl Into<WorkerRequestIdV2>,
        audio_ref_id: impl Into<WorkerArtifactIdV2>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            audio_ref_id: audio_ref_id.into(),
        }
    }
}

/// Input bundle for the live openSMILE V2 request builder.
#[derive(Debug, Clone)]
pub struct OpenSmileBuildInputV2<'a> {
    /// Stable ids for the request and prepared audio.
    pub ids: &'a PreparedOpenSmileRequestIdsV2,
    /// Audio file to analyze.
    pub audio_path: &'a Path,
    /// Requested openSMILE feature-set name.
    pub feature_set: &'a str,
    /// Requested openSMILE feature-level name.
    pub feature_level: &'a str,
}

/// Errors produced while building a live V2 openSMILE request.
#[derive(Debug, Error)]
pub enum OpenSmileRequestBuildErrorV2 {
    /// The request referenced an empty audio path.
    #[error("worker protocol V2 openSMILE request is missing an audio path")]
    MissingAudioPath,

    /// Rust-owned prepared-artifact creation failed.
    #[error(transparent)]
    Artifact(#[from] PreparedArtifactErrorV2),
}

/// Build a live worker-protocol V2 openSMILE request.
pub async fn build_opensmile_request_v2(
    store: &PreparedArtifactStoreV2,
    input: OpenSmileBuildInputV2<'_>,
) -> Result<ExecuteRequestV2, OpenSmileRequestBuildErrorV2> {
    if input.audio_path.as_os_str().is_empty() {
        return Err(OpenSmileRequestBuildErrorV2::MissingAudioPath);
    }

    let audio_attachment = store
        .prepare_audio_file_f32le(&input.ids.audio_ref_id, input.audio_path)
        .await?;

    Ok(ExecuteRequestV2 {
        request_id: input.ids.request_id.clone(),
        task: InferenceTaskV2::Opensmile,
        payload: TaskRequestV2::Opensmile(OpenSmileRequestV2 {
            audio_ref_id: audio_attachment.id.clone(),
            feature_set: input.feature_set.to_string(),
            feature_level: input.feature_level.to_string(),
        }),
        attachments: vec![ArtifactRefV2::PreparedAudio(audio_attachment)],
    })
}
