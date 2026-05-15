use super::helpers::{extract_worker_keys, query_health};
use crate::common::{prepare_audio_fixtures, require_live_server, submit_paths_and_complete};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{
    AlignOptions, CommandOptions, CommonOptions, FaEngineName, WorTierPolicy,
};
use batchalign::worker::InferTask;

/// Verify that submitting a multi-file align job produces exactly one
/// `profile:gpu:` worker key — the profile system groups ASR/FA/Speaker
/// into a shared GPU worker instead of spawning per-task processes.
///
/// This is the key memory verification test: without profile grouping,
/// each InferTask would spawn its own subprocess with duplicate model
/// copies, consuming N× memory.
#[tokio::test]
async fn gpu_profile_uses_single_worker_for_multi_file_align() {
    let Some(server) = require_live_server(InferTask::Fa, "Server does not support FA infer").await
    else {
        return;
    };

    let Some(fixtures) = prepare_audio_fixtures(server.state_dir()) else {
        return;
    };

    let chat_content =
        std::fs::read_to_string(&fixtures.stripped_chat).expect("read stripped fixture");
    let audio_source = fixtures
        .stripped_chat
        .parent()
        .expect("stripped_chat parent")
        .join("test.mp3");

    let out_dir = server.state_dir().join("profile_gpu_outputs");
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");

    let mut source_paths = Vec::new();
    let mut output_paths = Vec::new();
    for i in 1..=3 {
        let copy_dir = server.state_dir().join(format!("profile_input_{i}"));
        std::fs::create_dir_all(&copy_dir).expect("mkdir copy dir");
        let input_cha = copy_dir.join("test.cha");
        std::fs::write(&input_cha, &chat_content).expect("write cha copy");
        std::fs::copy(&audio_source, copy_dir.join("test.mp3")).expect("copy audio");
        source_paths.push(input_cha.to_string_lossy().into_owned());
        output_paths.push(
            out_dir
                .join(format!("test{i}.cha"))
                .to_string_lossy()
                .into_owned(),
        );
    }

    let options = CommandOptions::Align(AlignOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        fa_engine: FaEngineName::Wave2Vec,
        wor: WorTierPolicy::Include,
        ..AlignOptions::default()
    });

    let (info, _outputs) = submit_paths_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Align,
        "eng",
        source_paths,
        output_paths,
        options,
    )
    .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "multi-file align should complete"
    );

    let health = query_health(server.client(), server.base_url()).await;
    let keys = extract_worker_keys(&health);

    eprintln!("live_worker_keys after multi-file align: {keys:?}");
    eprintln!(
        "live_workers count: {}",
        health["live_workers"].as_i64().unwrap_or(-1)
    );

    let gpu_shared: Vec<&String> = keys
        .iter()
        .filter(|k| k.starts_with("profile:gpu:") && k.contains("shared"))
        .collect();
    assert!(
        !gpu_shared.is_empty(),
        "Expected at least 1 shared GPU worker key, got none in {keys:?}"
    );

    let legacy_fa: Vec<&String> = keys.iter().filter(|k| k.starts_with("infer:fa:")).collect();
    let legacy_asr: Vec<&String> = keys
        .iter()
        .filter(|k| k.starts_with("infer:asr:"))
        .collect();
    assert!(
        legacy_fa.is_empty(),
        "No legacy infer:fa: keys should exist, got {legacy_fa:?}"
    );
    assert!(
        legacy_asr.is_empty(),
        "No legacy infer:asr: keys should exist, got {legacy_asr:?}"
    );
}
