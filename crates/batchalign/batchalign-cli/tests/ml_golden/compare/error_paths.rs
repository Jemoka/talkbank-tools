use crate::common::require_live_direct;
use batchalign::api::{
    JobStatus, JobSubmission, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
};
use batchalign::options::{CommandOptions, CommonOptions, CompareOptions};
use batchalign::worker::InferTask;

/// Compare without a `.gold.cha` companion should fail cleanly and should not
/// leave behind partial output artifacts from the new compare execution path.
#[tokio::test]
async fn error_compare_missing_gold_companion_fails_cleanly() {
    let Some(session) = require_live_direct(
        InferTask::Morphosyntax,
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let input_dir = session.state_dir().join("compare_missing_gold_in");
    let output_dir = session.state_dir().join("compare_missing_gold_out");
    std::fs::create_dir_all(&input_dir).expect("mkdir compare input dir");
    std::fs::create_dir_all(&output_dir).expect("mkdir compare output dir");

    let main_path = input_dir.join("sample.cha");
    let requested_output = output_dir.join("requested.cha");
    let expected_chat = output_dir.join("sample.cha");
    let expected_csv = output_dir.join("sample.compare.csv");

    std::fs::write(
        &main_path,
        "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\thello world .
@End
",
    )
    .expect("write compare main input");

    let submission = JobSubmission {
        command: ReleasedCommand::Compare,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Compare(CompareOptions {
            common: CommonOptions {
                override_media_cache: true,
                ..CommonOptions::default()
            },
            merge_abbrev: false.into(),
        }),
        paths_mode: true,
        source_paths: vec![main_path.to_string_lossy().as_ref().into()],
        output_paths: vec![requested_output.to_string_lossy().as_ref().into()],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let (final_info, detail) = session.run_submission(submission).await;
    assert_eq!(
        final_info.status,
        JobStatus::Failed,
        "compare without a gold companion should fail cleanly"
    );
    assert_eq!(
        detail.results.len(),
        1,
        "compare failure should still record one file result"
    );
    let error = detail.results[0].error.clone().unwrap_or_default();
    assert!(
        error.contains("gold")
            || error.contains("No such file")
            || error.contains("failed to read"),
        "compare missing-gold failure should mention the missing companion, got: {error}"
    );
    assert!(
        !expected_chat.exists(),
        "compare failure should not materialize a partial CHAT output"
    );
    assert!(
        !expected_csv.exists(),
        "compare failure should not materialize a partial CSV sidecar"
    );
}
