use crate::common::{LiveServerJobClient, require_live_server_warmed};
use crate::ml_golden::opensmile::helpers::{opensmile_options, prepare_opensmile_fixture_job};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::worker::InferTask;

#[tokio::test]
async fn opensmile_server_eng() {
    let Some(server) = require_live_server_warmed(
        InferTask::Opensmile,
        ReleasedCommand::Opensmile,
        "eng",
        "live server does not support OpenSMILE infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_opensmile_fixture_job(server.state_dir(), "opensmile_server_eng")
    else {
        return;
    };

    let (info, _outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Opensmile,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path.clone()],
            opensmile_options("eGeMAPSv02"),
        )
        .await;
    let results = jobs.job_results(&info.job_id).await;
    let file_error = results
        .files
        .first()
        .and_then(|result| result.error.clone())
        .unwrap_or_default();

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "opensmile_server_eng: job should complete; error={}",
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
        "opensmile_server_eng: output should be non-empty"
    );
}
