use crate::common::{LiveDirectJobClient, assert_completed_without_errors, require_live_direct};
use crate::ml_golden::align::helpers::{align_options, prepare_align_fixture_job};
use crate::ml_golden::audio_helpers::assert_all_utterances_timed;
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{FaEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

fn first_main_line(chat: &str, label: &str) -> String {
    chat.lines()
        .find(|line| line.starts_with('*'))
        .unwrap_or_else(|| panic!("{label}: expected at least one main-tier line"))
        .to_string()
}

#[tokio::test]
async fn direct_align_produces_timed_output() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_align_fixture_job(jobs.state_dir(), "direct_align_verify") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("direct_align_verify", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "direct_align_verify");
}

#[tokio::test]
async fn direct_align_before_preserves_existing_first_bullet() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_align_fixture_job(
        jobs.state_dir(),
        "direct_align_before_preserves_first_bullet",
    ) else {
        return;
    };

    let expected_first = first_main_line(
        &std::fs::read_to_string(&fixture.before_path).expect("read before CHAT"),
        "direct_align_before_before_file",
    );

    let (info, outputs) = jobs
        .submit_paths_job_with_before(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            vec![fixture.before_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("direct_align_before_preserves_first_bullet", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        first_main_line(&outputs[0], "direct_align_before_output"),
        expected_first,
        "incremental align should preserve the first unchanged main-tier bullet"
    );
}
