//! Rust-owned worker-protocol V2 prepared-audio executor control plane.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) for:
//! - Section "4. Media Analysis V2: OpenSMILE" → `python/batchalign/worker/_opensmile_v2.py`
//! - Section "5. Media Analysis V2: AVQI" → `python/batchalign/worker/_avqi_v2.py`
//! - Section "6. Media Analysis V2: Speaker Diarization" → `python/batchalign/worker/_speaker_v2.py`

use std::time::Instant;

use crate::error::BatchalignBoundaryError;
use batchalign_types::api::DurationSeconds;
use batchalign_types::worker_v2::{
    AvqiRequestV2, AvqiResultV2, ExecuteOutcomeV2, ExecuteRequestV2, ExecuteResponseV2,
    InferenceTaskV2, OpenSmileRequestV2, OpenSmileResultV2, ProtocolErrorCodeV2, SpeakerBackendV2,
    SpeakerInputV2, SpeakerRequestV2, SpeakerResultV2, TaskRequestV2, TaskResultV2,
};
use numpy::IntoPyArray;
use pyo3::prelude::*;

use crate::py_json_bridge::py_to_json_value;
use crate::worker_artifacts::{
    load_prepared_audio_bytes_impl, require_prepared_audio_attachment,
    validate_attachment_descriptors,
};

enum MediaExecuteFailure {
    Artifact(String),
    InvalidPayload(String),
    ModelUnavailable(String),
    Runtime(String),
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

fn extract_opensmile_request<'a>(
    request: &'a ExecuteRequestV2,
) -> Result<&'a OpenSmileRequestV2, MediaExecuteFailure> {
    if request.task != InferenceTaskV2::Opensmile {
        return Err(MediaExecuteFailure::InvalidPayload(format!(
            "expected opensmile task, got {:?}",
            request.task
        )));
    }
    match &request.payload {
        TaskRequestV2::Opensmile(value) => Ok(value),
        _ => Err(MediaExecuteFailure::InvalidPayload(
            "execute payload did not contain openSMILE request data".to_owned(),
        )),
    }
}

fn extract_avqi_request<'a>(
    request: &'a ExecuteRequestV2,
) -> Result<&'a AvqiRequestV2, MediaExecuteFailure> {
    if request.task != InferenceTaskV2::Avqi {
        return Err(MediaExecuteFailure::InvalidPayload(format!(
            "expected avqi task, got {:?}",
            request.task
        )));
    }
    match &request.payload {
        TaskRequestV2::Avqi(value) => Ok(value),
        _ => Err(MediaExecuteFailure::InvalidPayload(
            "execute payload did not contain AVQI request data".to_owned(),
        )),
    }
}

fn extract_speaker_request<'a>(
    request: &'a ExecuteRequestV2,
) -> Result<&'a SpeakerRequestV2, MediaExecuteFailure> {
    if request.task != InferenceTaskV2::Speaker {
        return Err(MediaExecuteFailure::InvalidPayload(format!(
            "expected speaker task, got {:?}",
            request.task
        )));
    }
    match &request.payload {
        TaskRequestV2::Speaker(value) => Ok(value),
        _ => Err(MediaExecuteFailure::InvalidPayload(
            "execute payload did not contain speaker request data".to_owned(),
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

fn parse_opensmile_result(
    response: &Bound<'_, PyAny>,
) -> Result<OpenSmileResultV2, MediaExecuteFailure> {
    serde_json::from_value(py_to_json_value(response).map_err(|error| {
        MediaExecuteFailure::Runtime(format!("invalid openSMILE host output: {error}"))
    })?)
    .map_err(|error| {
        MediaExecuteFailure::Runtime(format!("invalid openSMILE host output: {error}"))
    })
}

fn parse_avqi_result(response: &Bound<'_, PyAny>) -> Result<AvqiResultV2, MediaExecuteFailure> {
    serde_json::from_value(py_to_json_value(response).map_err(|error| {
        MediaExecuteFailure::Runtime(format!("invalid AVQI host output: {error}"))
    })?)
    .map_err(|error| MediaExecuteFailure::Runtime(format!("invalid AVQI host output: {error}")))
}

fn parse_speaker_result(
    response: &Bound<'_, PyAny>,
) -> Result<SpeakerResultV2, MediaExecuteFailure> {
    let parsed: SpeakerResultV2 =
        serde_json::from_value(py_to_json_value(response).map_err(|error| {
            MediaExecuteFailure::Runtime(format!("invalid speaker host output: {error}"))
        })?)
        .map_err(|error| {
            MediaExecuteFailure::Runtime(format!("invalid speaker host output: {error}"))
        })?;
    if parsed
        .segments
        .iter()
        .any(|segment| segment.end_ms < segment.start_ms)
    {
        return Err(MediaExecuteFailure::Runtime(
            "invalid speaker host output: Speaker segment end_ms must be >= start_ms".to_owned(),
        ));
    }
    Ok(parsed)
}

fn run_opensmile(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    prepared_audio_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, MediaExecuteFailure> {
    let opensmile_request = extract_opensmile_request(request)?;
    let attachment = require_prepared_audio_attachment(
        &request.attachments,
        opensmile_request.audio_ref_id.as_ref(),
    )
    .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?;
    if attachment.channels.0 != 1 {
        return Err(MediaExecuteFailure::InvalidPayload(
            "openSMILE V2 currently expects mono prepared audio".to_owned(),
        ));
    }
    let runner = prepared_audio_runner.ok_or_else(|| {
        MediaExecuteFailure::ModelUnavailable(
            "no openSMILE host loaded for worker protocol V2".to_owned(),
        )
    })?;
    let audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(attachment)
            .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?,
    );
    let audio_array = audio.into_pyarray(py);
    let response = runner
        .bind(py)
        .call1((
            audio_array,
            attachment.sample_rate_hz.0,
            opensmile_request.feature_set.as_str(),
            opensmile_request.feature_level.as_str(),
            attachment.path.as_ref(),
        ))
        .map_err(|error| MediaExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::OpensmileResult(parse_opensmile_result(
        &response,
    )?))
}

fn run_avqi(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    prepared_audio_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, MediaExecuteFailure> {
    let avqi_request = extract_avqi_request(request)?;
    let cs_attachment = require_prepared_audio_attachment(
        &request.attachments,
        avqi_request.cs_audio_ref_id.as_ref(),
    )
    .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?;
    let sv_attachment = require_prepared_audio_attachment(
        &request.attachments,
        avqi_request.sv_audio_ref_id.as_ref(),
    )
    .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?;
    if cs_attachment.channels.0 != 1 || sv_attachment.channels.0 != 1 {
        return Err(MediaExecuteFailure::InvalidPayload(
            "AVQI V2 currently expects mono prepared audio".to_owned(),
        ));
    }
    let runner = prepared_audio_runner.ok_or_else(|| {
        MediaExecuteFailure::ModelUnavailable(
            "no AVQI host loaded for worker protocol V2".to_owned(),
        )
    })?;
    let cs_audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(cs_attachment)
            .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?,
    );
    let sv_audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(sv_attachment)
            .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?,
    );
    let cs_audio_array = cs_audio.into_pyarray(py);
    let sv_audio_array = sv_audio.into_pyarray(py);
    let response = runner
        .bind(py)
        .call1((
            cs_audio_array,
            cs_attachment.sample_rate_hz.0,
            sv_audio_array,
            sv_attachment.sample_rate_hz.0,
            cs_attachment.path.as_ref(),
            sv_attachment.path.as_ref(),
        ))
        .map_err(|error| MediaExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::AvqiResult(parse_avqi_result(&response)?))
}

fn run_speaker(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    pyannote_prepared_audio_runner: Option<Py<PyAny>>,
    nemo_prepared_audio_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, MediaExecuteFailure> {
    let speaker_request = extract_speaker_request(request)?;
    let audio_ref_id = match &speaker_request.input {
        SpeakerInputV2::PreparedAudio(value) => value.audio_ref_id.as_ref(),
    };
    let attachment = require_prepared_audio_attachment(&request.attachments, audio_ref_id)
        .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?;
    if attachment.channels.0 != 1 {
        return Err(MediaExecuteFailure::InvalidPayload(
            "worker protocol V2 speaker currently expects mono prepared audio".to_owned(),
        ));
    }
    let expected_speakers = speaker_request
        .expected_speakers
        .map(|value| value.0)
        .unwrap_or(2);
    let runner = match speaker_request.backend {
        SpeakerBackendV2::Pyannote => pyannote_prepared_audio_runner.ok_or_else(|| {
            MediaExecuteFailure::ModelUnavailable(
                "no pyannote speaker host loaded for prepared-audio V2".to_owned(),
            )
        })?,
        SpeakerBackendV2::Nemo => nemo_prepared_audio_runner.ok_or_else(|| {
            MediaExecuteFailure::ModelUnavailable(
                "no NeMo speaker host loaded for prepared-audio V2".to_owned(),
            )
        })?,
    };
    let audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(attachment)
            .map_err(|error| MediaExecuteFailure::Artifact(error.to_string()))?,
    );
    let audio_array = audio.into_pyarray(py);
    let response = runner
        .bind(py)
        .call1((audio_array, attachment.sample_rate_hz.0, expected_speakers))
        .map_err(|error| MediaExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::SpeakerResult(parse_speaker_result(
        &response,
    )?))
}

#[pyfunction]
#[pyo3(signature = (request, prepared_audio_runner=None))]
pub(crate) fn execute_opensmile_request_v2(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    prepared_audio_runner: Option<Py<PyAny>>,
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
        Ok(()) => match run_opensmile(py, &request, prepared_audio_runner) {
            Ok(result) => success_response(&request, result, started_at),
            Err(MediaExecuteFailure::Artifact(message)) => {
                error_response(&request, artifact_code(&message), message, started_at)
            }
            Err(MediaExecuteFailure::InvalidPayload(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::InvalidPayload,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::ModelUnavailable(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::ModelUnavailable,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::Runtime(message)) => error_response(
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

#[pyfunction]
#[pyo3(signature = (request, prepared_audio_runner=None))]
pub(crate) fn execute_avqi_request_v2(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    prepared_audio_runner: Option<Py<PyAny>>,
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
        Ok(()) => match run_avqi(py, &request, prepared_audio_runner) {
            Ok(result) => success_response(&request, result, started_at),
            Err(MediaExecuteFailure::Artifact(message)) => {
                error_response(&request, artifact_code(&message), message, started_at)
            }
            Err(MediaExecuteFailure::InvalidPayload(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::InvalidPayload,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::ModelUnavailable(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::ModelUnavailable,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::Runtime(message)) => error_response(
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

#[pyfunction]
#[pyo3(signature = (request, pyannote_prepared_audio_runner=None, nemo_prepared_audio_runner=None))]
pub(crate) fn execute_speaker_request_v2(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    pyannote_prepared_audio_runner: Option<Py<PyAny>>,
    nemo_prepared_audio_runner: Option<Py<PyAny>>,
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
        Ok(()) => match run_speaker(
            py,
            &request,
            pyannote_prepared_audio_runner,
            nemo_prepared_audio_runner,
        ) {
            Ok(result) => success_response(&request, result, started_at),
            Err(MediaExecuteFailure::Artifact(message)) => {
                error_response(&request, artifact_code(&message), message, started_at)
            }
            Err(MediaExecuteFailure::InvalidPayload(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::InvalidPayload,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::ModelUnavailable(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::ModelUnavailable,
                message,
                started_at,
            ),
            Err(MediaExecuteFailure::Runtime(message)) => error_response(
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
