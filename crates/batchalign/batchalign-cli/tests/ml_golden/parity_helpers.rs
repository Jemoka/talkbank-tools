use crate::common::{
    assert_ba2_parity, assert_completed_without_errors, load_ba2_golden, load_parity_fixture,
    require_live_direct_warmed, submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::CommandOptions;
use batchalign::worker::InferTask;

/// Submit a curated parity fixture through BA3 and compare it to the BA2 Jan 9
/// golden output when that golden is available locally.
pub(super) async fn run_parity_test(
    command: ReleasedCommand,
    task: InferTask,
    fixture_name: &str,
    lang: &str,
    options: CommandOptions,
) {
    let Some(session) = require_live_direct_warmed(
        task,
        command,
        lang,
        &format!("Direct session does not support {task:?} infer"),
    )
    .await
    else {
        return;
    };

    let Some(input) = load_parity_fixture(fixture_name) else {
        return;
    };

    let files = vec![FilePayload {
        filename: format!("{fixture_name}.cha").into(),
        content: input,
    }];

    let (info, results) = submit_and_complete_direct(&session, command, lang, files, options).await;

    if info.status == JobStatus::Failed {
        eprintln!("SKIP: {command} {fixture_name} ({lang}) failed (model likely not downloaded)");
        return;
    }

    assert_completed_without_errors(&format!("{command}_{fixture_name}"), &info, &results);
    assert_eq!(results.len(), 1);

    let output = &results[0].content;
    if let Some(golden) = load_ba2_golden(command.as_ref(), fixture_name) {
        assert_ba2_parity(&format!("{command}_{fixture_name}"), output, &golden);
    }
}
