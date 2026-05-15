use crate::common::require_live_direct_warmed;
use crate::ml_golden::avqi::helpers::{avqi_options, prepare_avqi_fixture_job};
use batchalign::api::{JobStatus, JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand};
use batchalign::worker::InferTask;

#[tokio::test]
async fn golden_avqi_eng_pair() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Avqi,
        ReleasedCommand::Avqi,
        "eng",
        "Direct session does not support AVQI infer",
    )
    .await
    else {
        return;
    };
    let Some(fixture) = prepare_avqi_fixture_job(session.state_dir(), "avqi_direct_eng") else {
        return;
    };

    let submission = JobSubmission {
        command: ReleasedCommand::Avqi,
        lang: LanguageSpec::try_from("eng").expect("valid eng language"),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: avqi_options(),
        paths_mode: true,
        source_paths: vec![fixture.cs_source_path.clone().into()],
        output_paths: vec![fixture.output_path.clone().into()],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };
    let (info, detail) = session.run_submission(submission).await;
    let file_error = detail
        .results
        .first()
        .and_then(|result| result.error.clone())
        .unwrap_or_default();

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "golden_avqi_eng_pair: job should complete; status={:?}; file_results={}; error={}",
        info.status,
        detail.results.len(),
        file_error
    );
    let output_path = std::path::Path::new(&fixture.output_path)
        .parent()
        .expect("avqi output dir")
        .join("test.avqi.txt");
    let output = std::fs::read_to_string(&output_path).expect("avqi should materialize txt output");
    assert!(
        output.contains("avqi,"),
        "golden_avqi_eng_pair: output should include AVQI metrics"
    );
}
