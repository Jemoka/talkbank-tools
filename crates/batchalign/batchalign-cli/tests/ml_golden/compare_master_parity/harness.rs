use std::path::PathBuf;

use crate::common::{
    assert_ba2_parity, assert_exact_text_parity, load_ba2_compare_master_golden,
    load_compare_fixture_pair, require_live_direct, submit_paths_and_complete_direct,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, CompareOptions};
use batchalign::worker::InferTask;

fn compare_opts() -> CommandOptions {
    CommandOptions::Compare(CompareOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    })
}

pub async fn run_compare_master_parity(fixture_name: &str) {
    let Some(session) = require_live_direct(
        InferTask::Morphosyntax,
        "Direct session does not support morphosyntax infer (required for compare parity)",
    )
    .await
    else {
        return;
    };

    let Some((main_text, gold_text)) = load_compare_fixture_pair(fixture_name) else {
        return;
    };
    let Some((golden_chat, golden_csv)) = load_ba2_compare_master_golden(fixture_name) else {
        return;
    };

    let tempdir = tempfile::tempdir().expect("tempdir");
    let input_dir = tempdir.path().join("in");
    let output_dir = tempdir.path().join("out");
    std::fs::create_dir_all(&input_dir).expect("create input dir");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let main_path = input_dir.join(format!("{fixture_name}.cha"));
    let gold_path = input_dir.join(format!("{fixture_name}.gold.cha"));
    std::fs::write(&main_path, main_text).expect("write main fixture");
    std::fs::write(&gold_path, gold_text).expect("write gold fixture");

    let requested_output = output_dir.join(format!("{fixture_name}.cha"));
    let (info, outputs) = submit_paths_and_complete_direct(
        &session,
        ReleasedCommand::Compare,
        "eng",
        vec![main_path.to_string_lossy().to_string()],
        vec![requested_output.to_string_lossy().to_string()],
        compare_opts(),
    )
    .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "compare parity job should complete"
    );
    assert_eq!(
        outputs.len(),
        1,
        "compare parity should return one CHAT output"
    );

    let csv_path = PathBuf::from(&output_dir).join("compare.csv");
    let csv_output =
        std::fs::read_to_string(&csv_path).expect("compare parity should write compare.csv");

    assert_ba2_parity(
        &format!("compare_master_chat_{fixture_name}"),
        &outputs[0],
        &golden_chat,
    );
    assert_exact_text_parity(
        &format!("compare_master_csv_{fixture_name}"),
        &csv_output,
        &golden_csv,
    );
}
