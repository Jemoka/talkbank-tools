//! Rust-owned worker-protocol V2 forced-alignment executor control plane.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "3. Forced Alignment V2" for:
//! - Python caller: `python/batchalign/worker/_fa_v2.py::execute_forced_alignment_request_v2()`
//! - Full Rust/Python responsibility split and input/output contracts.

use std::time::Instant;

use batchalign_types::api::{DurationMs, DurationSeconds};
use batchalign_types::worker_v2::{
    ExecuteOutcomeV2, ExecuteRequestV2, ExecuteResponseV2, FaBackendV2, FaTextModeV2,
    ForcedAlignmentRequestV2, IndexedWordTimingResultV2, IndexedWordTimingV2, InferenceTaskV2,
    ProtocolErrorCodeV2, TaskRequestV2, TaskResultV2, WhisperTokenTimingResultV2,
    WhisperTokenTimingV2,
};
use numpy::IntoPyArray;
use pyo3::prelude::*;

use crate::error::BatchalignBoundaryError;

use crate::py_json_bridge::py_to_json_value;
use crate::worker_artifacts::{
    load_prepared_audio_bytes_impl, load_prepared_text_json_impl,
    require_prepared_audio_attachment, require_prepared_text_attachment,
    validate_attachment_descriptors,
};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PreparedFaPayloadV2 {
    words: Vec<String>,
    word_ids: Vec<String>,
    word_utterance_indices: Vec<i64>,
    word_utterance_word_indices: Vec<i64>,
}

enum FaExecuteFailure {
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

fn extract_fa_request<'a>(
    request: &'a ExecuteRequestV2,
) -> Result<&'a ForcedAlignmentRequestV2, FaExecuteFailure> {
    if request.task != InferenceTaskV2::ForcedAlignment {
        return Err(FaExecuteFailure::InvalidPayload(format!(
            "expected forced_alignment task, got {:?}",
            request.task
        )));
    }
    match &request.payload {
        TaskRequestV2::ForcedAlignment(value) => Ok(value),
        _ => Err(FaExecuteFailure::InvalidPayload(
            "execute payload did not contain forced-alignment request data".to_owned(),
        )),
    }
}

fn load_fa_payload(
    request: &ExecuteRequestV2,
    fa_request: &ForcedAlignmentRequestV2,
) -> Result<PreparedFaPayloadV2, FaExecuteFailure> {
    let attachment =
        require_prepared_text_attachment(&request.attachments, fa_request.payload_ref_id.as_ref())
            .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?;
    let raw = load_prepared_text_json_impl(attachment)
        .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?;
    serde_json::from_str(&raw).map_err(|error| FaExecuteFailure::InvalidPayload(error.to_string()))
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

fn join_fa_words(words: &[String], text_mode: FaTextModeV2) -> String {
    match text_mode {
        FaTextModeV2::CharJoined => words.join("").replace('_', " ").trim().to_owned(),
        FaTextModeV2::SpaceJoined => words.join(" ").replace('_', " ").trim().to_owned(),
    }
}

fn parse_whisper_tokens(
    response: &Bound<'_, PyAny>,
) -> Result<WhisperTokenTimingResultV2, FaExecuteFailure> {
    let tokens: Vec<(String, f64)> =
        serde_json::from_value(py_to_json_value(response).map_err(|error| {
            FaExecuteFailure::Runtime(format!("invalid forced-alignment host output: {error}"))
        })?)
        .map_err(|error| {
            FaExecuteFailure::Runtime(format!("invalid forced-alignment host output: {error}"))
        })?;

    let mut normalized = Vec::with_capacity(tokens.len());
    for (text, time_s) in tokens {
        if time_s < 0.0 {
            return Err(FaExecuteFailure::Runtime(
                "invalid forced-alignment host output: Whisper token time_s must be >= 0"
                    .to_owned(),
            ));
        }
        normalized.push(WhisperTokenTimingV2 {
            text,
            time_s: DurationSeconds(time_s),
        });
    }
    Ok(WhisperTokenTimingResultV2 { tokens: normalized })
}

fn parse_indexed_timings(
    response: &Bound<'_, PyAny>,
    expected_words: usize,
) -> Result<IndexedWordTimingResultV2, FaExecuteFailure> {
    let spans: Vec<(String, (u64, u64))> =
        serde_json::from_value(py_to_json_value(response).map_err(|error| {
            FaExecuteFailure::Runtime(format!("invalid forced-alignment host output: {error}"))
        })?)
        .map_err(|error| {
            FaExecuteFailure::Runtime(format!("invalid forced-alignment host output: {error}"))
        })?;

    let mut indexed_timings = vec![None; expected_words];
    for (index, (_, (start_ms, end_ms))) in spans.into_iter().take(expected_words).enumerate() {
        if end_ms < start_ms {
            return Err(FaExecuteFailure::Runtime(
                "invalid forced-alignment host output: Indexed word timing end_ms must be >= start_ms"
                    .to_owned(),
            ));
        }
        indexed_timings[index] = Some(IndexedWordTimingV2 {
            start_ms: DurationMs(start_ms),
            end_ms: DurationMs(end_ms),
            confidence: None,
        });
    }
    Ok(IndexedWordTimingResultV2 { indexed_timings })
}

fn run_whisper(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    fa_request: &ForcedAlignmentRequestV2,
    whisper_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, FaExecuteFailure> {
    let payload = load_fa_payload(request, fa_request)?;
    let attachment =
        require_prepared_audio_attachment(&request.attachments, fa_request.audio_ref_id.as_ref())
            .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?;
    if attachment.channels.0 != 1 {
        return Err(FaExecuteFailure::InvalidPayload(
            "forced-alignment V2 currently expects mono prepared audio".to_owned(),
        ));
    }
    let runner = whisper_runner.ok_or_else(|| {
        FaExecuteFailure::ModelUnavailable(
            "no whisper FA host loaded for worker protocol V2".to_owned(),
        )
    })?;
    let audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(attachment)
            .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?,
    );
    let audio_array = audio.into_pyarray(py);
    let text = join_fa_words(&payload.words, fa_request.text_mode);
    let response = runner
        .bind(py)
        .call1((audio_array, text.as_str(), fa_request.pauses))
        .map_err(|error| FaExecuteFailure::Runtime(error.to_string()))?;
    Ok(TaskResultV2::WhisperTokenTimingResult(
        parse_whisper_tokens(&response)?,
    ))
}

fn run_wave2vec_like(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    fa_request: &ForcedAlignmentRequestV2,
    runner: Option<Py<PyAny>>,
    unavailable_message: &'static str,
    canto_mode: bool,
) -> Result<TaskResultV2, FaExecuteFailure> {
    let payload = load_fa_payload(request, fa_request)?;
    let attachment =
        require_prepared_audio_attachment(&request.attachments, fa_request.audio_ref_id.as_ref())
            .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?;
    if attachment.channels.0 != 1 {
        return Err(FaExecuteFailure::InvalidPayload(
            "forced-alignment V2 currently expects mono prepared audio".to_owned(),
        ));
    }
    let runner =
        runner.ok_or_else(|| FaExecuteFailure::ModelUnavailable(unavailable_message.to_owned()))?;
    let audio = decode_f32le_audio(
        load_prepared_audio_bytes_impl(attachment)
            .map_err(|error| FaExecuteFailure::Artifact(error.to_string()))?,
    );
    let audio_array = audio.into_pyarray(py);
    let response = if canto_mode {
        let payload_json = serde_json::to_string(&payload)
            .map_err(|error| FaExecuteFailure::Runtime(error.to_string()))?;
        let request_json = serde_json::to_string(fa_request)
            .map_err(|error| FaExecuteFailure::Runtime(error.to_string()))?;
        runner
            .bind(py)
            .call1((audio_array, payload_json.as_str(), request_json.as_str()))
            .map_err(|error| FaExecuteFailure::Runtime(error.to_string()))?
    } else {
        runner
            .bind(py)
            .call1((audio_array, payload.words.clone()))
            .map_err(|error| FaExecuteFailure::Runtime(error.to_string()))?
    };
    Ok(TaskResultV2::IndexedWordTimingResult(
        parse_indexed_timings(&response, payload.words.len())?,
    ))
}

fn run_fa(
    py: Python<'_>,
    request: &ExecuteRequestV2,
    whisper_runner: Option<Py<PyAny>>,
    wave2vec_runner: Option<Py<PyAny>>,
    canto_runner: Option<Py<PyAny>>,
) -> Result<TaskResultV2, FaExecuteFailure> {
    let fa_request = extract_fa_request(request)?;
    match fa_request.backend {
        FaBackendV2::Whisper => run_whisper(py, request, fa_request, whisper_runner),
        FaBackendV2::Wave2vec => run_wave2vec_like(
            py,
            request,
            fa_request,
            wave2vec_runner,
            "no wave2vec FA host loaded for worker protocol V2",
            false,
        ),
        FaBackendV2::Wav2vecCanto => run_wave2vec_like(
            py,
            request,
            fa_request,
            canto_runner,
            "no Cantonese FA host loaded for worker protocol V2",
            true,
        ),
    }
}

#[pyfunction]
#[pyo3(signature = (request, whisper_runner=None, wave2vec_runner=None, canto_runner=None))]
pub(crate) fn execute_forced_alignment_request_v2(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    whisper_runner: Option<Py<PyAny>>,
    wave2vec_runner: Option<Py<PyAny>>,
    canto_runner: Option<Py<PyAny>>,
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
        Ok(()) => match run_fa(py, &request, whisper_runner, wave2vec_runner, canto_runner) {
            Ok(result) => success_response(&request, result, started_at),
            Err(FaExecuteFailure::Artifact(message)) => {
                error_response(&request, artifact_code(&message), message, started_at)
            }
            Err(FaExecuteFailure::InvalidPayload(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::InvalidPayload,
                message,
                started_at,
            ),
            Err(FaExecuteFailure::ModelUnavailable(message)) => error_response(
                &request,
                ProtocolErrorCodeV2::ModelUnavailable,
                message,
                started_at,
            ),
            Err(FaExecuteFailure::Runtime(message)) => error_response(
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
