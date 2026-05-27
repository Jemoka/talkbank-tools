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
use crate::utils::{BAError, BAResult, SpeakerLabel, prepare_pcm};
use async_trait::async_trait;
use talkbank_model::Line;
use talkbank_model::alignment::helpers::{WordItem, walk_words};

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

        let media = chat
            .media()
            .cloned()
            .ok_or_else(|| BAError::Internal("FaTaskRunner: chat has no attached media".into()))?;

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
        out.push(AsrSegment {
            start_ms: 0,
            end_ms: 0,
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

fn inject_word_timings(chat: &mut Chat, aligned: &[AsrSegment]) -> BAResult<()> {
    use talkbank_model::model::dependent_tier::UserDefinedDependentTier;
    use talkbank_model::{DependentTier, NonEmptyString, Span};

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(seg) = aligned.get(idx) else {
            return Err(BAError::Internal(format!(
                "FA: missing aligned segment for utterance {idx}"
            )));
        };
        if !seg.words.is_empty() {
            let blob = seg
                .words
                .iter()
                .map(|w| format!("{} \u{15}{}_{}\u{15}", w.text, w.start_ms, w.end_ms))
                .collect::<Vec<_>>()
                .join(" ");
            let label = NonEmptyString::new("wor")
                .ok_or_else(|| BAError::Internal("wor label empty".into()))?;
            let content = NonEmptyString::new(&blob)
                .ok_or_else(|| BAError::Internal("wor content empty".into()))?;
            u.dependent_tiers
                .push(DependentTier::UserDefined(UserDefinedDependentTier {
                    label,
                    content,
                    span: Span::DUMMY,
                }));
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
