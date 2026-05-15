use crate::common::{
    LiveDirectJobClient, prepare_named_audio, require_live_direct, require_revai_key,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::chat_ops::TierDomain;
use batchalign::options::{
    AlignOptions, AsrEngineName, CommandOptions, CommonOptions, TranscribeOptions, WorTierPolicy,
};
use batchalign::worker::InferTask;
use talkbank_transform::extract::extract_words;
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

pub(crate) fn parse_output(chat: &str, label: &str) -> batchalign::chat_ops::ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    let (file, errors) = parse_lenient(&parser, chat);
    assert!(errors.is_empty(), "{label}: CHAT parse errors: {errors:?}");
    file
}

pub(crate) fn assert_all_utterances_timed(chat: &str, label: &str) {
    let file = parse_output(chat, label);
    let extracted = extract_words(&file, TierDomain::Mor);
    for (ext_utt, utt) in extracted.iter().zip(file.utterances()) {
        if ext_utt.words.is_empty() {
            continue;
        }
        assert!(
            utt.main.content.bullet.is_some(),
            "{label}: utterance by {} missing timing bullet",
            utt.main.speaker
        );
    }
}

pub(crate) fn count_wor_tiers(chat: &str) -> usize {
    let file = parse_output(chat, "count_wor_tiers");
    file.utterances()
        .filter(|utt| utt.wor_tier().is_some())
        .count()
}

pub(crate) fn assert_valid_chat_structure(chat: &str, label: &str) {
    let file = parse_output(chat, label);
    assert!(
        file.utterance_count() >= 1,
        "{label}: output should have at least 1 utterance"
    );
}

pub(crate) fn assert_min_utterances(chat: &str, label: &str, min_utterances: usize) {
    let file = parse_output(chat, label);
    assert!(
        file.utterance_count() >= min_utterances,
        "{label}: expected at least {min_utterances} utterances, got {}",
        file.utterance_count()
    );
}

pub(crate) fn assert_first_utterance_max_words(chat: &str, label: &str, max_words: usize) {
    let file = parse_output(chat, label);
    let first = file
        .utterances()
        .next()
        .unwrap_or_else(|| panic!("{label}: expected at least one utterance"));
    let words = extract_words(&file, TierDomain::Mor);
    let first_words = words
        .first()
        .unwrap_or_else(|| panic!("{label}: missing extracted words for first utterance"));
    assert!(
        first_words.words.len() <= max_words,
        "{label}: expected first utterance to have at most {max_words} words, got {}",
        first_words.words.len()
    );
    assert!(
        first.main.content.bullet.is_some(),
        "{label}: expected first utterance to carry a timing bullet"
    );
}

pub(crate) async fn transcribe_audio_clip(audio_name: &str, lang: &str, label: &str) {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixtures) = prepare_named_audio(jobs.state_dir(), audio_name, None) else {
        return;
    };

    let out_dir = jobs.state_dir().join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    let output_path = out_dir.join(format!("{audio_name}.cha"));

    let options = CommandOptions::Transcribe(TranscribeOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        asr_engine: AsrEngineName::Whisper,
        diarize: false,
        wor: WorTierPolicy::Omit,
        merge_abbrev: false.into(),
        batch_size: 8,
        utseg_fallback: false.into(),
    });

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            lang,
            vec![fixtures.audio.to_string_lossy().into()],
            vec![output_path.to_string_lossy().into()],
            options,
        )
        .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "{label}: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], label);
}

pub(crate) async fn align_audio_clip(audio_name: &str, chat_name: &str, lang: &str, label: &str) {
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

    let options = CommandOptions::Align(AlignOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        wor: WorTierPolicy::Include,
        ..AlignOptions::default()
    });

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
        JobStatus::Completed,
        "{label}: job should complete"
    );
    assert_eq!(outputs.len(), 1);

    let output = &outputs[0];
    assert_valid_chat_structure(output, label);
    assert_all_utterances_timed(output, label);
}

pub(crate) fn revai_available() -> bool {
    if require_revai_key().is_none() {
        eprintln!("SKIP: Rev.AI credentials not configured in env or ~/.batchalign.ini");
        return false;
    }
    true
}
