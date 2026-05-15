// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Cross-language staged roundtrip for worker-protocol V2 forced alignment.
//!
//! This test does not switch production worker dispatch over to V2. Its role
//! is narrower and architectural:
//!
//! - build the staged V2 FA request in Rust
//! - execute the staged Python V2 FA path against prepared artifacts
//! - parse the typed V2 response back into the established Rust FA domain
//!
//! That proves the seam is already coherent before any live dispatch wiring.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use batchalign::api::DurationMs;
use batchalign::chat_ops::fa::{FaEngineType, FaInferItem, FaTimingMode, FaWord};
use batchalign::chat_ops::{UtteranceIdx, WordIdx};
use batchalign::worker::artifacts_v2::PreparedArtifactStoreV2;
use batchalign::worker::fa_result_v2::parse_forced_alignment_result_v2;
use batchalign::worker::request_builder_v2::{
    ForcedAlignmentBuildInputV2, PreparedFaRequestIdsV2, build_forced_alignment_request_v2,
};
use common::resolve_python_for_module;
use tokio::process::Command;

/// Return the repo root for cross-language staged roundtrip fixtures.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Return whether ffmpeg is available for staged audio preparation.
fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Write a short WAV tone fixture for the staged V2 request builder.
async fn write_test_tone(path: &Path) {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=16000",
            "-t",
            "0.30",
            path.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .expect("ffmpeg process should run");
    assert!(
        output.status.success(),
        "ffmpeg should generate the roundtrip tone fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build small FA word records for the result-adapter assertion.
fn make_words(texts: &[&str]) -> Vec<FaWord> {
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| FaWord {
            utterance_index: UtteranceIdx(0),
            utterance_word_index: WordIdx(index),
            text: (*text).into(),
        })
        .collect()
}

#[tokio::test]
async fn staged_worker_v2_fa_roundtrip_crosses_rust_and_python() {
    let Some(python) = resolve_python_for_module("batchalign.worker._fa_v2") else {
        eprintln!("SKIP: Python with batchalign.worker._fa_v2 not available");
        return;
    };
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not installed");
        return;
    }

    let repo_root = repo_root();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let store = PreparedArtifactStoreV2::new(tempdir.path().join("artifacts"))
        .expect("prepared artifact store");
    let wav_path = tempdir.path().join("tone.wav");
    write_test_tone(&wav_path).await;

    let request = build_forced_alignment_request_v2(
        &store,
        ForcedAlignmentBuildInputV2 {
            ids: &PreparedFaRequestIdsV2::new(
                "req-fa-roundtrip-1",
                "payload-fa-roundtrip-1",
                "audio-fa-roundtrip-1",
            ),
            infer_item: &FaInferItem {
                words: vec!["hello".into(), "world".into()],
                word_ids: vec!["u0:w0".into(), "u0:w1".into()],
                word_utterance_indices: vec![0, 0],
                word_utterance_word_indices: vec![0, 1],
                audio_path: wav_path.to_string_lossy().into_owned(),
                audio_start_ms: 0,
                audio_end_ms: 150,
                timing_mode: FaTimingMode::Continuous,
            },
            engine: FaEngineType::WhisperFa,
        },
    )
    .await
    .expect("staged V2 FA request should build");

    let request_path = tempdir.path().join("request.json");
    let response_path = tempdir.path().join("response.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request).expect("request should serialize"),
    )
    .expect("write staged request");

    let script_path = repo_root.join("python/batchalign/tests/support/worker_fa_v2_roundtrip.py");
    let output = Command::new(&python)
        .current_dir(&repo_root)
        .args([
            script_path.to_string_lossy().as_ref(),
            request_path.to_string_lossy().as_ref(),
            response_path.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .expect("python staged V2 FA roundtrip should run");
    assert!(
        output.status.success(),
        "python staged V2 FA roundtrip failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let response: batchalign::types::worker_v2::ExecuteResponseV2 =
        serde_json::from_slice(&fs::read(&response_path).expect("read staged response"))
            .expect("staged response should parse");
    let timings = parse_forced_alignment_result_v2(
        &response,
        &make_words(&["hello", "world"]),
        DurationMs(0),
        FaTimingMode::Continuous,
    )
    .expect("staged response should parse back into Rust FA domain");

    assert_eq!(timings.len(), 2);
    assert_eq!(timings[0].as_ref().expect("timing").start_ms, 100);
    assert_eq!(timings[0].as_ref().expect("timing").end_ms, 100);
    assert_eq!(timings[1].as_ref().expect("timing").start_ms, 250);
    assert_eq!(timings[1].as_ref().expect("timing").end_ms, 250);
}
