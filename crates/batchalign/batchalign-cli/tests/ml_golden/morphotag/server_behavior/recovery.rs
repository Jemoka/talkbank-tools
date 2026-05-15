use super::super::helpers::minimal_chat;
use crate::common::{LiveServerJobClient, require_live_server};
use batchalign::api::{
    FilePayload, FileStatusKind, JobInfo, JobStatus, LanguageCode3, LanguageSpec, ReleasedCommand,
};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// Opportunistic live check for the language-group failure isolation path.
///
/// We probe a small set of supported Stanza languages that are plausible to be
/// absent from a given local model cache. If none fail at dispatch time on this
/// machine, the test skips instead of becoming flaky.
#[tokio::test]
async fn morphotag_server_missing_language_group_only_fails_affected_file() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let candidates = [
        ("cat", "CAT", "bon dia"),
        ("dan", "DAN", "hej verden"),
        ("hrv", "HRV", "bok svijete"),
        ("eus", "EUS", "kaixo mundu"),
        ("slv", "SLV", "zdravo svet"),
        ("ces", "CES", "ahoj svete"),
    ];

    for (lang, speaker, utterance) in candidates {
        let initial = jobs
            .submit_content_job(
                ReleasedCommand::Morphotag,
                LanguageSpec::Resolved(LanguageCode3::eng()),
                vec![
                    FilePayload {
                        filename: "probe_eng.cha".into(),
                        content: minimal_chat("eng", "ENG", "hello world"),
                    },
                    FilePayload {
                        filename: format!("probe_{lang}.cha").into(),
                        content: minimal_chat(lang, speaker, utterance),
                    },
                ],
                CommandOptions::Morphotag(MorphotagOptions {
                    common: CommonOptions {
                        override_media_cache: true,
                        batch_window: 0,
                        ..CommonOptions::default()
                    },

                    ..Default::default()
                }),
            )
            .await;

        let Ok(_final_info) = tokio::time::timeout(
            tokio::time::Duration::from_secs(20),
            jobs.poll_done(&initial.job_id),
        )
        .await
        else {
            continue;
        };
        let results = jobs.job_results(&initial.job_id).await;
        if results.files.len() != 2 {
            continue;
        }

        let eng = results
            .files
            .iter()
            .find(|file| file.filename.as_ref() == "probe_eng.cha")
            .expect("eng probe file result");
        let candidate = results
            .files
            .iter()
            .find(|file| file.filename.as_ref() == format!("probe_{lang}.cha"))
            .expect("candidate probe file result");

        if eng.error.is_none() && eng.content.contains("%mor:") && candidate.error.is_some() {
            let candidate_error = candidate.error.as_deref().unwrap_or_default();
            assert!(
                candidate_error.contains("dispatch failed")
                    || candidate_error.contains("timed out")
                    || candidate_error.contains("worker"),
                "candidate failure should be a dispatch/runtime failure, got: {candidate_error}"
            );
            assert!(
                !eng.content.contains("L2|xxx"),
                "successful English neighbor should not inherit failure fallback"
            );
            return;
        }
    }

    eprintln!(
        "SKIP: no candidate language produced an isolated dispatch-time failure on this machine"
    );
}

/// Restarting a mixed-language morphotag job after one language-group failure
/// should preserve successful file results while re-queuing only the failed
/// file work.
#[tokio::test]
async fn morphotag_server_restart_preserves_successful_neighbors_after_language_group_failure() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphotag infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let candidates = [
        ("cat", "CAT", "bon dia"),
        ("dan", "DAN", "hej verden"),
        ("hrv", "HRV", "bok svijete"),
        ("eus", "EUS", "kaixo mundu"),
        ("slv", "SLV", "zdravo svet"),
        ("ces", "CES", "ahoj svete"),
    ];

    for (lang, speaker, utterance) in candidates {
        let initial = jobs
            .submit_content_job(
                ReleasedCommand::Morphotag,
                LanguageSpec::Resolved(LanguageCode3::eng()),
                vec![
                    FilePayload {
                        filename: "restart_probe_eng.cha".into(),
                        content: minimal_chat("eng", "ENG", "hello world"),
                    },
                    FilePayload {
                        filename: format!("restart_probe_{lang}.cha").into(),
                        content: minimal_chat(lang, speaker, utterance),
                    },
                ],
                CommandOptions::Morphotag(MorphotagOptions {
                    common: CommonOptions {
                        override_media_cache: true,
                        batch_window: 0,
                        ..CommonOptions::default()
                    },

                    ..Default::default()
                }),
            )
            .await;

        let Ok(final_info) = tokio::time::timeout(
            tokio::time::Duration::from_secs(20),
            jobs.poll_done(&initial.job_id),
        )
        .await
        else {
            continue;
        };
        if final_info.status != JobStatus::Failed {
            continue;
        }

        let results = jobs.job_results(&initial.job_id).await;
        if results.files.len() != 2 {
            continue;
        }

        let eng_before = results
            .files
            .iter()
            .find(|file| file.filename.as_ref() == "restart_probe_eng.cha")
            .expect("english restart probe result");
        let candidate_before = results
            .files
            .iter()
            .find(|file| file.filename.as_ref() == format!("restart_probe_{lang}.cha"))
            .expect("candidate restart probe result");

        let candidate_error = candidate_before.error.as_deref().unwrap_or_default();
        if eng_before.error.is_some()
            || !eng_before.content.contains("%mor:")
            || candidate_before.error.is_none()
            || !(candidate_error.contains("dispatch failed")
                || candidate_error.contains("timed out")
                || candidate_error.contains("worker"))
        {
            continue;
        }

        let restart_resp = server
            .client()
            .post(format!(
                "{}/jobs/{}/restart",
                server.base_url(),
                initial.job_id
            ))
            .send()
            .await
            .expect("restart request");
        assert_eq!(
            restart_resp.status(),
            200,
            "restart should succeed for failed mixed-language morphotag job"
        );
        let restarted = restart_resp
            .json::<JobInfo>()
            .await
            .expect("parse restart job info");
        assert_eq!(
            restarted.status,
            JobStatus::Queued,
            "restarted mixed-language morphotag job should return to queued"
        );

        let restarted_eng = restarted
            .file_statuses
            .iter()
            .find(|file| file.filename.as_ref() == "restart_probe_eng.cha")
            .expect("restarted english file status");
        let restarted_candidate = restarted
            .file_statuses
            .iter()
            .find(|file| file.filename.as_ref() == format!("restart_probe_{lang}.cha"))
            .expect("restarted candidate file status");

        assert_eq!(
            restarted_eng.status,
            FileStatusKind::Done,
            "restart should preserve the successful neighbor file"
        );
        assert_eq!(
            restarted_candidate.status,
            FileStatusKind::Queued,
            "restart should re-queue only the failed language-group file"
        );
        assert!(
            restarted_eng.error.is_none(),
            "restart should not attach stale error state to preserved successful files"
        );
        assert!(
            restarted_candidate.error.is_none(),
            "restart should clear stale error state before retrying the failed file"
        );

        let rerun_info = jobs.poll_done(&initial.job_id).await;
        assert!(
            matches!(rerun_info.status, JobStatus::Completed | JobStatus::Failed),
            "restarted morphotag job should reach a terminal state"
        );

        let rerun_results = jobs.job_results(&initial.job_id).await;
        let eng_after = rerun_results
            .files
            .iter()
            .find(|file| file.filename.as_ref() == "restart_probe_eng.cha")
            .expect("english result after restart");

        assert!(
            eng_after.error.is_none(),
            "successful neighbor should stay successful after restart"
        );
        assert_eq!(
            eng_before.content, eng_after.content,
            "restart should preserve successful English output exactly across rerun"
        );
        return;
    }

    eprintln!(
        "SKIP: no candidate language produced a restartable isolated language-group failure on this machine"
    );
}
