use crate::common::prepare_named_audio;
use crate::common::{
    LiveDirectJobClient, LiveServerJobClient, require_live_direct, require_live_server,
    require_revai_key,
};
use crate::ml_golden::align::helpers::{align_options, align_options_with_utr};
use crate::ml_golden::audio_helpers::{align_audio_clip, assert_all_utterances_timed};
use batchalign::api::{JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand};
use batchalign::options::{FaEngineName, UtrEngine, WorTierPolicy};
use batchalign::worker::InferTask;

#[tokio::test]
async fn align_spa_wav2vec() {
    align_audio_clip("spa_marrero_clip", "spa_marrero_timed", "spa", "align_spa").await;
}

#[tokio::test]
async fn align_fra_wav2vec() {
    align_audio_clip("fra_geneva_clip", "fra_geneva_timed", "fra", "align_fra").await;
}

#[tokio::test]
async fn align_jpn_wav2vec() {
    align_audio_clip("jpn_tyo_clip", "jpn_tyo_timed", "jpn", "align_jpn").await;
}

#[tokio::test]
async fn align_yue_wav2vec() {
    align_audio_clip_with_options(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_yue",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::Whisper,
        ),
    )
    .await;
}

#[tokio::test]
async fn align_eng_multi_speaker_wav2vec() {
    align_audio_clip(
        "eng_multi_speaker",
        "eng_multi_speaker",
        "eng",
        "align_eng_multi_speaker",
    )
    .await;
}

async fn align_audio_clip_server(audio_name: &str, chat_name: &str, lang: &str, label: &str) {
    align_audio_clip_server_with_options(
        audio_name,
        chat_name,
        lang,
        label,
        align_options(FaEngineName::Wave2Vec, WorTierPolicy::Include),
    )
    .await;
}

async fn align_audio_clip_with_options(
    audio_name: &str,
    chat_name: &str,
    lang: &str,
    label: &str,
    options: batchalign::options::CommandOptions,
) {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixtures) = prepare_named_audio(jobs.state_dir(), audio_name, Some(chat_name)) else {
        return;
    };

    let out_dir = jobs.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let input_basename = fixtures
        .stripped_chat
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let output_path = out_dir.join(&input_basename);

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            lang,
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![output_path.to_string_lossy().into()],
            options,
        )
        .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "{label}: direct job should complete; error={:?}",
        info.error
    );
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], label);
}

fn untimed_chat_text(chat: &str) -> String {
    chat.lines()
        .map(|line| {
            if line.starts_with('*') {
                let mut out = String::new();
                let mut chars = line.chars().peekable();
                while let Some(ch) = chars.next() {
                    if ch == '\u{0015}' {
                        for next in chars.by_ref() {
                            if next == '\u{0015}' {
                                break;
                            }
                        }
                    } else {
                        out.push(ch);
                    }
                }
                out
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

async fn align_untimed_audio_clip_with_options(
    audio_name: &str,
    chat_name: &str,
    lang: &str,
    label: &str,
    options: batchalign::options::CommandOptions,
) -> (batchalign::api::JobInfo, Vec<String>) {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        panic!("{label}: direct FA session unavailable");
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixtures) = prepare_named_audio(jobs.state_dir(), audio_name, Some(chat_name)) else {
        panic!("{label}: named audio fixture unavailable");
    };

    let untimed_path = fixtures.stripped_chat.clone();
    std::fs::write(
        &untimed_path,
        untimed_chat_text(&std::fs::read_to_string(&fixtures.chat).expect("read timed chat")),
    )
    .expect("write untimed chat");

    let out_dir = jobs.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let output_path = out_dir.join(
        untimed_path
            .file_name()
            .expect("untimed path should have file name"),
    );

    jobs.submit_paths_job(
        ReleasedCommand::Align,
        lang,
        vec![untimed_path.to_string_lossy().into()],
        vec![output_path.to_string_lossy().into()],
        options,
    )
    .await
}

async fn align_untimed_audio_clip_with_options_detailed(
    audio_name: &str,
    chat_name: &str,
    lang: &str,
    label: &str,
    options: batchalign::options::CommandOptions,
) -> (batchalign::api::JobInfo, batchalign::store::JobDetail) {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        panic!("{label}: direct FA session unavailable");
    };

    let Some(fixtures) = prepare_named_audio(session.state_dir(), audio_name, Some(chat_name))
    else {
        panic!("{label}: named audio fixture unavailable");
    };

    let untimed_path = fixtures.stripped_chat.clone();
    std::fs::write(
        &untimed_path,
        untimed_chat_text(&std::fs::read_to_string(&fixtures.chat).expect("read timed chat")),
    )
    .expect("write untimed chat");

    let out_dir = session.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let output_path = out_dir.join(
        untimed_path
            .file_name()
            .expect("untimed path should have file name"),
    );

    session
        .run_submission(JobSubmission {
            command: ReleasedCommand::Align,
            lang: LanguageSpec::try_from(lang).expect("test lang should be valid"),
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options,
            paths_mode: true,
            source_paths: vec![untimed_path.to_string_lossy().as_ref().into()],
            output_paths: vec![output_path.to_string_lossy().as_ref().into()],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        })
        .await
}

async fn align_audio_clip_server_with_options(
    audio_name: &str,
    chat_name: &str,
    lang: &str,
    label: &str,
    options: batchalign::options::CommandOptions,
) {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixtures) = prepare_named_audio(server.state_dir(), audio_name, Some(chat_name))
    else {
        return;
    };

    let out_dir = server.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let input_basename = fixtures
        .stripped_chat
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let output_path = out_dir.join(&input_basename);

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            lang,
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![output_path.to_string_lossy().into()],
            options,
        )
        .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "{label}: server job should complete; error={:?}",
        info.error
    );
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], label);
}

async fn align_server_untimed_audio_clip_with_options(
    audio_name: &str,
    chat_name: &str,
    lang: &str,
    label: &str,
    options: batchalign::options::CommandOptions,
) -> (
    batchalign::api::JobInfo,
    batchalign::api::JobResultResponse,
    Vec<String>,
) {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        panic!("{label}: live server FA session unavailable");
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixtures) = prepare_named_audio(server.state_dir(), audio_name, Some(chat_name))
    else {
        panic!("{label}: named audio fixture unavailable");
    };

    let untimed_path = fixtures.stripped_chat.clone();
    std::fs::write(
        &untimed_path,
        untimed_chat_text(&std::fs::read_to_string(&fixtures.chat).expect("read timed chat")),
    )
    .expect("write untimed chat");

    let out_dir = server.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let output_path = out_dir.join(
        untimed_path
            .file_name()
            .expect("untimed path should have file name"),
    );

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            lang,
            vec![untimed_path.to_string_lossy().into()],
            vec![output_path.to_string_lossy().into()],
            options,
        )
        .await;
    let results = jobs.job_results(&info.job_id).await;
    (info, results, outputs)
}

#[tokio::test]
async fn align_server_spa_wav2vec() {
    align_audio_clip_server(
        "spa_marrero_clip",
        "spa_marrero_timed",
        "spa",
        "align_server_spa",
    )
    .await;
}

#[tokio::test]
async fn align_server_yue_wav2vec() {
    align_audio_clip_server_with_options(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_server_yue",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::Whisper,
        ),
    )
    .await;
}

#[tokio::test]
async fn align_yue_untimed_rev_utr_fails_cleanly() {
    let (info, detail) = align_untimed_audio_clip_with_options_detailed(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_yue_untimed_rev",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::RevAi,
        ),
    )
    .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Failed,
        "untimed yue align with explicit Rev UTR should fail cleanly"
    );
    let error = detail
        .results
        .first()
        .and_then(|result| result.error.clone())
        .or_else(|| info.error.clone())
        .unwrap_or_default();
    assert!(
        error.contains("requires utterance timing recovery"),
        "expected stage-aware UTR failure, got: {error}"
    );
}

#[tokio::test]
async fn align_yue_untimed_whisper_utr_succeeds() {
    let (info, outputs) = align_untimed_audio_clip_with_options(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_yue_untimed_whisper",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::Whisper,
        ),
    )
    .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "untimed yue align with explicit Whisper UTR should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_yue_untimed_whisper");
}

#[tokio::test]
async fn align_server_yue_untimed_rev_utr_fails_cleanly() {
    let (info, results, _outputs) = align_server_untimed_audio_clip_with_options(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_server_yue_untimed_rev",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::RevAi,
        ),
    )
    .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Failed,
        "server untimed yue align with explicit Rev UTR should fail cleanly"
    );
    let error = results
        .files
        .first()
        .and_then(|result| result.error.clone())
        .or_else(|| info.error.clone())
        .unwrap_or_default();
    assert!(
        error.contains("requires utterance timing recovery"),
        "expected stage-aware UTR failure, got: {error}"
    );
}

#[tokio::test]
async fn align_server_yue_untimed_whisper_utr_succeeds() {
    let (info, _results, outputs) = align_server_untimed_audio_clip_with_options(
        "yue_hku_clip",
        "yue_hku_timed",
        "yue",
        "align_server_yue_untimed_whisper",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::Whisper,
        ),
    )
    .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "server untimed yue align with explicit Whisper UTR should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_server_yue_untimed_whisper");
}

#[tokio::test]
async fn align_eng_untimed_rev_utr_succeeds_when_key_available() {
    if require_revai_key().is_none() {
        eprintln!("SKIP: REVAI_API_KEY / BATCHALIGN_REV_API_KEY not set");
        return;
    }

    let (info, outputs) = align_untimed_audio_clip_with_options(
        "eng_multi_speaker",
        "eng_multi_speaker",
        "eng",
        "align_eng_untimed_rev",
        align_options_with_utr(
            FaEngineName::Wave2Vec,
            WorTierPolicy::Include,
            UtrEngine::RevAi,
        ),
    )
    .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "untimed eng align with explicit Rev UTR should complete when credentials are available"
    );
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_eng_untimed_rev");
}
