use crate::common::require_live_direct_warmed;
use crate::ml_golden::opensmile::helpers::{opensmile_options, prepare_opensmile_fixture_job};
use batchalign::api::{JobStatus, JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand};
use batchalign::worker::InferTask;

#[tokio::test]
async fn golden_opensmile_eng() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Opensmile,
        ReleasedCommand::Opensmile,
        "eng",
        "Direct session does not support OpenSMILE infer",
    )
    .await
    else {
        return;
    };
    let Some(fixture) = prepare_opensmile_fixture_job(session.state_dir(), "opensmile_direct_eng")
    else {
        return;
    };

    let submission = JobSubmission {
        command: ReleasedCommand::Opensmile,
        lang: LanguageSpec::try_from("eng").expect("valid eng language"),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: opensmile_options("eGeMAPSv02"),
        paths_mode: true,
        source_paths: vec![fixture.source_path.clone().into()],
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
        "opensmile_direct_eng: job should complete; status={:?}; file_results={}; error={}",
        info.status,
        detail.results.len(),
        file_error
    );
    let output_path = std::path::Path::new(&fixture.output_path)
        .parent()
        .expect("opensmile output dir")
        .join("test.opensmile.csv");
    let output =
        std::fs::read_to_string(&output_path).expect("opensmile should materialize csv output");
    assert!(
        !output.trim().is_empty(),
        "opensmile_direct_eng: output should be non-empty"
    );
}
