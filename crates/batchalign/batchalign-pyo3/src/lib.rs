// Test code is exempt from this crate's `deny`-level panic lints —
// see `<workspace>/talkbank-tools/book/src/operations/panic-audit/pyo3.md`.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! PyO3 worker runtime for batchalign3.
//!
//! This crate is the thin Rust↔Python boundary for batchalign3's ML worker
//! processes. Python workers are stateless inference endpoints; this crate
//! provides:
//!
//! - `worker_protocol` — IPC message dispatch (health, capabilities, infer,
//!   batch_infer, execute_v2)
//! - `worker_asr_exec` — ASR execution (Whisper, HK providers)
//! - `worker_fa_exec` — Forced alignment execution
//! - `worker_media_exec` — Speaker diarization, OpenSMILE, AVQI
//! - `worker_text_results` — Text task result normalization + token alignment
//! - `worker_artifacts` — Prepared artifact loading from IPC attachments
//! - `cantonese_asr_bridge` — HK/Cantonese provider projection + normalization
//!
//! **See also:** [Interface Map](../../INTERFACE_MAP.md) for unified documentation of all
//! Python/Rust boundaries, including Python caller locations and schema definitions.

mod cantonese_asr_bridge;
pub(crate) mod error;
pub(crate) mod py_json_bridge;
mod worker_artifacts;
mod worker_asr_exec;
mod worker_fa_exec;
mod worker_media_exec;
mod worker_protocol;
mod worker_text_results;

use pyo3::prelude::*;

/// Initialize tracing subscriber for structured logging.
///
/// Uses the `BATCHALIGN_RUST_LOG` env var for filtering (default: `warn`).
/// Safe to call multiple times — `try_init` is a no-op if already initialized.
fn init_rust_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_env("BATCHALIGN_RUST_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

/// batchalign_core — Rust worker runtime for batchalign3.
#[pymodule]
fn batchalign_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    init_rust_tracing();

    // Typed boundary exceptions. Python `batchalign/errors.py`
    // re-exports these so application catch sites match by class
    // rather than parsing the message string. See
    // `book/src/batchalign/architecture/python-rust-errors.md`.
    let py = m.py();
    m.add("BatchalignError", py.get_type::<error::BatchalignError>())?;
    m.add(
        "CHATValidationException",
        py.get_type::<error::CHATValidationException>(),
    )?;
    m.add(
        "DocumentValidationException",
        py.get_type::<error::DocumentValidationException>(),
    )?;
    m.add(
        "ConfigNotFoundError",
        py.get_type::<error::ConfigNotFoundError>(),
    )?;
    m.add("ConfigError", py.get_type::<error::ConfigError>())?;
    m.add(
        "PayloadTooLargeError",
        py.get_type::<error::PayloadTooLargeError>(),
    )?;
    m.add("SkipFileWarning", py.get_type::<error::SkipFileWarning>())?;

    // Worker protocol dispatch
    m.add_function(wrap_pyfunction!(
        worker_protocol::dispatch_protocol_message,
        m
    )?)?;

    // Worker V2 execution
    m.add_function(wrap_pyfunction!(
        worker_asr_exec::execute_asr_request_v2,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_fa_exec::execute_forced_alignment_request_v2,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_media_exec::execute_opensmile_request_v2,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_media_exec::execute_avqi_request_v2,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_media_exec::execute_speaker_request_v2,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_text_results::normalize_text_task_result,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(worker_text_results::align_tokens, m)?)?;

    // Worker artifact loaders
    m.add_function(wrap_pyfunction!(
        worker_artifacts::find_worker_attachment_by_id,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_artifacts::load_worker_json_attachment,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_artifacts::load_worker_prepared_text_json,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        worker_artifacts::load_worker_prepared_audio_f32le_bytes,
        m
    )?)?;

    // HK/Cantonese ASR bridges
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::clean_funaudio_segment_text,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::funaudio_segments_to_asr,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::tencent_result_detail_to_asr,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::aliyun_sentences_to_asr,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::normalize_cantonese,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        cantonese_asr_bridge::cantonese_char_tokens,
        m
    )?)?;

    Ok(())
}
