//! `SpeakerTaskRunner` — assigns utterances to speakers via diarization.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::speaker::{SpeakerInput, SpeakerOutput};
use crate::utils::{BAError, BAResult, prepare_pcm};
use async_trait::async_trait;
use talkbank_model::Line;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::content::UtteranceContent;

pub struct SpeakerTaskRunner;

#[async_trait]
impl TaskRunner for SpeakerTaskRunner {
    const TASK: Task = Task::Speaker;

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
                    "SpeakerTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        let media = chat.media().cloned().ok_or_else(|| {
            BAError::Internal("SpeakerTaskRunner: chat has no attached media".into())
        })?;

        sink.emit(ProgressEvent::stage_started(
            chat.source_id(),
            Task::Speaker,
        ));

        let audio =
            prepare_pcm(&media).map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let input = SpeakerInput {
            source_id: chat.source_id().clone(),
            audio,
            // Default 0 = "let the backend decide". The backend's
            // constructor (`PyannoteBackend(num_speakers=3)`) is where the
            // user pins a specific count.
            num_speakers: 0,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Speaker(input)).await?;
        let output: SpeakerOutput = output_raw.try_into()?;

        relabel_utterances_by_diarization(chat, &output, sink)?;

        sink.emit(ProgressEvent::stage_injected(
            chat.source_id(),
            Task::Speaker,
        ));
        Ok(())
    }
}

fn relabel_utterances_by_diarization(
    chat: &mut Chat,
    out: &SpeakerOutput,
    sink: &dyn ProgressSink,
) -> BAResult<()> {
    use talkbank_model::SpeakerCode;
    let segs = &out.diarization.segments;
    if segs.is_empty() {
        return Ok(());
    }
    let source_id = chat.source_id().clone();
    let total = chat
        .ast()
        .lines
        .0
        .iter()
        .filter(|l| matches!(l, Line::Utterance(_)))
        .count() as u64;
    let mut completed: u64 = 0;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        if let Some(mid) = utterance_midpoint_ms(&u.main.content.content.0) {
            let best = segs
                .iter()
                .find(|s| mid >= s.start_ms && mid <= s.end_ms)
                .or_else(|| {
                    segs.iter().min_by_key(|s| {
                        let lo = s.start_ms.abs_diff(mid);
                        let hi = s.end_ms.abs_diff(mid);
                        lo.min(hi)
                    })
                });
            if let Some(seg) = best {
                u.main.speaker = SpeakerCode::new(canonical_speaker_code(seg.speaker.as_str()));
            }
        }
        completed += 1;
        sink.emit(ProgressEvent::stage_tick(
            &source_id,
            Task::Speaker,
            completed,
            total,
        ));
    }
    Ok(())
}

fn utterance_midpoint_ms(content: &[UtteranceContent]) -> Option<u64> {
    let mut t0: Option<u64> = None;
    let mut t1: Option<u64> = None;
    walk_words(content, None, &mut |w| {
        if let Some((s, e)) = word_timing(&w) {
            t0 = Some(t0.map_or(s, |c| c.min(s)));
            t1 = Some(t1.map_or(e, |c| c.max(e)));
        }
    });
    match (t0, t1) {
        (Some(a), Some(b)) if b >= a => Some(a + (b - a) / 2),
        _ => None,
    }
}

fn word_timing(w: &WordItem<'_>) -> Option<(u64, u64)> {
    let word = match w {
        WordItem::Word(w) => *w,
        WordItem::ReplacedWord(r) => &r.word,
        WordItem::Separator(_) => return None,
    };
    let b = word.inline_bullet.as_ref()?;
    Some((b.timing.start_ms, b.timing.end_ms))
}

fn canonical_speaker_code(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "PAR".into();
    }
    let mut out = String::new();
    for c in trimmed.chars().take(8) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        }
    }
    if out.is_empty() { "PAR".into() } else { out }
}
