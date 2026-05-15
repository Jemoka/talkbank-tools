use talkbank_model::model::{ChatFile, LanguageCode};

use crate::asr_postprocess;

use super::{ParticipantDesc, TranscriptDescription, UtteranceDesc, WordDesc};

/// Build a CHAT file from a JSON transcript description string.
///
/// This is the entry point used by the PyO3 bridge (`build_chat_inner`).
pub fn build_chat_from_json(json: &str) -> Result<ChatFile, String> {
    let desc: TranscriptDescription =
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {e}"))?;
    super::build_chat(&desc)
}

/// Domain errors from building a `TranscriptDescription`.
///
/// Exposes structured failure information — the offending word's
/// position, text, declared language, and the full
/// `Vec<talkbank_model::ParseError>` from `ChatWordText::try_from_lang`
/// — so upstream callers can render diagnostics or branch on failure
/// class without re-parsing a string.
#[derive(Debug, thiserror::Error)]
pub enum TranscriptBuildError {
    /// A word failed CHAT-legality validation under its utterance's
    /// language. Normalization upstream in `process_raw_asr` should
    /// have rewritten reporter-class tokens (`%`, digit-hyphen compounds)
    /// before this gate; any failure surfacing here is a residual case
    /// the normalizer hasn't been taught yet.
    #[error(
        "word {word_idx} ({word_text:?}) in utterance {utt_idx} \
         (speaker *{speaker_id}:, lang {lang}) failed CHAT validation: {}",
        parse_errors.iter()
            .map(|e| format!("[{}] {}", e.code.as_str(), e.message))
            .collect::<Vec<_>>()
            .join("; ")
    )]
    WordFailedValidation {
        /// Zero-based index of the utterance containing the bad word.
        utt_idx: usize,
        /// Zero-based index of the word within its utterance.
        word_idx: usize,
        /// Speaker code for the enclosing utterance (e.g. `"PAR0"`).
        speaker_id: String,
        /// Original ASR token text (before any attempted normalization).
        word_text: String,
        /// ISO 639-3 language code the word was validated under.
        lang: String,
        /// Structured parse/validation errors from
        /// [`ChatWordText::try_from_lang`].
        parse_errors: Vec<talkbank_model::ParseError>,
    },
}

/// Convert post-processed ASR utterances into a pre-serialization
/// `TranscriptDescription`.
///
/// Each word's text is validated at construction via
/// [`ChatWordText::try_from_lang`][try_lang] under the utterance's declared
/// language (falling back to the primary `langs[0]` or `"eng"`). Fails
/// with [`TranscriptBuildError`] at the first offending word. This is
/// the "loud guard" half of strategy 4c: normalization runs upstream
/// in `process_raw_asr`'s stages; this gate is the belt after the
/// braces.
///
/// [try_lang]: asr_postprocess::ChatWordText::try_from_lang
pub fn transcript_from_asr_utterances(
    utterances: &[asr_postprocess::Utterance],
    participant_ids: &[String],
    langs: &[String],
    media_name: Option<&str>,
    write_wor: bool,
) -> Result<TranscriptDescription, TranscriptBuildError> {
    if let Ok(path) = std::env::var("BA3_DUMP_UTTERANCES")
        && let Ok(json) = serde_json::to_string_pretty(utterances)
    {
        let _ = std::fs::write(&path, json);
        tracing::warn!(path = %path, "BA3_DUMP_UTTERANCES wrote post-processed utterances");
    }

    let participants = build_asr_participants(utterances, participant_ids);
    let primary_lang_code = LanguageCode::from(langs.first().map(String::as_str).unwrap_or("eng"));

    let mut utterance_descs = Vec::with_capacity(utterances.len());
    for (utt_idx, utterance) in utterances.iter().enumerate() {
        let speaker_id = resolve_speaker_id(utterance.speaker, participant_ids);
        let utterance_lang = utterance
            .lang
            .as_deref()
            .map(LanguageCode::from)
            .unwrap_or_else(|| primary_lang_code.clone());

        let words = utterance
            .words
            .iter()
            .enumerate()
            .map(|(word_idx, word)| {
                validate_asr_word(word, &speaker_id, &utterance_lang, utt_idx, word_idx)
            })
            .collect::<Result<Vec<_>, _>>()?;

        utterance_descs.push(UtteranceDesc {
            speaker: speaker_id,
            words: Some(words),
            text: None,
            start_ms: None,
            end_ms: None,
            lang: utterance.lang.clone(),
        });
    }

    Ok(TranscriptDescription {
        langs: if langs.is_empty() {
            vec!["eng".to_string()]
        } else {
            langs.to_vec()
        },
        participants,
        media_name: media_name.map(String::from),
        media_type: Some("audio".to_string()),
        utterances: utterance_descs,
        write_wor,
    })
}

fn build_asr_participants(
    utterances: &[asr_postprocess::Utterance],
    participant_ids: &[String],
) -> Vec<ParticipantDesc> {
    let mut seen_speakers: Vec<asr_postprocess::SpeakerIndex> = Vec::new();
    for utterance in utterances {
        if !seen_speakers.contains(&utterance.speaker) {
            seen_speakers.push(utterance.speaker);
        }
    }
    seen_speakers.sort_unstable();

    seen_speakers
        .iter()
        .map(|&speaker| {
            let id = resolve_speaker_id(speaker, participant_ids);
            let (_name, role) = role_for_speaker_code(&id);
            ParticipantDesc {
                id,
                name: None,
                role,
                corpus: String::new(),
            }
        })
        .collect()
}

fn resolve_speaker_id(
    speaker: asr_postprocess::SpeakerIndex,
    participant_ids: &[String],
) -> String {
    let index = speaker.as_usize();
    if index < participant_ids.len() {
        participant_ids[index].clone()
    } else {
        format!("SP{index}")
    }
}

fn validate_asr_word(
    word: &asr_postprocess::AsrWord,
    speaker_id: &str,
    utterance_lang: &LanguageCode,
    utt_idx: usize,
    word_idx: usize,
) -> Result<WordDesc, TranscriptBuildError> {
    let text =
        match asr_postprocess::ChatWordText::try_from_lang(word.text.as_str(), utterance_lang) {
            Ok(text) => text,
            Err(lang_errors) => fallback_or_fail_word(
                word,
                speaker_id,
                utterance_lang,
                utt_idx,
                word_idx,
                lang_errors,
            )?,
        };

    Ok(WordDesc {
        text,
        start_ms: word.start_ms.map(|ms| ms as u64),
        end_ms: word.end_ms.map(|ms| ms as u64),
        kind: word.kind,
    })
}

fn fallback_or_fail_word(
    word: &asr_postprocess::AsrWord,
    speaker_id: &str,
    utterance_lang: &LanguageCode,
    utt_idx: usize,
    word_idx: usize,
    lang_errors: Vec<talkbank_model::ParseError>,
) -> Result<asr_postprocess::ChatWordText, TranscriptBuildError> {
    match asr_postprocess::ChatWordText::try_from(word.text.as_str()) {
        Ok(structural) => {
            tracing::warn!(
                utt_idx,
                word_idx,
                speaker_id = %speaker_id,
                word_text = %word.text.as_str(),
                lang = %utterance_lang.as_str(),
                lang_errors = ?lang_errors,
                "ASR token fails language-level validation \
                 (structurally legal CHAT); emitting verbatim \
                 for downstream validator + CHECK to surface",
            );
            Ok(structural)
        }
        Err(parse_errors) => Err(TranscriptBuildError::WordFailedValidation {
            utt_idx,
            word_idx,
            speaker_id: speaker_id.to_owned(),
            word_text: word.text.as_str().to_owned(),
            lang: utterance_lang.as_str().to_owned(),
            parse_errors,
        }),
    }
}

fn role_for_speaker_code(code: &str) -> (String, String) {
    match code {
        "INV" => ("Investigator".into(), "Investigator".into()),
        "CHI" => ("Target_Child".into(), "Target_Child".into()),
        "MOT" => ("Mother".into(), "Mother".into()),
        "FAT" => ("Father".into(), "Father".into()),
        "EXP" => ("Experimenter".into(), "Experimenter".into()),
        "OBS" => ("Observer".into(), "Observer".into()),
        "TEA" => ("Teacher".into(), "Teacher".into()),
        _ => ("Participant".into(), "Participant".into()),
    }
}
