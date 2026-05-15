//! ASR and speaker inference dispatch to worker backends.

use std::path::Path;

use super::types::{AsrBackend, AsrResponse, AsrWorkerMode};
use crate::api::{LanguageCode3, LanguageSpec, NumSpeakers, WorkerLanguage};
use crate::error::ServerError;
use crate::revai::infer_revai_asr;
use crate::types::worker_v2::{SpeakerBackendV2, SpeakerSegmentV2};
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::asr_request_v2::{
    AsrBuildInputV2, AsrInputSourceV2, PreparedAsrRequestIdsV2, build_asr_request_v2,
};
use crate::worker::asr_result_v2::parse_asr_response_v2;
use crate::worker::pool::WorkerPool;
use crate::worker::speaker_request_v2::{
    PreparedSpeakerRequestIdsV2, SpeakerBuildInputV2, build_speaker_request_v2,
};
use crate::worker::speaker_result_v2::parse_speaker_result_v2;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parameters for ASR worker inference.
pub(crate) struct AsrInferParams<'a> {
    /// Which runtime boundary owns raw ASR inference.
    pub backend: AsrBackend,
    /// Audio file to transcribe.
    pub audio_path: &'a Path,
    /// Language specification for ASR dispatch. May be `Auto` — the GPU
    /// worker and ASR engine handle auto-detect internally.
    pub lang: &'a LanguageSpec,
    /// Expected number of speakers for diarization.
    pub num_speakers: NumSpeakers,
    /// Rev.AI pre-submitted job ID (from preflight).
    pub rev_job_id: Option<&'a str>,
}

/// Parameters for dedicated speaker-diarization inference.
pub(crate) struct SpeakerInferParams<'a> {
    /// Audio file to diarize.
    pub audio_path: &'a Path,
    /// Language specification for worker dispatch. May be `Auto`.
    pub lang: &'a LanguageSpec,
    /// Expected number of speakers when known.
    pub expected_speakers: NumSpeakers,
    /// Dedicated diarization backend chosen by Rust.
    pub backend: SpeakerBackendV2,
}

/// Compute the worker-runtime language and an "expected response
/// language" hint used by `parse_asr_response_v2` when the ASR response
/// does not carry a usable detected language of its own.
///
/// For `Resolved(code)` jobs, both values are derived from `code` — the
/// CHAT header will reflect what the user explicitly asked for. For
/// `Auto` jobs there is no concrete hint, and the parse helper must
/// drive the language from the response itself; we return `None` so
/// the caller can surface a typed error if the response is also empty.
/// `PerFile` is not legal at this point (transcribe-class commands are
/// rejected by submission validation if they carry it).
pub(super) fn asr_worker_languages(
    lang: &LanguageSpec,
) -> Result<(WorkerLanguage, Option<LanguageCode3>), ServerError> {
    match lang {
        LanguageSpec::Resolved(code) => {
            Ok((WorkerLanguage::Resolved(code.clone()), Some(code.clone())))
        }
        LanguageSpec::Auto => Ok((WorkerLanguage::Auto, None)),
        LanguageSpec::PerFile => Err(ServerError::Validation(
            "transcribe pipeline received LanguageSpec::PerFile, which is reserved for \
             morphotag/translate/coref. Submission validation should have rejected \
             this — please file a bug report."
                .into(),
        )),
    }
}

/// Call the Python worker for ASR inference on a single audio file.
pub(crate) async fn infer_asr(
    pool: &WorkerPool,
    params: &AsrInferParams<'_>,
) -> Result<AsrResponse, ServerError> {
    let (worker_lang, fallback_lang) = asr_worker_languages(params.lang)?;

    match params.backend {
        AsrBackend::RustRevAi => {
            // Rev.AI path receives the full LanguageSpec so it can pass
            // "auto" to Rev.AI and read the detected language from the job.
            infer_revai_asr(
                params.audio_path,
                params.lang,
                params.num_speakers,
                params.rev_job_id,
            )
            .await
        }
        AsrBackend::Worker(worker_mode) => {
            infer_asr_via_worker_v2(
                pool,
                params,
                worker_mode,
                &worker_lang,
                fallback_lang.as_ref(),
            )
            .await
        }
    }
}

/// Call the live V2 Python worker path for ASR inference on a single audio
/// file and normalize its typed result into the shared Rust ASR response
/// shape.
async fn infer_asr_via_worker_v2(
    pool: &WorkerPool,
    params: &AsrInferParams<'_>,
    worker_mode: AsrWorkerMode,
    worker_lang: &WorkerLanguage,
    fallback_lang: Option<&LanguageCode3>,
) -> Result<AsrResponse, ServerError> {
    let artifacts = PreparedArtifactRuntimeV2::new("asr_v2").map_err(|error| {
        ServerError::Validation(format!("failed to create ASR V2 artifact runtime: {error}"))
    })?;
    let request = build_asr_request_v2(
        artifacts.store(),
        AsrBuildInputV2 {
            ids: &PreparedAsrRequestIdsV2::fresh(),
            input: match worker_mode {
                // Fine-tune HF Whisper shares the prepared-audio wire shape
                // with stock Whisper: Rust owns media decoding, the worker
                // receives a resampled mono waveform. The only difference
                // is which checkpoint the worker's ``WhisperASRHandle`` was
                // constructed around at bootstrap.
                AsrWorkerMode::LocalWhisperV2 | AsrWorkerMode::WhisperHubV2 => {
                    AsrInputSourceV2::PreparedAudio {
                        audio_path: params.audio_path,
                    }
                }
                AsrWorkerMode::HkTencentV2
                | AsrWorkerMode::HkAliyunV2
                | AsrWorkerMode::HkFunaudioV2 => AsrInputSourceV2::ProviderMedia {
                    media_path: params.audio_path,
                    num_speakers: params.num_speakers,
                },
            },
            lang: worker_lang,
            backend: worker_mode.as_v2_backend(),
        },
    )
    .await
    .map_err(|error| {
        ServerError::Validation(format!(
            "failed to build worker protocol V2 ASR request: {error}"
        ))
    })?;

    let response = pool
        .dispatch_execute_v2(worker_lang, &request)
        .await
        .map_err(ServerError::Worker)?;

    parse_asr_response_v2(&response, fallback_lang)
        .map_err(|error| ServerError::Validation(format!("ASR V2 response parse failed: {error}")))
}

/// Call the live V2 Python worker path for dedicated speaker diarization on a
/// single audio file.
pub(crate) async fn infer_speaker(
    pool: &WorkerPool,
    params: &SpeakerInferParams<'_>,
) -> Result<Vec<SpeakerSegmentV2>, ServerError> {
    let artifacts = PreparedArtifactRuntimeV2::new("speaker_v2").map_err(|error| {
        ServerError::Validation(format!(
            "failed to create speaker V2 artifact runtime: {error}"
        ))
    })?;
    let request = build_speaker_request_v2(
        artifacts.store(),
        SpeakerBuildInputV2 {
            ids: &PreparedSpeakerRequestIdsV2::fresh(),
            audio_path: params.audio_path,
            backend: params.backend,
            expected_speakers: Some(params.expected_speakers),
        },
    )
    .await
    .map_err(|error| {
        ServerError::Validation(format!(
            "failed to build worker protocol V2 speaker request: {error}"
        ))
    })?;

    // Speaker diarization runs after ASR has resolved the language —
    // `params.lang` should always be `Resolved(_)` by the time we reach
    // here. No silent eng fallback: surface a typed validation error if
    // the invariant is broken.
    let pool_lang = params.lang.as_resolved().cloned().ok_or_else(|| {
        ServerError::Validation(format!(
            "speaker diarization received unresolved language `{}`. ASR must \
             resolve the language before speaker dispatch runs; this is a \
             pipeline-ordering bug.",
            params.lang,
        ))
    })?;
    let response = pool
        .dispatch_execute_v2(&pool_lang, &request)
        .await
        .map_err(ServerError::Worker)?;

    parse_speaker_result_v2(&response)
        .map(|result| result.segments.clone())
        .map_err(|error| {
            ServerError::Validation(format!("speaker V2 response parse failed: {error}"))
        })
}
