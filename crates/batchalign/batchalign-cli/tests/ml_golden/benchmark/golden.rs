use crate::common::{prepare_audio_fixtures, require_live_direct};
use batchalign::api::{JobStatus, JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand};
use batchalign::options::{
    AsrEngineName, BenchmarkOptions, CommandOptions, CommonOptions, WorTierPolicy,
};
use batchalign::worker::InferTask;

use super::helpers::count_wor_tiers;

fn media_header(chat: &str, label: &str) -> String {
    chat.lines()
        .find(|line| line.starts_with("@Media:\t"))
        .unwrap_or_else(|| panic!("{label}: expected @Media header"))
        .to_string()
}

#[tokio::test]
async fn golden_benchmark_eng() {
    let Some(session) = require_live_direct(
        InferTask::Asr,
        "Direct session does not support ASR infer (required for benchmark)",
    )
    .await
    else {
        return;
    };

    let Some(fixtures) = prepare_audio_fixtures(session.state_dir()) else {
        return;
    };

    let out_dir = session.state_dir().join("out_benchmark");
    std::fs::create_dir_all(&out_dir).expect("mkdir");
    let output_audio = out_dir.join("test.csv");
    let output_gold = out_dir.join("test.cha");

    let options = CommandOptions::Benchmark(BenchmarkOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        asr_engine: AsrEngineName::Whisper,
        wor: WorTierPolicy::Omit,
        merge_abbrev: false.into(),
    });

    let submission = JobSubmission {
        command: ReleasedCommand::Benchmark,
        lang: LanguageSpec::try_from("eng").expect("valid eng language"),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: true,
        source_paths: vec![
            fixtures.audio.to_string_lossy().to_string().into(),
            fixtures.chat.to_string_lossy().to_string().into(),
        ],
        output_paths: vec![
            output_audio.to_string_lossy().to_string().into(),
            output_gold.to_string_lossy().to_string().into(),
        ],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let (info, _detail) = session.run_submission(submission).await;
    let csv_output = std::fs::read_to_string(out_dir.join("test.compare.csv"))
        .expect("benchmark should materialize compare csv sidecar");
    let chat_output = std::fs::read_to_string(out_dir.join("test.cha"))
        .expect("benchmark should materialize chat output");

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "benchmark_eng: job should complete"
    );
    assert_eq!(
        count_wor_tiers(&chat_output),
        0,
        "benchmark_eng: %wor tier should be absent when benchmark wor=Omit"
    );
    assert_eq!(
        media_header(&chat_output, "benchmark_eng"),
        "@Media:\ttest, audio",
        "benchmark_eng: benchmark should preserve the original media basename in the CHAT output"
    );
    assert!(
        csv_output.contains("wer") || csv_output.contains("token"),
        "benchmark_eng: expected benchmark csv sidecar to contain comparison metrics"
    );
}
