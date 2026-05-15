use super::helpers::{ENG_TEXT_FIXTURE, extract_worker_keys, query_health};
use crate::common::require_live_server;
use crate::common::submit_paths_and_complete;
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions, UtsegOptions};
use batchalign::worker::InferTask;

/// Verify that morphotag and utseg share the same Stanza profile worker.
///
/// Both commands use Stanza NLP processors. The profile system should
/// group them under a single `profile:stanza:` key rather than spawning
/// separate workers for each task.
#[tokio::test]
async fn stanza_profile_groups_morphotag_and_utseg() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    if !server.has_infer_task(InferTask::Utseg) {
        eprintln!("SKIP: Server does not support utseg infer");
        return;
    }

    let input_dir = server.state_dir().join("profile_stanza_inputs");
    std::fs::create_dir_all(&input_dir).expect("mkdir input dir");
    let input_path = input_dir.join("stanza_test.cha");
    std::fs::write(&input_path, ENG_TEXT_FIXTURE).expect("write text fixture");

    let morphotag_out_dir = server.state_dir().join("profile_stanza_morphotag_out");
    std::fs::create_dir_all(&morphotag_out_dir).expect("mkdir morphotag output");

    let morphotag_options = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    });

    let (info_mt, _outputs_mt) = submit_paths_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Morphotag,
        "eng",
        vec![input_path.to_string_lossy().into_owned()],
        vec![
            morphotag_out_dir
                .join("stanza_test.cha")
                .to_string_lossy()
                .into_owned(),
        ],
        morphotag_options,
    )
    .await;

    assert_eq!(
        info_mt.status,
        JobStatus::Completed,
        "morphotag should complete"
    );

    let utseg_out_dir = server.state_dir().join("profile_stanza_utseg_out");
    std::fs::create_dir_all(&utseg_out_dir).expect("mkdir utseg output");

    let utseg_options = CommandOptions::Utseg(UtsegOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
        utseg_fallback: false.into(),
    });

    let (info_ut, _outputs_ut) = submit_paths_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Utseg,
        "eng",
        vec![input_path.to_string_lossy().into_owned()],
        vec![
            utseg_out_dir
                .join("stanza_test.cha")
                .to_string_lossy()
                .into_owned(),
        ],
        utseg_options,
    )
    .await;

    assert_eq!(
        info_ut.status,
        JobStatus::Completed,
        "utseg should complete"
    );

    let health = query_health(server.client(), server.base_url()).await;
    let keys = extract_worker_keys(&health);

    eprintln!("live_worker_keys after morphotag + utseg: {keys:?}");

    let stanza_keys: Vec<&String> = keys
        .iter()
        .filter(|k| k.starts_with("profile:stanza:"))
        .collect();
    assert_eq!(
        stanza_keys.len(),
        1,
        "Expected exactly 1 profile:stanza: worker key after morphotag + utseg, got {stanza_keys:?}"
    );
}
