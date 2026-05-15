// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Matrix tests for worker-protocol V2 fixture invariants and live boundary behavior.
//!
//! These tests complement the roundtrip drift suites by asserting properties
//! that serde/Pydantic alone cannot encode:
//!
//! - canonical request fixtures cover the current live backend matrix
//! - task/payload/backend/input invariants hold for valid fixtures
//! - intentionally invalid combinations are rejected before they reach models
//! - a real Rust->Python stdio worker roundtrip returns typed protocol errors
//!   for mismatched execute requests

mod common;

use batchalign::api::{LanguageCode3, WorkerLanguage};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use batchalign::api::{DurationSeconds, NumSpeakers};
use batchalign::types::worker_v2::{
    ArtifactRefV2, AsrBackendV2, AsrInputV2, AsrRequestV2, ExecuteOutcomeV2, ExecuteRequestV2,
    ExecuteResponseV2, FaBackendV2, FaTextModeV2, ForcedAlignmentRequestV2, InferenceTaskV2,
    ProtocolErrorCodeV2, ProviderMediaInputV2, SpeakerBackendV2, TaskRequestV2, TaskResultV2,
    WorkerArtifactIdV2,
};
use batchalign::worker::WorkerProfile;
use batchalign::worker::handle::WorkerConfig;
use common::resolve_python;
use serde::Deserialize;
use serde_json::Value;

/// One fixture manifest entry shared by the Rust and Python V2 drift tests.
#[derive(Debug, Clone, Deserialize)]
struct FixtureEntry {
    /// Logical schema name used to select the parser.
    schema: String,
    /// Fixture filename relative to the worker-protocol fixture root.
    file: String,
}

/// Top-level manifest for the shared worker-protocol V2 fixtures.
#[derive(Debug, Deserialize)]
struct FixtureManifest {
    /// Canonical fixture entries consumed by both test suites.
    fixtures: Vec<FixtureEntry>,
}

/// Return the shared repo-level fixture directory for worker protocol V2.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/worker_protocol_v2")
}

/// Load and deserialize the shared fixture manifest.
fn load_manifest() -> FixtureManifest {
    let path = fixture_root().join("manifest.json");
    let raw = fs::read_to_string(path).expect("worker protocol v2 manifest should exist");
    serde_json::from_str(&raw).expect("worker protocol v2 manifest should parse")
}

/// Load one execute-request fixture through the Rust schema.
fn load_execute_request(file: &str) -> ExecuteRequestV2 {
    let path = fixture_root().join(file);
    let raw = fs::read_to_string(path).expect("worker protocol v2 request fixture should exist");
    serde_json::from_str(&raw).expect("worker protocol v2 request fixture should parse")
}

/// Load one execute-response fixture through the Rust schema.
fn load_execute_response(file: &str) -> ExecuteResponseV2 {
    let path = fixture_root().join(file);
    let raw = fs::read_to_string(path).expect("worker protocol v2 response fixture should exist");
    serde_json::from_str(&raw).expect("worker protocol v2 response fixture should parse")
}

/// Return the canonical execute-request fixture entries from the shared manifest.
fn execute_request_entries() -> Vec<FixtureEntry> {
    load_manifest()
        .fixtures
        .into_iter()
        .filter(|entry| entry.schema == "execute_request")
        .collect()
}

/// Return the canonical execute-response fixture entries from the shared manifest.
fn execute_response_entries() -> Vec<FixtureEntry> {
    load_manifest()
        .fixtures
        .into_iter()
        .filter(|entry| entry.schema == "execute_response")
        .collect()
}

/// Return the payload-kind label for one typed request payload.
fn payload_kind_name(payload: &TaskRequestV2) -> &'static str {
    match payload {
        TaskRequestV2::Asr(_) => "asr",
        TaskRequestV2::ForcedAlignment(_) => "forced_alignment",
        TaskRequestV2::Morphosyntax(_) => "morphosyntax",
        TaskRequestV2::Utseg(_) => "utseg",
        TaskRequestV2::Translate(_) => "translate",
        TaskRequestV2::Coref(_) => "coref",
        TaskRequestV2::Speaker(_) => "speaker",
        TaskRequestV2::Opensmile(_) => "opensmile",
        TaskRequestV2::Avqi(_) => "avqi",
    }
}

/// Return the expected payload-kind label for one top-level V2 task.
fn expected_payload_kind(task: InferenceTaskV2) -> &'static str {
    match task {
        InferenceTaskV2::Asr => "asr",
        InferenceTaskV2::ForcedAlignment => "forced_alignment",
        InferenceTaskV2::Morphosyntax => "morphosyntax",
        InferenceTaskV2::Utseg => "utseg",
        InferenceTaskV2::Translate => "translate",
        InferenceTaskV2::Coref => "coref",
        InferenceTaskV2::Speaker => "speaker",
        InferenceTaskV2::Opensmile => "opensmile",
        InferenceTaskV2::Avqi => "avqi",
    }
}

/// Return the stable label for one ASR backend.
fn asr_backend_label(backend: AsrBackendV2) -> &'static str {
    match backend {
        AsrBackendV2::LocalWhisper => "local_whisper",
        AsrBackendV2::WhisperHub => "whisper_hub",
        AsrBackendV2::HkTencent => "hk_tencent",
        AsrBackendV2::HkAliyun => "hk_aliyun",
        AsrBackendV2::HkFunaudio => "hk_funaudio",
        AsrBackendV2::Revai => "revai",
    }
}

/// Return the stable label for one FA backend.
fn fa_backend_label(backend: FaBackendV2) -> &'static str {
    match backend {
        FaBackendV2::Whisper => "whisper",
        FaBackendV2::Wave2vec => "wave2vec",
        FaBackendV2::Wav2vecCanto => "wav2vec_canto",
    }
}

/// Return the stable label for one speaker backend.
fn speaker_backend_label(backend: SpeakerBackendV2) -> &'static str {
    match backend {
        SpeakerBackendV2::Pyannote => "pyannote",
        SpeakerBackendV2::Nemo => "nemo",
    }
}

/// Return the stable label for one ASR input transport.
fn asr_input_kind_name(input: &AsrInputV2) -> &'static str {
    match input {
        AsrInputV2::PreparedAudio(_) => "prepared_audio",
        AsrInputV2::ProviderMedia(_) => "provider_media",
        AsrInputV2::SubmittedJob(_) => "submitted_job",
    }
}

/// Return the stable label for one execute result kind.
fn result_kind_name(result: &TaskResultV2) -> &'static str {
    match result {
        TaskResultV2::WhisperChunkResult(_) => "whisper_chunk_result",
        TaskResultV2::MonologueAsrResult(_) => "monologue_asr_result",
        TaskResultV2::WhisperTokenTimingResult(_) => "whisper_token_timing_result",
        TaskResultV2::IndexedWordTimingResult(_) => "indexed_word_timing_result",
        TaskResultV2::MorphosyntaxResult(_) => "morphosyntax_result",
        TaskResultV2::UtsegResult(_) => "utseg_result",
        TaskResultV2::TranslationResult(_) => "translation_result",
        TaskResultV2::CorefResult(_) => "coref_result",
        TaskResultV2::SpeakerResult(_) => "speaker_result",
        TaskResultV2::OpensmileResult(_) => "opensmile_result",
        TaskResultV2::AvqiResult(_) => "avqi_result",
    }
}

/// Return the expected result kind for one canonical execute-response fixture.
fn expected_result_kind(file: &str) -> Option<&'static str> {
    match file {
        "execute_response_asr_success.json" => Some("whisper_chunk_result"),
        "execute_response_asr_monologues.json" => Some("monologue_asr_result"),
        "execute_response_fa_whisper_tokens.json" => Some("whisper_token_timing_result"),
        "execute_response_fa_indexed_timings.json" => Some("indexed_word_timing_result"),
        "execute_response_morphosyntax_success.json" => Some("morphosyntax_result"),
        "execute_response_utseg_success.json" => Some("utseg_result"),
        "execute_response_translate_success.json" => Some("translation_result"),
        "execute_response_coref_success.json" => Some("coref_result"),
        "execute_response_protocol_error.json" => None,
        "execute_response_speaker_segments.json" => Some("speaker_result"),
        "execute_response_opensmile_success.json" => Some("opensmile_result"),
        "execute_response_avqi_success.json" => Some("avqi_result"),
        other => panic!("missing expected result-kind mapping for {other}"),
    }
}

/// Return the stable id of one attachment descriptor.
fn attachment_id(attachment: &ArtifactRefV2) -> &WorkerArtifactIdV2 {
    match attachment {
        ArtifactRefV2::PreparedAudio(value) => &value.id,
        ArtifactRefV2::PreparedText(value) => &value.id,
        ArtifactRefV2::InlineJson(value) => &value.id,
    }
}

/// Return the stable kind label of one attachment descriptor.
fn attachment_kind_name(attachment: &ArtifactRefV2) -> &'static str {
    match attachment {
        ArtifactRefV2::PreparedAudio(_) => "prepared_audio",
        ArtifactRefV2::PreparedText(_) => "prepared_text",
        ArtifactRefV2::InlineJson(_) => "inline_json",
    }
}

/// Return the attachment with the requested id if present.
fn attachment_by_id<'a>(
    attachments: &'a [ArtifactRefV2],
    artifact_id: &WorkerArtifactIdV2,
) -> Option<&'a ArtifactRefV2> {
    attachments
        .iter()
        .find(|attachment| attachment_id(attachment) == artifact_id)
}

/// Require that one request attachment exists and has the expected kind.
fn require_attachment_kind(
    request: &ExecuteRequestV2,
    artifact_id: &WorkerArtifactIdV2,
    expected_kind: &'static str,
) -> Result<(), String> {
    let Some(attachment) = attachment_by_id(&request.attachments, artifact_id) else {
        return Err(format!("missing attachment {artifact_id}"));
    };
    let actual_kind = attachment_kind_name(attachment);
    if actual_kind != expected_kind {
        return Err(format!(
            "attachment {artifact_id} had kind {actual_kind}, expected {expected_kind}"
        ));
    }
    Ok(())
}

/// Require that one request attachment exists and carries JSON data.
fn require_json_attachment<'a>(
    request: &'a ExecuteRequestV2,
    artifact_id: &WorkerArtifactIdV2,
) -> Result<Option<&'a Value>, String> {
    let Some(attachment) = attachment_by_id(&request.attachments, artifact_id) else {
        return Err(format!("missing attachment {artifact_id}"));
    };
    match attachment {
        ArtifactRefV2::PreparedText(_) => Ok(None),
        ArtifactRefV2::InlineJson(value) => Ok(Some(&value.value)),
        ArtifactRefV2::PreparedAudio(_) => Err(format!(
            "attachment {artifact_id} had kind prepared_audio, expected prepared_text or inline_json"
        )),
    }
}

/// Require that one inline JSON field exists and is an array.
fn require_array_field<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a Vec<Value>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("forced-alignment inline JSON missing array field {field}"))
}

/// Require that all values in one JSON array are strings.
fn require_string_array(values: &[Value], field: &'static str) -> Result<(), String> {
    if values.iter().all(Value::is_string) {
        Ok(())
    } else {
        Err(format!(
            "forced-alignment inline JSON field {field} must be a string array"
        ))
    }
}

/// Require that all values in one JSON array are unsigned integers.
fn require_u64_array(values: &[Value], field: &'static str) -> Result<(), String> {
    if values.iter().all(|value| value.as_u64().is_some()) {
        Ok(())
    } else {
        Err(format!(
            "forced-alignment inline JSON field {field} must be a u64 array"
        ))
    }
}

/// Validate the FA inline JSON fallback shape used by canonical fixtures.
fn validate_forced_alignment_inline_json(value: &Value) -> Result<(), String> {
    let words = require_array_field(value, "words")?;
    require_string_array(words, "words")?;
    let expected_len = words.len();

    let word_ids = require_array_field(value, "word_ids")?;
    require_string_array(word_ids, "word_ids")?;
    if word_ids.len() != expected_len {
        return Err(format!(
            "forced-alignment inline JSON field word_ids had length {}, expected {expected_len}",
            word_ids.len()
        ));
    }

    let utterance_indices = require_array_field(value, "word_utterance_indices")?;
    require_u64_array(utterance_indices, "word_utterance_indices")?;
    if utterance_indices.len() != expected_len {
        return Err(format!(
            "forced-alignment inline JSON field word_utterance_indices had length {}, expected {expected_len}",
            utterance_indices.len()
        ));
    }

    let word_indices = require_array_field(value, "word_utterance_word_indices")?;
    require_u64_array(word_indices, "word_utterance_word_indices")?;
    if word_indices.len() != expected_len {
        return Err(format!(
            "forced-alignment inline JSON field word_utterance_word_indices had length {}, expected {expected_len}",
            word_indices.len()
        ));
    }

    Ok(())
}

/// Validate live-boundary invariants for one typed execute request.
fn validate_execute_request_invariants(request: &ExecuteRequestV2) -> Result<(), String> {
    let mut attachment_ids = BTreeSet::new();
    for attachment in &request.attachments {
        let inserted = attachment_ids.insert(attachment_id(attachment).to_string());
        if !inserted {
            return Err(format!(
                "duplicate attachment id {} in request {}",
                attachment_id(attachment),
                request.request_id
            ));
        }
    }

    if payload_kind_name(&request.payload) != expected_payload_kind(request.task) {
        return Err(format!(
            "task {} expected payload kind {}, got {}",
            expected_payload_kind(request.task),
            expected_payload_kind(request.task),
            payload_kind_name(&request.payload)
        ));
    }

    match &request.payload {
        TaskRequestV2::Asr(asr_request) => validate_asr_request(request, asr_request),
        TaskRequestV2::ForcedAlignment(fa_request) => {
            validate_forced_alignment_request(request, fa_request)
        }
        TaskRequestV2::Morphosyntax(morphosyntax_request) => {
            require_json_attachment(request, &morphosyntax_request.payload_ref_id)?;
            Ok(())
        }
        TaskRequestV2::Utseg(utseg_request) => {
            require_json_attachment(request, &utseg_request.payload_ref_id)?;
            Ok(())
        }
        TaskRequestV2::Translate(translate_request) => {
            require_json_attachment(request, &translate_request.payload_ref_id)?;
            Ok(())
        }
        TaskRequestV2::Coref(coref_request) => {
            require_json_attachment(request, &coref_request.payload_ref_id)?;
            Ok(())
        }
        TaskRequestV2::Speaker(speaker_request) => {
            let audio_ref_id = match &speaker_request.input {
                batchalign::types::worker_v2::SpeakerInputV2::PreparedAudio(value) => {
                    &value.audio_ref_id
                }
            };
            require_attachment_kind(request, audio_ref_id, "prepared_audio")?;
            match speaker_request.backend {
                SpeakerBackendV2::Pyannote | SpeakerBackendV2::Nemo => Ok(()),
            }
        }
        TaskRequestV2::Opensmile(opensmile_request) => {
            require_attachment_kind(request, &opensmile_request.audio_ref_id, "prepared_audio")
        }
        TaskRequestV2::Avqi(avqi_request) => {
            require_attachment_kind(request, &avqi_request.cs_audio_ref_id, "prepared_audio")?;
            require_attachment_kind(request, &avqi_request.sv_audio_ref_id, "prepared_audio")?;
            if avqi_request.cs_audio_ref_id == avqi_request.sv_audio_ref_id {
                return Err("avqi request reused the same audio ref for cs and sv payloads".into());
            }
            Ok(())
        }
    }
}

/// Validate live-boundary invariants for one typed ASR request.
fn validate_asr_request(
    request: &ExecuteRequestV2,
    asr_request: &AsrRequestV2,
) -> Result<(), String> {
    match (&asr_request.backend, &asr_request.input) {
        (
            AsrBackendV2::LocalWhisper | AsrBackendV2::WhisperHub,
            AsrInputV2::PreparedAudio(input),
        ) => require_attachment_kind(request, &input.audio_ref_id, "prepared_audio"),
        (AsrBackendV2::LocalWhisper | AsrBackendV2::WhisperHub, other) => Err(format!(
            "ASR backend {} requires prepared_audio input, got {}",
            asr_backend_label(asr_request.backend),
            asr_input_kind_name(other)
        )),
        (AsrBackendV2::HkTencent, AsrInputV2::ProviderMedia(_))
        | (AsrBackendV2::HkAliyun, AsrInputV2::ProviderMedia(_))
        | (AsrBackendV2::HkFunaudio, AsrInputV2::ProviderMedia(_)) => Ok(()),
        (AsrBackendV2::HkTencent, other)
        | (AsrBackendV2::HkAliyun, other)
        | (AsrBackendV2::HkFunaudio, other) => Err(format!(
            "ASR backend {} requires provider_media input, got {}",
            asr_backend_label(asr_request.backend),
            asr_input_kind_name(other)
        )),
        (AsrBackendV2::Revai, _) => Err(
            "Rev.AI requests are handled by the Rust control plane, not the Python V2 worker"
                .into(),
        ),
    }
}

/// Validate live-boundary invariants for one typed forced-alignment request.
fn validate_forced_alignment_request(
    request: &ExecuteRequestV2,
    fa_request: &ForcedAlignmentRequestV2,
) -> Result<(), String> {
    if let Some(inline_value) = require_json_attachment(request, &fa_request.payload_ref_id)? {
        validate_forced_alignment_inline_json(inline_value)?;
    }
    require_attachment_kind(request, &fa_request.audio_ref_id, "prepared_audio")?;

    match (fa_request.backend, fa_request.text_mode) {
        (FaBackendV2::Whisper, FaTextModeV2::SpaceJoined)
        | (FaBackendV2::Wave2vec, FaTextModeV2::SpaceJoined)
        | (FaBackendV2::Wav2vecCanto, FaTextModeV2::CharJoined) => Ok(()),
        (FaBackendV2::Whisper, FaTextModeV2::CharJoined) => {
            Err("forced-alignment backend whisper requires space_joined text".into())
        }
        (FaBackendV2::Wave2vec, FaTextModeV2::CharJoined) => {
            Err("forced-alignment backend wave2vec requires space_joined text".into())
        }
        (FaBackendV2::Wav2vecCanto, FaTextModeV2::SpaceJoined) => {
            Err("forced-alignment backend wav2vec_canto requires char_joined text".into())
        }
    }
}

/// Validate live-boundary invariants for one typed execute response.
fn validate_execute_response_invariants(
    file: &str,
    response: &ExecuteResponseV2,
) -> Result<(), String> {
    if response.elapsed_s.0 < 0.0 {
        return Err(format!(
            "response fixture {file} had negative elapsed_s {}",
            response.elapsed_s.0
        ));
    }

    match (&response.outcome, &response.result) {
        (ExecuteOutcomeV2::Success, Some(result)) => {
            if response.elapsed_s.0 <= 0.0 {
                return Err(format!(
                    "success response fixture {file} must record elapsed_s > 0"
                ));
            }
            let expected_kind = expected_result_kind(file)
                .expect("successful response fixtures should declare an expected result kind");
            let actual_kind = result_kind_name(result);
            if actual_kind != expected_kind {
                return Err(format!(
                    "response fixture {file} had result kind {actual_kind}, expected {expected_kind}"
                ));
            }
            validate_task_result_shape(result)
        }
        (ExecuteOutcomeV2::Success, None) => Err(format!(
            "success response fixture {file} was missing a result"
        )),
        (ExecuteOutcomeV2::Error { message, .. }, None) => {
            if message.trim().is_empty() {
                return Err(format!(
                    "error response fixture {file} had an empty message"
                ));
            }
            if expected_result_kind(file).is_some() {
                return Err(format!(
                    "error response fixture {file} unexpectedly declared a success result kind"
                ));
            }
            Ok(())
        }
        (ExecuteOutcomeV2::Error { .. }, Some(_)) => Err(format!(
            "error response fixture {file} unexpectedly included a result"
        )),
    }
}

/// Validate result-shape invariants that the Rust schema alone cannot encode.
fn validate_task_result_shape(result: &TaskResultV2) -> Result<(), String> {
    match result {
        TaskResultV2::WhisperChunkResult(value) => {
            if value.text.trim().is_empty() {
                return Err("whisper chunk result text must not be empty".into());
            }
            if value.chunks.is_empty() {
                return Err("whisper chunk result must contain at least one chunk".into());
            }
            let mut previous_end = DurationSeconds(0.0);
            for (index, chunk) in value.chunks.iter().enumerate() {
                if chunk.end_s < chunk.start_s {
                    return Err(format!("whisper chunk {index} ended before it started"));
                }
                if index > 0 && chunk.start_s < previous_end {
                    return Err(format!(
                        "whisper chunk {index} started before the previous chunk"
                    ));
                }
                previous_end = chunk.end_s;
            }
            Ok(())
        }
        TaskResultV2::MonologueAsrResult(value) => {
            if value.monologues.is_empty() {
                return Err("monologue ASR result must contain at least one monologue".into());
            }
            if value
                .monologues
                .iter()
                .flat_map(|monologue| monologue.elements.iter())
                .any(|element| element.value.trim().is_empty())
            {
                return Err("monologue ASR result contained an empty element value".into());
            }
            Ok(())
        }
        TaskResultV2::WhisperTokenTimingResult(value) => {
            if value.tokens.is_empty() {
                return Err("whisper token timing result must contain at least one token".into());
            }
            let mut previous_time = DurationSeconds(0.0);
            for (index, token) in value.tokens.iter().enumerate() {
                if !token.time_s.0.is_finite() {
                    return Err(format!("whisper token {index} had a non-finite timestamp"));
                }
                if index > 0 && token.time_s < previous_time {
                    return Err(format!("whisper token {index} regressed in time"));
                }
                previous_time = token.time_s;
            }
            Ok(())
        }
        TaskResultV2::IndexedWordTimingResult(value) => {
            if value.indexed_timings.is_empty() {
                return Err("indexed timing result must contain at least one slot".into());
            }
            for (index, timing) in value.indexed_timings.iter().enumerate() {
                if let Some(timing) = timing
                    && timing.end_ms < timing.start_ms
                {
                    return Err(format!("indexed timing {index} ended before it started"));
                }
            }
            Ok(())
        }
        TaskResultV2::MorphosyntaxResult(value) => {
            if value.items.is_empty() {
                return Err("morphosyntax result must contain at least one item".into());
            }
            if value
                .items
                .iter()
                .all(|item| item.raw_sentences.is_none() && item.error.is_none())
            {
                return Err("morphosyntax result items must contain data or an error".into());
            }
            Ok(())
        }
        TaskResultV2::UtsegResult(value) => {
            if value.items.is_empty() {
                return Err("utseg result must contain at least one item".into());
            }
            if value
                .items
                .iter()
                .all(|item| item.trees.is_none() && item.error.is_none())
            {
                return Err("utseg result items must contain trees or an error".into());
            }
            Ok(())
        }
        TaskResultV2::TranslationResult(value) => {
            if value.items.is_empty() {
                return Err("translation result must contain at least one item".into());
            }
            if value
                .items
                .iter()
                .all(|item| item.raw_translation.is_none() && item.error.is_none())
            {
                return Err("translation result items must contain text or an error".into());
            }
            Ok(())
        }
        TaskResultV2::CorefResult(value) => {
            if value.items.is_empty() {
                return Err("coref result must contain at least one item".into());
            }
            if value
                .items
                .iter()
                .all(|item| item.annotations.is_none() && item.error.is_none())
            {
                return Err("coref result items must contain annotations or an error".into());
            }
            Ok(())
        }
        TaskResultV2::SpeakerResult(value) => {
            if value.segments.is_empty() {
                return Err("speaker result must contain at least one segment".into());
            }
            for (index, segment) in value.segments.iter().enumerate() {
                if segment.end_ms < segment.start_ms {
                    return Err(format!("speaker segment {index} ended before it started"));
                }
            }
            Ok(())
        }
        TaskResultV2::OpensmileResult(value) => {
            if value.success && value.rows.is_empty() {
                return Err("successful openSMILE result must contain at least one row".into());
            }
            if value.audio_file.trim().is_empty() {
                return Err("openSMILE result must echo a non-empty audio label".into());
            }
            Ok(())
        }
        TaskResultV2::AvqiResult(value) => {
            if value.cs_file.trim().is_empty() || value.sv_file.trim().is_empty() {
                return Err("avqi result must echo both audio labels".into());
            }
            if !value.avqi.is_finite()
                || !value.cpps.is_finite()
                || !value.hnr.is_finite()
                || !value.shimmer_local.is_finite()
                || !value.shimmer_local_db.is_finite()
                || !value.slope.is_finite()
                || !value.tilt.is_finite()
            {
                return Err("avqi result contained a non-finite metric".into());
            }
            Ok(())
        }
    }
}

/// Require a Python interpreter for real Rust->Python worker tests.
macro_rules! require_python {
    () => {{
        common::test_server_fixture::isolate_host_memory_ledger();
        let available_mb = batchalign::worker::memory_guard::available_memory_mb();
        if available_mb < 4096 {
            eprintln!("SKIP: insufficient memory ({available_mb} MB available, 4096 MB required)");
            return;
        }
        match resolve_python() {
            Some(path) => path,
            None => {
                eprintln!("SKIP: Python 3 with batchalign.worker not available");
                return;
            }
        }
    }};
}

/// Build one intentionally mismatched execute request that should fail before
/// any model host is required.
fn mismatched_execute_request(request_id: &str, task: InferenceTaskV2) -> ExecuteRequestV2 {
    ExecuteRequestV2 {
        request_id: request_id.into(),
        task,
        payload: TaskRequestV2::Asr(AsrRequestV2 {
            lang: WorkerLanguage::from(LanguageCode3::yue()),
            backend: AsrBackendV2::HkTencent,
            input: AsrInputV2::ProviderMedia(ProviderMediaInputV2 {
                media_path: "/tmp/mismatched-provider.wav".into(),
                num_speakers: NumSpeakers(2),
            }),
        }),
        attachments: vec![],
    }
}

#[test]
fn worker_protocol_v2_request_manifest_covers_live_backend_matrix() {
    let mut asr_backends = BTreeSet::new();
    let mut fa_backends = BTreeSet::new();
    let mut speaker_backends = BTreeSet::new();

    for entry in execute_request_entries() {
        let request = load_execute_request(&entry.file);
        validate_execute_request_invariants(&request).unwrap_or_else(|error| {
            panic!("request fixture {} failed invariants: {error}", entry.file)
        });
        match &request.payload {
            TaskRequestV2::Asr(value) => {
                asr_backends.insert(asr_backend_label(value.backend).to_string());
            }
            TaskRequestV2::ForcedAlignment(value) => {
                fa_backends.insert(fa_backend_label(value.backend).to_string());
            }
            TaskRequestV2::Speaker(value) => {
                speaker_backends.insert(speaker_backend_label(value.backend).to_string());
            }
            TaskRequestV2::Morphosyntax(_)
            | TaskRequestV2::Utseg(_)
            | TaskRequestV2::Translate(_)
            | TaskRequestV2::Coref(_)
            | TaskRequestV2::Opensmile(_)
            | TaskRequestV2::Avqi(_) => {}
        }
    }

    for backend in ["local_whisper", "hk_tencent", "hk_aliyun", "hk_funaudio"] {
        assert!(
            asr_backends.contains(backend),
            "request fixtures should cover ASR backend {backend}"
        );
    }
    for backend in ["whisper", "wave2vec", "wav2vec_canto"] {
        assert!(
            fa_backends.contains(backend),
            "request fixtures should cover forced-alignment backend {backend}"
        );
    }
    for backend in ["pyannote", "nemo"] {
        assert!(
            speaker_backends.contains(backend),
            "request fixtures should cover speaker backend {backend}"
        );
    }
}

#[test]
fn worker_protocol_v2_response_manifest_covers_result_shape_matrix() {
    let mut result_kinds = BTreeSet::new();

    for entry in execute_response_entries() {
        let response = load_execute_response(&entry.file);
        validate_execute_response_invariants(&entry.file, &response).unwrap_or_else(|error| {
            panic!("response fixture {} failed invariants: {error}", entry.file)
        });
        if let Some(result) = &response.result {
            result_kinds.insert(result_kind_name(result).to_string());
        }
    }

    for result_kind in [
        "whisper_chunk_result",
        "monologue_asr_result",
        "whisper_token_timing_result",
        "indexed_word_timing_result",
        "morphosyntax_result",
        "utseg_result",
        "translation_result",
        "coref_result",
        "speaker_result",
        "opensmile_result",
        "avqi_result",
    ] {
        assert!(
            result_kinds.contains(result_kind),
            "response fixtures should cover result kind {result_kind}"
        );
    }
}

#[test]
fn worker_protocol_v2_request_invariants_reject_invalid_live_combinations() {
    let mut invalid_task_payload =
        load_execute_request("execute_request_speaker_prepared_audio.json");
    invalid_task_payload.task = InferenceTaskV2::Morphosyntax;
    let error = validate_execute_request_invariants(&invalid_task_payload)
        .expect_err("mismatched task/payload pair should fail");
    assert!(error.contains("expected payload kind morphosyntax"));

    let mut invalid_asr_transport = load_execute_request("execute_request_asr_prepared_audio.json");
    let TaskRequestV2::Asr(asr_request) = &mut invalid_asr_transport.payload else {
        panic!("asr fixture should deserialize as an ASR request");
    };
    asr_request.input = AsrInputV2::ProviderMedia(ProviderMediaInputV2 {
        media_path: "/tmp/provider.wav".into(),
        num_speakers: NumSpeakers(2),
    });
    let error = validate_execute_request_invariants(&invalid_asr_transport)
        .expect_err("local whisper with provider_media should fail");
    assert!(error.contains("local_whisper requires prepared_audio"));

    let mut invalid_fa_text_mode =
        load_execute_request("execute_request_fa_wave2vec_prepared_audio.json");
    let TaskRequestV2::ForcedAlignment(fa_request) = &mut invalid_fa_text_mode.payload else {
        panic!("forced-alignment fixture should deserialize as an FA request");
    };
    fa_request.text_mode = FaTextModeV2::CharJoined;
    let error = validate_execute_request_invariants(&invalid_fa_text_mode)
        .expect_err("wave2vec with char_joined text should fail");
    assert!(error.contains("wave2vec requires space_joined"));

    let mut invalid_fa_inline_json = load_execute_request("execute_request_fa_prepared_audio.json");
    let TaskRequestV2::ForcedAlignment(fa_request) = &invalid_fa_inline_json.payload else {
        panic!("forced-alignment fixture should deserialize as an FA request");
    };
    let Some(ArtifactRefV2::InlineJson(inline_json)) = invalid_fa_inline_json
        .attachments
        .iter_mut()
        .find(|attachment| attachment_id(attachment) == &fa_request.payload_ref_id)
    else {
        panic!("forced-alignment fixture should use inline_json payload");
    };
    inline_json
        .value
        .as_object_mut()
        .expect("inline JSON payload should be an object")
        .remove("word_utterance_indices");
    let error = validate_execute_request_invariants(&invalid_fa_inline_json)
        .expect_err("incomplete forced-alignment inline JSON should fail");
    assert!(error.contains("word_utterance_indices"));
}

#[tokio::test]
async fn worker_execute_v2_returns_typed_invalid_payload_for_mismatched_task_matrix() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");

    for (request_id, task) in [
        (
            "req-v2-mismatch-morphosyntax",
            InferenceTaskV2::Morphosyntax,
        ),
        ("req-v2-mismatch-speaker", InferenceTaskV2::Speaker),
        ("req-v2-mismatch-fa", InferenceTaskV2::ForcedAlignment),
    ] {
        let response = lease
            .execute_v2(&mismatched_execute_request(request_id, task))
            .await
            .expect("worker should return a typed invalid-payload response");

        assert_eq!(&*response.request_id, request_id);
        assert!(
            response.result.is_none(),
            "invalid payloads should not return a result"
        );
        match response.outcome {
            ExecuteOutcomeV2::Error { code, message } => {
                assert_eq!(code, ProtocolErrorCodeV2::InvalidPayload);
                assert!(
                    !message.trim().is_empty(),
                    "invalid payload responses should carry an error message"
                );
            }
            other => panic!("expected invalid-payload response, got {other:?}"),
        }
    }
}
