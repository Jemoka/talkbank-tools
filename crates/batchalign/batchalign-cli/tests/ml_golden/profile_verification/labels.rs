use super::helpers::{ENG_TEXT_FIXTURE, extract_worker_keys, query_health};
use crate::common::{
    assert_completed_without_errors, require_live_server, submit_paths_and_complete,
};
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// Assert that ALL live worker keys use the `profile:` prefix and NONE
/// use the legacy `infer:` prefix.
///
/// This is a regression guard: if new code accidentally bypasses the
/// profile system and spawns per-task workers, this test will catch it.
#[tokio::test]
async fn profile_worker_keys_use_profile_labels() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let input_dir = server.state_dir().join("profile_labels_inputs");
    std::fs::create_dir_all(&input_dir).expect("mkdir input dir");
    let input_path = input_dir.join("labels_test.cha");
    std::fs::write(&input_path, ENG_TEXT_FIXTURE).expect("write text fixture");

    let out_dir = server.state_dir().join("profile_labels_out");
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");

    let options = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    });

    let (info, _outputs) = submit_paths_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Morphotag,
        "eng",
        vec![input_path.to_string_lossy().into_owned()],
        vec![
            out_dir
                .join("labels_test.cha")
                .to_string_lossy()
                .into_owned(),
        ],
        options,
    )
    .await;

    assert_completed_without_errors("profile_labels", &info, &[]);

    let health = query_health(server.client(), server.base_url()).await;
    let keys = extract_worker_keys(&health);

    eprintln!("live_worker_keys for regression guard: {keys:?}");

    for key in &keys {
        assert!(
            key.starts_with("profile:"),
            "Worker key should use profile: prefix, got: {key}"
        );
    }

    let legacy_keys: Vec<&String> = keys.iter().filter(|k| k.starts_with("infer:")).collect();
    assert!(
        legacy_keys.is_empty(),
        "No legacy infer: keys should exist, got {legacy_keys:?}"
    );
}
