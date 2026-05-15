use crate::common::{LiveServerJobClient, require_live_server};
use batchalign::worker::InferTask;

/// Submitting a job with an invalid command name should be rejected.
///
/// This remains an HTTP-server test because the behavior under test is request
/// deserialization and validation at the `/jobs` boundary, not command execution.
#[tokio::test]
async fn error_invalid_command_name() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let body = serde_json::json!({
        "command": "nonexistent_command",
        "lang": "eng",
        "num_speakers": 1,
        "files": [{
            "filename": "test.cha",
            "content": "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n*PAR:\thello .\n@End\n"
        }],
        "media_files": [],
        "media_mapping": "",
        "media_subdir": "",
        "source_dir": "",
        "options": {
            "command": "nonexistent_command",
            "override_media_cache": false
        },
        "paths_mode": false,
        "source_paths": [],
        "output_paths": [],
        "display_names": [],
        "debug_traces": false,
        "before_paths": []
    });

    let resp = jobs.post_json("/jobs", &body).await;
    let status = resp.status().as_u16();
    assert!(
        (400..500).contains(&status),
        "Invalid command should be rejected with 4xx, got {status}"
    );
}
