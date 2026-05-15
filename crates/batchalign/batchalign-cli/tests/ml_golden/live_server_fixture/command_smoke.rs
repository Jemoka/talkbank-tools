use crate::common::{
    assert_completed_without_errors, chat_fixtures, require_live_server, submit_and_complete,
};
use batchalign::api::{FilePayload, ReleasedCommand};
use batchalign::options::{
    CommandOptions, CommonOptions, CorefOptions, TranslateOptions, UtsegOptions,
};
use batchalign::worker::InferTask;

/// The fixture should run a second infer-only command family when that backend is available.
#[tokio::test]
async fn live_fixture_runs_utseg_job_when_available() {
    let Some(server) = require_live_server(
        InferTask::Utseg,
        "live fixture does not support utseg infer",
    )
    .await
    else {
        return;
    };

    let files = vec![FilePayload {
        filename: "fixture-utseg.cha".into(),
        content: chat_fixtures::ENG_MULTI_UTT.into(),
    }];
    let options = CommandOptions::Utseg(UtsegOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
        utseg_fallback: false.into(),
    });

    let (info, results) = submit_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Utseg,
        "eng",
        files,
        options,
    )
    .await;

    assert_completed_without_errors("live_fixture_utseg", &info, &results);
    assert_eq!(results.len(), 1);
    assert!(
        !results[0].content.is_empty(),
        "utseg output should not be empty"
    );
}

/// The fixture should run translate jobs when that backend is available.
#[tokio::test]
async fn live_fixture_runs_translate_job_when_available() {
    let Some(server) = require_live_server(
        InferTask::Translate,
        "live fixture does not support translate infer",
    )
    .await
    else {
        return;
    };

    let files = vec![FilePayload {
        filename: "fixture-translate.cha".into(),
        content: chat_fixtures::ENG_SIMPLE.into(),
    }];
    let options = CommandOptions::Translate(TranslateOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });

    let (info, results) = submit_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Translate,
        "eng",
        files,
        options,
    )
    .await;

    assert_completed_without_errors("live_fixture_translate", &info, &results);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].content.contains("%xtra:"),
        "translate output should contain %xtra tier"
    );
}

/// The fixture should run coref jobs when that backend is available.
#[tokio::test]
async fn live_fixture_runs_coref_job_when_available() {
    let Some(server) = require_live_server(
        InferTask::Coref,
        "live fixture does not support coref infer",
    )
    .await
    else {
        return;
    };

    let files = vec![FilePayload {
        filename: "fixture-coref.cha".into(),
        content: chat_fixtures::ENG_COREF.into(),
    }];
    let options = CommandOptions::Coref(CorefOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });
    let (info, results) = submit_and_complete(
        server.client(),
        server.base_url(),
        ReleasedCommand::Coref,
        "eng",
        files,
        options,
    )
    .await;

    assert_completed_without_errors("live_fixture_coref", &info, &results);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].content.contains("@Begin") && results[0].content.contains("*CHI:"),
        "coref output should remain valid CHAT with CHI speaker"
    );
}
