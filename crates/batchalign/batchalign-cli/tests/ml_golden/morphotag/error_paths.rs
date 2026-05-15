use crate::common::{
    LiveServerJobClient, require_live_direct_warmed, require_live_server,
    submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

fn morphotag_options() -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    })
}

/// Morphotag with an unsupported Stanza language should fail fast at request
/// validation rather than reaching the worker layer.
#[tokio::test]
async fn error_morphotag_unsupported_language_rejected() {
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
        "command": "morphotag",
        "lang": "xyz",
        "num_speakers": 1,
        "files": [{
            "filename": "unsupported.cha",
            "content": "@UTF8\n@Begin\n@Languages:\txyz\n@Participants:\tPAR Participant\n@ID:\txyz|test|PAR|||||Participant|||\n*PAR:\thello world .\n@End\n"
        }],
        "media_files": [],
        "media_mapping": "",
        "media_subdir": "",
        "source_dir": "",
        "options": {
            "command": "morphotag",
            "override_media_cache": false,
            "retokenize": false,
            "skipmultilang": false,
            "merge_abbrev": false,
            "no_l2_morphotag": false
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
    let text = resp.text().await.expect("error response body");
    assert!(
        (400..500).contains(&status),
        "Unsupported morphotag language should be rejected with 4xx, got {status}: {text}"
    );
    assert!(
        text.contains("not supported by Stanza"),
        "Expected unsupported-language message, got: {text}"
    );
}

/// Morphotag on an empty CHAT file should complete or fail gracefully.
#[tokio::test]
async fn error_morphotag_empty_file() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let empty_chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@End
";

    let files = vec![FilePayload {
        filename: "empty.cha".into(),
        content: empty_chat.into(),
    }];

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        files,
        morphotag_options(),
    )
    .await;

    assert!(
        matches!(info.status, JobStatus::Completed | JobStatus::Failed),
        "Empty file should complete or fail gracefully, not crash"
    );

    if info.status == JobStatus::Completed {
        assert!(!results.is_empty());
        assert!(
            results[0].content.contains("@End"),
            "Output should be valid CHAT (contains @End)"
        );
    }
}

/// Morphotag on `xxx` (unintelligible) should pass through without crash.
#[tokio::test]
async fn edge_morphotag_xxx_utterance() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\txxx .
*PAR:\thello world .
@End
";

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "xxx_test.cha".into(),
            content: chat.into(),
        }],
        morphotag_options(),
    )
    .await;

    assert!(
        matches!(info.status, JobStatus::Completed | JobStatus::Failed),
        "xxx utterance should not crash the server"
    );

    if info.status == JobStatus::Completed {
        assert!(
            results[0].content.contains("@End"),
            "Output should be valid CHAT"
        );
    }
}

/// Morphotag on `www` (untranscribed speech) should pass through without crash.
#[tokio::test]
async fn edge_morphotag_www_utterance() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\twww .
*PAR:\thello world .
@End
";

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "www_test.cha".into(),
            content: chat.into(),
        }],
        morphotag_options(),
    )
    .await;

    assert!(
        matches!(info.status, JobStatus::Completed | JobStatus::Failed),
        "www utterance should not crash the server"
    );

    if info.status == JobStatus::Completed {
        assert!(
            results[0].content.contains("@End"),
            "Output should be valid CHAT"
        );
    }
}

/// Malformed CHAT (no @Begin) should fail gracefully.
#[tokio::test]
async fn error_morphotag_invalid_chat() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let bad_chat = "\
@UTF8
@Languages:\teng
@Participants:\tPAR Participant
*PAR:\thello .
@End
";

    let (info, _results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "invalid.cha".into(),
            content: bad_chat.into(),
        }],
        morphotag_options(),
    )
    .await;

    assert!(
        matches!(info.status, JobStatus::Completed | JobStatus::Failed),
        "Invalid CHAT should not crash the server"
    );
}
