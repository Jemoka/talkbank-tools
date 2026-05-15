use crate::common::{LiveServerJobClient, require_live_server_warmed};
use crate::ml_golden::avqi::helpers::{avqi_options, prepare_avqi_fixture_job};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::worker::InferTask;

#[tokio::test]
async fn avqi_server_eng_pair() {
    let Some(server) = require_live_server_warmed(
        InferTask::Avqi,
        ReleasedCommand::Avqi,
        "eng",
        "live server does not support AVQI infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_avqi_fixture_job(server.state_dir(), "avqi_server_eng") else {
        return;
    };

    let (info, _outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Avqi,
            "eng",
            vec![fixture.cs_source_path],
            vec![fixture.output_path.clone()],
            avqi_options(),
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
        "avqi_server_eng_pair: job should complete; error={}",
        file_error
    );
    let output_path = std::path::Path::new(&fixture.output_path)
        .parent()
        .expect("avqi output dir")
        .join("test.avqi.txt");
    let output = std::fs::read_to_string(&output_path).expect("avqi should materialize txt output");
    assert!(
        output.contains("avqi,"),
        "avqi_server_eng_pair: output should include AVQI metrics"
    );
}
