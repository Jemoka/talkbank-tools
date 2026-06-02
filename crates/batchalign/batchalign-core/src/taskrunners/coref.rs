//! `CorefTaskRunner` — adds a `%xcor:` coreference tier per utterance.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::coref::{CorefInput, CorefOutput};
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use talkbank_model::Line;
use talkbank_model::alignment::helpers::{WordItem, walk_words};

pub struct CorefTaskRunner;

#[async_trait]
impl TaskRunner for CorefTaskRunner {
    const TASK: Task = Task::Coref;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let chat = match value {
            BAValue::Chat(c) => c,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "CorefTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(chat.source_id(), Task::Coref));

        let (utterances, speakers) = collect_inputs(chat);
        if utterances.is_empty() {
            sink.emit(ProgressEvent::stage_injected(chat.source_id(), Task::Coref));
            return Ok(());
        }

        let input = CorefInput {
            source_id: chat.source_id().clone(),
            utterances,
            speakers,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Coref(input)).await?;
        let output: CorefOutput = output_raw.try_into()?;

        inject_coref_tiers(chat, &output.annotations, &*sink)?;

        sink.emit(ProgressEvent::stage_injected(chat.source_id(), Task::Coref));
        Ok(())
    }
}

fn collect_inputs(chat: &Chat) -> (Vec<String>, Vec<String>) {
    let mut utts = Vec::new();
    let mut spks = Vec::new();
    for line in chat.ast().lines.0.iter() {
        let Line::Utterance(u) = line else { continue };
        let mut parts: Vec<String> = Vec::new();
        walk_words(&u.main.content.content.0, None, &mut |w| {
            if let Some(t) = word_text(&w) {
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        });
        utts.push(parts.join(" "));
        spks.push(u.main.speaker.as_str().to_string());
    }
    (utts, spks)
}

fn inject_coref_tiers(
    chat: &mut Chat,
    annotations: &[String],
    sink: &dyn ProgressSink,
) -> BAResult<()> {
    use talkbank_model::model::dependent_tier::UserDefinedDependentTier;
    use talkbank_model::{DependentTier, NonEmptyString, Span};

    let source_id = chat.source_id().clone();
    let total = annotations.len() as u64;
    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(ann) = annotations.get(idx) else {
            return Err(BAError::Internal(format!(
                "Coref: missing annotation for utterance {idx}"
            )));
        };
        idx += 1;
        let trimmed = ann.trim();
        if !trimmed.is_empty() {
            let label = NonEmptyString::new("xcor")
                .ok_or_else(|| BAError::Internal("xcor label empty".into()))?;
            let content = NonEmptyString::new(trimmed)
                .ok_or_else(|| BAError::Internal("xcor content empty".into()))?;
            u.dependent_tiers
                .push(DependentTier::UserDefined(UserDefinedDependentTier {
                    label,
                    content,
                    span: Span::DUMMY,
                }));
        }
        sink.emit(ProgressEvent::stage_tick(
            &source_id,
            Task::Coref,
            idx as u64,
            total,
        ));
    }
    if idx != annotations.len() {
        return Err(BAError::Internal(format!(
            "Coref: utterance/output count mismatch ({idx} vs {})",
            annotations.len()
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
