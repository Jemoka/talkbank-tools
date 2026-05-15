//! Rust-owned worker-protocol V2 ASR executor control plane.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "2. ASR Execution V2" for:
//! - Python caller: `python/batchalign/worker/_asr_v2.py::execute_asr_request_v2()`
//! - Full Rust/Python responsibility split and input/output contracts.

use std::time::Instant;

use batchalign_types::api::{DurationSeconds, LanguageCode3};
use batchalign_types::worker_v2::{
    AsrBackendV2, AsrElementKindV2, AsrElementV2, AsrInputV2, AsrMonologueV2, AsrRequestV2,
    ExecuteOutcomeV2, ExecuteRequestV2, ExecuteResponseV2, InferenceTaskV2, MonologueAsrResultV2,
    ProtocolErrorCodeV2, TaskRequestV2, TaskResultV2, WhisperChunkResultV2,
};
use numpy::IntoPyArray;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::error::BatchalignBoundaryError;
use crate::py_json_bridge::py_to_json_value;
use crate::worker_artifacts::{
    load_prepared_audio_bytes_impl, require_prepared_audio_attachment,
    validate_attachment_descriptors,
};

enum AsrExecuteFailure {
    Artifact(String),
    InvalidPayload(String),
    ModelUnavailable(String),
    Runtime(String),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum ProviderSpeakerId {
    Signed(i64),
    Unsigned(u64),
    Text(String),
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProviderAsrElementInput {
    value: String,
    #[serde(default)]
    ts: Option<f64>,
    #[serde(default)]
    end_ts: Option<f64>,
    #[serde(default = "default_provider_element_type", rename = "type")]
    type_name: String,
    #[serde(default)]
    confidence: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProviderAsrMonologueInput {
    speaker: ProviderSpeakerId,
    #[serde(default)]
    elements: Vec<ProviderAsrElementInput>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProviderAsrResponseInput {
    lang: LanguageCode3,
    #[serde(default)]
    monologues: Vec<ProviderAsrMonologueInput>,
}

fn default_provider_element_type() -> String {
    "text".to_owned()
}

fn parse_execute_request(request: &Bound<'_, PyAny>) -> PyResult<ExecuteRequestV2> {
    serde_json::from_value(py_to_json_value(request)?)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())
}

fn decode_f32le_audio(raw: Vec<u8>) -> Vec<f32> {
    raw.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn extract_asr_request<'a>(
    request: &'a ExecuteRequestV2,
) -> Result<&'a AsrRequestV2, AsrExecuteFailure> {
    if request.task != InferenceTaskV2::Asr {
        return Err(AsrExecuteFailure::InvalidPayload(format!(
            "expected asr task, got {:?}",
            request.task
        )));
    }
    match &request.payload {
        TaskRequestV2::Asr(value) => Ok(value),
        _ => Err(AsrExecuteFailure::InvalidPayload(
            "execute payload did not contain ASR request data".to_owned(),
        )),
    }
}

fn artifact_code(message: &str) -> ProtocolErrorCodeV2 {
    if message.contains("missing worker protocol V2 attachment") {
        ProtocolErrorCodeV2::MissingAttachment
    } else {
        ProtocolErrorCodeV2::AttachmentUnreadable
    }
}

fn error_response(
    request: &ExecuteRequestV2,
    code: ProtocolErrorCodeV2,
    message: String,
    started_at: Instant,
) -> ExecuteResponseV2 {
    ExecuteResponseV2 {
        request_id: request.request_id.clone(),
        outcome: ExecuteOutcomeV2::Error { code, message },
        result: None,
        elapsed_s: DurationSeconds(started_at.elapsed().as_secs_f64()),
    }
}

fn success_response(
    request: &ExecuteRequestV2,
    result: TaskResultV2,
    started_at: Instant,
) -> ExecuteResponseV2 {
    ExecuteResponseV2 {
        request_id: request.request_id.clone(),
        outcome: ExecuteOutcomeV2::Success,
        result: Some(result),
        elapsed_s: DurationSeconds(started_at.elapsed().as_secs_f64()),
    }
}

fn validate_non_negative(label: &str, value: f64) -> Result<(), AsrExecuteFailure> {
    if value < 0.0 {
        return Err(AsrExecuteFailure::Runtime(format!(
            "invalid ASR host output: {label} must be >= 0"
        )));
    }
    Ok(())
}

fn parse_whisper_result(
    response: &Bound<'_, PyAny>,
) -> Result<WhisperChunkResultV2, AsrExecuteFailure> {
    let parsed: WhisperChunkResultV2 =
        serde_json::from_value(py_to_json_value(response).map_err(|error| {
            AsrExecuteFailure::Runtime(format!("invalid ASR host output: {error}"))
        })?)
        .map_err(|error| AsrExecuteFailure::Runtime(format!("invalid ASR host output: {error}")))?;

    for chunk in &parsed.chunks {
        validate_non_negative("Whisper chunk start_s", chunk.start_s.0)?;
        validate_non_negative("Whisper chunk end_s", chunk.end_s.0)?;
        if chunk.end_s < chunk.start_s {
            return Err(AsrExecuteFailure::Runtime(
                "invalid ASR host output: Whisper chunk end_s must be >= start_s".to_owned(),
            ));
        }
    }

    Ok(parsed)
}

fn stringify_speaker(speaker: ProviderSpeakerId) -> String {
    match speaker {
        ProviderSpeakerId::Signed(value) => value.to_string(),
        ProviderSpeakerId::Unsigned(value) => value.to_string(),
        ProviderSpeakerId::Text(value) => value,
    }
}

fn parse_provider_result(
    response: &Bound<'_, PyAny>,
) -> Result<MonologueAsrResultV2, AsrExecuteFailure> {
    let parsed: ProviderAsrResponseInput =
        serde_json::from_value(py_to_json_value(response).map_err(|error| {
            AsrExecuteFailure::Runtime(format!("invalid ASR host output: {error}"))
        })?)
        .map_err(|error| AsrExecuteFailure::Runtime(format!("invalid ASR host output: {error}")))?;

    let mut monologues = Vec::with_capacity(parsed.monologues.len());
    for monologue in parsed.monologues {
        let mut elements = Vec::with_capacity(monologue.elements.len());
        for element in monologue.elements {
            if let Some(start_s) = element.ts {
                validate_non_negative("ASR element start_s", start_s)?;
            }
            if let Some(end_s) = element.end_ts {
                validate_non_negative("ASR element end_s", end_s)?;
            }
            if let (Some(start_s), Some(end_s)) = (element.ts, element.end_ts) {
                if end_s < start_s {
                    return Err(AsrExecuteFailure::Runtime(
                        "invalid ASR host output: ASR element end_s must be >= start_s".to_owned(),
                    ));
                }
            }

            let kind = if element.type_name == "punctuation" {
                AsrElementKindV2::Punctuation
            } else {
                AsrElementKindV2::Text
            };

            elements.push(AsrElementV2 {
                value: element.value,
                start_s: element.ts.map(DurationSeconds),
                end_s: element.end_ts.map(DurationSeconds),
                kind,
                confidence: element.confidence,
            });
        }

        monologues.push(AsrMonologueV2 {
            speaker: stringify_speaker(monologue.speaker),
            elements,
        });
    }

    Ok(MonologueAsrResultV2 {
        lang: parsed.lang,
        monologues,
    })
}

fn load_local_whisper_audio(
    request: &ExecuteRequestV2,
    asr_request: &AsrRequestV2,
) -> Result<Vec<f32>, AsrExecuteFailure> {
    let audio_ref_id = match &asr_request.input {
        AsrInputV2::PreparedAudio(value) => value.audio_ref_id.as_ref(),
        _ => {
            return Err(AsrExecuteFailure::InvalidPayload(
                "ASR backend expected prepared_audio input".to_owned(),
            ));
        }
    };

    let attachment = require_prepared_audio_attachment(&request.attachments, audio_ref_id)
        .map_err(|error| AsrExecuteFailure::Artifact(error.to_string()))?;
    if attachment.channels.0 != 1 {
        return Err(AsrExecuteFailure::InvalidPayload(
            "worker protocol V2 ASR currently expects mono prepared audio".to_owned(),
        ));
    }
    Ok(decode_f32le_audio(
        load_prepared_audio_bytes_impl(attachment)
            .map_err(|error| AsrExecuteFailure::Artifact(error.to_string()))?,
    ))
}

fn build_provider_media_item<'py>(
    py: Python<'py>,
    asr_request: &AsrRequestV2,
) -> Result<Bound<'py, PyAny>, AsrExecuteFailure> {
    let provider_input = match &asr_request.input {
        AsrInputV2::ProviderMedia(value) => value,
        _ => {
            return Err(AsrExecuteFailure::InvalidPayload(
                "ASR backend expected provider_media input".to_owned(),
            ));
        }
    };

    let asr_module = PyModule::import(py, "batchalign.inference.asr")
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    let asr_batch_item = asr_module
        .getattr("AsrBatchItem")
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    let kwargs = PyDict::new(py);
    kwargs
        .set_item("audio_path", provider_input.media_path.as_ref())
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    kwargs
        .set_item("lang", asr_request.lang.as_worker_arg())
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    kwargs
        .set_item("num_speakers", provider_input.num_speakers.0)
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    asr_batch_item
        .call((), Some(&kwargs))
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))
}

fn run_local_whisper(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    asr_request: &AsrRequestV2,
    local_whisper_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, AsrExecuteFailure> {
    let audio = load_local_whisper_audio(request, asr_request)?;
    let runner = local_whisper_runner.ok_or_else(|| {
        AsrExecuteFailure::ModelUnavailable(
            "no local Whisper ASR host loaded for worker protocol V2".to_owned(),
        )
    })?;
    let audio_array = audio.into_pyarray(py);
    let response = runner
        .bind(py)
        .call1((audio_array, asr_request.lang.as_worker_arg()))
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::WhisperChunkResult(parse_whisper_result(
        &response,
    )?))
}

fn run_provider_backend(
    py: Python<'_>,
    asr_request: &AsrRequestV2,
    provider_runner: Option<Py<PyAny>>,
    unavailable_message: &'static str,
) -> Result<TaskResultV2, AsrExecuteFailure> {
    let item = build_provider_media_item(py, asr_request)?;
    let runner = provider_runner
        .ok_or_else(|| AsrExecuteFailure::ModelUnavailable(unavailable_message.to_owned()))?;
    let response = runner
        .bind(py)
        .call1((item,))
        .map_err(|error| AsrExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::MonologueAsrResult(parse_provider_result(
        &response,
    )?))
}

fn run_asr(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    local_whisper_runner: Option<Py<PyAny>>,
    hk_tencent_runner: Option<Py<PyAny>>,
    hk_aliyun_runner: Option<Py<PyAny>>,
    hk_funaudio_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, AsrExecuteFailure> {
    let asr_request = extract_asr_request(request)?;
    // ``WhisperHub`` shares the worker-side runtime shape with
    // ``LocalWhisper`` — both host a ``WhisperASRHandle`` loaded at
    // worker bootstrap and receive prepared mono audio as the request
    // input. The distinction lives at load time (which checkpoint got
    // loaded) and at the worker-pool key (so separate workers serve
    // each variant); the PyO3 dispatch treats them identically.
    if matches!(
        asr_request.backend,
        AsrBackendV2::LocalWhisper | AsrBackendV2::WhisperHub
    ) {
        return run_local_whisper(py, request, asr_request, local_whisper_runner);
    }

    match asr_request.backend {
        AsrBackendV2::HkTencent => run_provider_backend(
            py,
            asr_request,
            hk_tencent_runner,
            "no Tencent ASR host loaded for worker protocol V2",
        ),
        AsrBackendV2::HkAliyun => run_provider_backend(
            py,
            asr_request,
            hk_aliyun_runner,
            "no Aliyun ASR host loaded for worker protocol V2",
        ),
        AsrBackendV2::HkFunaudio => run_provider_backend(
            py,
            asr_request,
            hk_funaudio_runner,
            "no FunAudio ASR host loaded for worker protocol V2",
        ),
        AsrBackendV2::Revai => Err(AsrExecuteFailure::ModelUnavailable(
            "Rev.AI is handled directly by the Rust control plane, not the Python worker"
                .to_owned(),
        )),
        // Control-flow invariant: the early `matches!` guard at the
        // top of this fn returns for `LocalWhisper`/`WhisperHub`
        // before the match runs, so reaching this arm is impossible.
        // The variants are listed explicitly (rather than using a
        // wildcard) to keep the match exhaustive against future
        // `AsrBackendV2` additions.
        #[allow(clippy::unreachable)]
        AsrBackendV2::LocalWhisper | AsrBackendV2::WhisperHub => unreachable!(),
    }
}

#[pyfunction]
#[pyo3(signature = (
    request,
    local_whisper_runner=None,
    hk_tencent_runner=None,
    hk_aliyun_runner=None,
    hk_funaudio_runner=None
))]
pub(crate) fn execute_asr_request_v2(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    local_whisper_runner: Option<Py<PyAny>>,
    hk_tencent_runner: Option<Py<PyAny>>,
    hk_aliyun_runner: Option<Py<PyAny>>,
    hk_funaudio_runner: Option<Py<PyAny>>,
) -> PyResult<String> {
    let request = parse_execute_request(request)?;
    let started_at = Instant::now();
    let response = match validate_attachment_descriptors(&request.attachments) {
        Err(message) => error_response(
            &request,
            ProtocolErrorCodeV2::InvalidPayload,
            message,
            started_at,
        ),
        Ok(()) => match run_asr(
            py,
            &request,
            local_whisper_runner,
            hk_tencent_runner,
            hk_aliyun_runner,
            hk_funaudio_runner,
        ) {
            Ok(result) => success_response(&request, result, started_at),
            Err(AsrExecuteFailure::Artifact(message)) => {
                error_response(&request, artifact_code(&message), message, started_at)
            }
            Err(AsrExecuteFailure::InvalidPayload(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::InvalidPayload,
                message,
                started_at,
            ),
            Err(AsrExecuteFailure::ModelUnavailable(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::ModelUnavailable,
                message,
                started_at,
            ),
            Err(AsrExecuteFailure::Runtime(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::RuntimeFailure,
                message,
                started_at,
            ),
        },
    };

    serde_json::to_string(&response)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())
}
