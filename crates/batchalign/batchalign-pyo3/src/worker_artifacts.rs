//! Rust-owned worker-protocol V2 prepared-artifact lookup and reads.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "8. Worker Artifact Loading" for:
//! - Python callers: All V2 executors use these functions
//! - Design: File-backed artifacts with future shared-memory migration path.

use std::fs;
use std::path::Path;

use crate::error::BatchalignBoundaryError;
use batchalign_types::worker_v2::{
    ArtifactRefV2, PreparedAudioEncodingV2, PreparedAudioRefV2, PreparedTextEncodingV2,
    PreparedTextRefV2,
};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::py_json_bridge::py_to_json_value;

pub(crate) fn validate_prepared_audio_attachment(
    attachment: &PreparedAudioRefV2,
) -> Result<(), String> {
    if attachment.channels.0 == 0 {
        return Err(format!(
            "prepared audio attachment {:?} must declare at least one channel",
            attachment.id.as_ref()
        ));
    }
    if attachment.sample_rate_hz.0 == 0 {
        return Err(format!(
            "prepared audio attachment {:?} must declare positive sample_rate_hz",
            attachment.id.as_ref()
        ));
    }
    Ok(())
}

pub(crate) fn validate_attachment_descriptors(attachments: &[ArtifactRefV2]) -> Result<(), String> {
    for attachment in attachments {
        if let ArtifactRefV2::PreparedAudio(value) = attachment {
            validate_prepared_audio_attachment(value)?;
        }
    }
    Ok(())
}

fn parse_attachments(attachments: &Bound<'_, PyAny>) -> PyResult<Vec<ArtifactRefV2>> {
    let attachments: Vec<ArtifactRefV2> = serde_json::from_value(py_to_json_value(attachments)?)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())?;
    validate_attachment_descriptors(&attachments).map_err(|error: String| {
        BatchalignBoundaryError::Internal { message: error }.into_py_err()
    })?;
    Ok(attachments)
}

fn parse_prepared_text_attachment(attachment: &Bound<'_, PyAny>) -> PyResult<PreparedTextRefV2> {
    serde_json::from_value(py_to_json_value(attachment)?)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())
}

fn parse_prepared_audio_attachment(attachment: &Bound<'_, PyAny>) -> PyResult<PreparedAudioRefV2> {
    let attachment: PreparedAudioRefV2 = serde_json::from_value(py_to_json_value(attachment)?)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())?;
    validate_prepared_audio_attachment(&attachment).map_err(|error: String| {
        BatchalignBoundaryError::Internal { message: error }.into_py_err()
    })?;
    Ok(attachment)
}

pub(crate) fn find_attachment<'a>(
    attachments: &'a [ArtifactRefV2],
    artifact_id: &str,
) -> PyResult<&'a ArtifactRefV2> {
    attachments
        .iter()
        .find(|attachment| match attachment {
            ArtifactRefV2::PreparedAudio(value) => value.id.as_ref() == artifact_id,
            ArtifactRefV2::PreparedText(value) => value.id.as_ref() == artifact_id,
            ArtifactRefV2::InlineJson(value) => value.id.as_ref() == artifact_id,
        })
        .ok_or_else(|| {
            BatchalignBoundaryError::internal(format!(
                "missing worker protocol V2 attachment {artifact_id:?}"
            ))
            .into()
        })
}

fn read_attachment_slice(path: &Path, byte_offset: usize, byte_len: usize) -> PyResult<Vec<u8>> {
    let raw =
        fs::read(path).map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())?;
    let end = byte_offset.checked_add(byte_len).ok_or_else(|| {
        BatchalignBoundaryError::internal(format!(
            "prepared artifact slice overflow for {}",
            path.display()
        ))
        .into_py_err()
    })?;
    if end > raw.len() {
        return Err(BatchalignBoundaryError::internal(format!(
            "prepared artifact slice {byte_offset}:{end} is outside {}",
            path.display()
        ))
        .into());
    }
    Ok(raw[byte_offset..end].to_vec())
}

pub(crate) fn load_prepared_text_json_impl(attachment: &PreparedTextRefV2) -> PyResult<String> {
    if attachment.encoding != PreparedTextEncodingV2::Utf8Json {
        return Err(BatchalignBoundaryError::internal(format!(
            "unsupported prepared text encoding utf8_json for {:?}",
            attachment.id.as_ref()
        ))
        .into());
    }
    let raw = read_attachment_slice(
        Path::new(attachment.path.as_ref()),
        attachment.byte_offset.0 as usize,
        attachment.byte_len.0 as usize,
    )?;
    String::from_utf8(raw).map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())
}

pub(crate) fn load_prepared_audio_bytes_impl(attachment: &PreparedAudioRefV2) -> PyResult<Vec<u8>> {
    validate_prepared_audio_attachment(attachment).map_err(|error: String| {
        BatchalignBoundaryError::Internal { message: error }.into_py_err()
    })?;
    if attachment.encoding != PreparedAudioEncodingV2::PcmF32le {
        return Err(BatchalignBoundaryError::internal(format!(
            "unsupported prepared audio encoding pcm_f32le for {:?}",
            attachment.id.as_ref()
        ))
        .into());
    }

    let raw = read_attachment_slice(
        Path::new(attachment.path.as_ref()),
        attachment.byte_offset.0 as usize,
        attachment.byte_len.0 as usize,
    )?;
    let expected_values = attachment.frame_count.0 as usize * attachment.channels.0 as usize;
    let expected_bytes = expected_values * std::mem::size_of::<f32>();
    if raw.len() != expected_bytes {
        return Err(BatchalignBoundaryError::internal(format!(
            "prepared audio artifact {:?} has {} bytes, expected {expected_bytes}",
            attachment.id.as_ref(),
            raw.len()
        ))
        .into());
    }
    Ok(raw)
}

pub(crate) fn require_prepared_audio_attachment<'a>(
    attachments: &'a [ArtifactRefV2],
    artifact_id: &str,
) -> PyResult<&'a PreparedAudioRefV2> {
    match find_attachment(attachments, artifact_id)? {
        ArtifactRefV2::PreparedAudio(value) => {
            validate_prepared_audio_attachment(value).map_err(|error: String| BatchalignBoundaryError::Internal { message: error }.into_py_err())?;
            Ok(value)
        }
        other => Err(BatchalignBoundaryError::internal(format!(
            "worker protocol V2 attachment {artifact_id:?} had type {}, expected PreparedAudioRefV2",
            match other {
                ArtifactRefV2::PreparedAudio(_) => "PreparedAudioRefV2",
                ArtifactRefV2::PreparedText(_) => "PreparedTextRefV2",
                ArtifactRefV2::InlineJson(_) => "InlineJsonRefV2",
            }
        )).into()),
    }
}

pub(crate) fn require_prepared_text_attachment<'a>(
    attachments: &'a [ArtifactRefV2],
    artifact_id: &str,
) -> PyResult<&'a PreparedTextRefV2> {
    match find_attachment(attachments, artifact_id)? {
        ArtifactRefV2::PreparedText(value) => Ok(value),
        other => Err(BatchalignBoundaryError::internal(format!(
            "worker protocol V2 attachment {artifact_id:?} had type {}, expected PreparedTextRefV2",
            match other {
                ArtifactRefV2::PreparedAudio(_) => "PreparedAudioRefV2",
                ArtifactRefV2::PreparedText(_) => "PreparedTextRefV2",
                ArtifactRefV2::InlineJson(_) => "InlineJsonRefV2",
            }
        ))
        .into()),
    }
}

#[pyfunction]
#[pyo3(signature = (attachments, artifact_id))]
pub(crate) fn find_worker_attachment_by_id(
    attachments: &Bound<'_, PyAny>,
    artifact_id: &str,
) -> PyResult<String> {
    let attachments = parse_attachments(attachments)?;
    let attachment = find_attachment(&attachments, artifact_id)?;
    serde_json::to_string(attachment)
        .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err())
}

#[pyfunction]
#[pyo3(signature = (attachments, artifact_id))]
pub(crate) fn load_worker_json_attachment(
    attachments: &Bound<'_, PyAny>,
    artifact_id: &str,
) -> PyResult<String> {
    let attachments = parse_attachments(attachments)?;
    match find_attachment(&attachments, artifact_id)? {
        ArtifactRefV2::InlineJson(value) => serde_json::to_string(&value.value)
            .map_err(|error| BatchalignBoundaryError::internal(error).into_py_err()),
        ArtifactRefV2::PreparedText(value) => load_prepared_text_json_impl(value),
        _ => Err(BatchalignBoundaryError::internal(format!(
            "worker protocol V2 attachment {artifact_id:?} does not contain JSON payload data"
        ))
        .into()),
    }
}

#[pyfunction]
#[pyo3(signature = (attachment))]
pub(crate) fn load_worker_prepared_text_json(attachment: &Bound<'_, PyAny>) -> PyResult<String> {
    let attachment = parse_prepared_text_attachment(attachment)?;
    load_prepared_text_json_impl(&attachment)
}

#[pyfunction]
#[pyo3(signature = (attachment))]
pub(crate) fn load_worker_prepared_audio_f32le_bytes<'py>(
    py: Python<'py>,
    attachment: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let attachment = parse_prepared_audio_attachment(attachment)?;
    let raw = load_prepared_audio_bytes_impl(&attachment)?;
    Ok(PyBytes::new(py, &raw))
}
