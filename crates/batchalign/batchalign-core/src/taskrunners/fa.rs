//! `FaTaskRunner` — refines word timings on an existing CHAT against its audio.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::asr::{AsrSegment, AsrWord, LanguageSpec};
use crate::proto::fa::{FaInput, FaOutput};
use crate::utils::{BAError, BAResult, MediaInput, SourceId, SpeakerLabel, prepare_pcm};
use async_trait::async_trait;
use std::path::Path;
use talkbank_model::Line;
use talkbank_model::alignment::helpers::{WordItem, walk_words};

/// Audio container extensions to probe for a transcript's sibling media,
/// in priority order (BA2/ffmpeg accept all of these).
const SIBLING_AUDIO_EXTS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "aac", "wma", "mov", "avi", "mpg", "mpeg",
];

/// Locate an audio file sitting next to a transcript whose `source_id` is its
/// absolute path. The CLI loads `.cha` files by path without scanning for
/// media siblings (and the engine's loader is frozen), so the audio task
/// resolves them here — the same sibling-audio resolution BA2 does at load.
fn sibling_media(source_id: &SourceId) -> Option<MediaInput> {
    let cha_path = Path::new(source_id.as_str());
    for ext in SIBLING_AUDIO_EXTS {
        let candidate = cha_path.with_extension(ext);
        if candidate.is_file() {
            return Some(MediaInput::new(source_id.clone(), candidate));
        }
    }
    None
}

pub struct FaTaskRunner;

#[async_trait]
impl TaskRunner for FaTaskRunner {
    const TASK: Task = Task::Fa;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        let chat = match value {
            BAValue::Chat(c) => c,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "FaTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        let media = match chat.media().cloned() {
            Some(m) => m,
            // No media attached at load — resolve the transcript's sibling
            // audio (its `source_id` is the absolute `.cha` path).
            None => sibling_media(chat.source_id()).ok_or_else(|| {
                BAError::Internal(
                    "FaTaskRunner: chat has no attached media and no sibling audio file found"
                        .into(),
                )
            })?,
        };

        sink.emit(ProgressEvent::stage_started(chat.source_id(), Task::Fa));

        let audio =
            prepare_pcm(&media).map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let utterances = extract_utterances_for_fa(chat);

        let input = FaInput {
            source_id: chat.source_id().clone(),
            audio,
            utterances,
            // FA reads `@Languages:` off the chat anyway; backends that
            // need a hard-pinned language pull it from their constructor.
            language: LanguageSpec::PerFile,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Fa(input)).await?;
        let output: FaOutput = output_raw.try_into()?;

        inject_word_timings(chat, &output.utterances)?;

        sink.emit(ProgressEvent::stage_injected(chat.source_id(), Task::Fa));
        Ok(())
    }
}

fn extract_utterances_for_fa(chat: &Chat) -> Vec<AsrSegment> {
    let mut out = Vec::new();
    for line in chat.ast().lines.0.iter() {
        let Line::Utterance(u) = line else { continue };
        let mut words = Vec::new();
        walk_words(&u.main.content.content.0, None, &mut |w| {
            if let Some(text) = word_text(&w) {
                if !text.is_empty() {
                    words.push(AsrWord {
                        text: text.to_string(),
                        start_ms: 0,
                        end_ms: 0,
                        confidence: None,
                    });
                }
            }
        });
        let speaker = Some(SpeakerLabel::new(u.main.speaker.as_str()));
        // The FA backend slices audio by each utterance's media-bullet window;
        // carry the existing utterance bullet (e.g. rev's `225_2405`) through.
        let (start_ms, end_ms) = u
            .main
            .content
            .bullet
            .as_ref()
            .map(|b| (b.timing.start_ms, b.timing.end_ms))
            .unwrap_or((0, 0));
        out.push(AsrSegment {
            start_ms,
            end_ms,
            text: words
                .iter()
                .map(|w| w.text.clone())
                .collect::<Vec<_>>()
                .join(" "),
            speaker,
            words,
        });
    }
    out
}

/// Attach a typed `%wor` tier per utterance from the aligned word timings.
///
/// Builds the tier with the official model types — each aligned word becomes a
/// `Word` carrying an `inline_bullet` (`\x15start_end\x15` media-time mark) —
/// and lets the CHAT writer serialize it. No `%wor` text is assembled by hand;
/// building CHAT by string concatenation is forbidden (see `CLAUDE.md`).
fn inject_word_timings(chat: &mut Chat, aligned: &[AsrSegment]) -> BAResult<()> {
    use talkbank_model::DependentTier;
    use talkbank_model::model::{Bullet, WorTier, Word};

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(seg) = aligned.get(idx) else {
            return Err(BAError::Internal(format!(
                "FA: missing aligned segment for utterance {idx}"
            )));
        };
        if !seg.words.is_empty() {
            let words: Vec<Word> = seg
                .words
                .iter()
                .map(|w| {
                    Word::simple(w.text.as_str())
                        .with_inline_bullet(Bullet::new(w.start_ms, w.end_ms))
                })
                .collect();
            // Carry the utterance's own terminator onto `%wor` (BA2 parity);
            // the typed writer renders the bullets and the terminator.
            let wor = WorTier::from_words(words).with_terminator(u.main.content.terminator.clone());
            u.dependent_tiers.push(DependentTier::Wor(wor));
            // BA2 refines the main-tier utterance bullet to span the aligned
            // words (first word start … last word end).
            u.main.content.bullet = Some(Bullet::new(seg.start_ms, seg.end_ms));
        }
        idx += 1;
    }
    if idx != aligned.len() {
        return Err(BAError::Internal(format!(
            "FA: utterance/output count mismatch ({idx} vs {})",
            aligned.len()
        )));
    }
    Ok(())
}

fn word_text<'a>(w: &WordItem<'a>) -> Option<&'a str> {
    match w {
        WordItem::Word(word) => Some(word.cleaned_text()),
        WordItem::ReplacedWord(r) => Some(r.word.cleaned_text()),
        WordItem::Separator(_) => None,
    }
}
