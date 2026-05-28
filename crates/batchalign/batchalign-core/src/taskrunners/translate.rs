//! `TranslateTaskRunner` — adds a `%xtra:` translation tier per utterance.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::asr::LanguageSpec;
use crate::proto::translate::{TranslateInput, TranslateOutput};
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use smol_str::SmolStr;
use talkbank_model::Line;
use talkbank_model::WriteChat;
use talkbank_model::alignment::helpers::{WordItem, walk_words};

pub struct TranslateTaskRunner;

#[async_trait]
impl TaskRunner for TranslateTaskRunner {
    const TASK: Task = Task::Translate;

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
                    "TranslateTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(
            chat.source_id(),
            Task::Translate,
        ));

        let utterances = utterance_texts(chat);
        if utterances.is_empty() {
            sink.emit(ProgressEvent::stage_injected(
                chat.source_id(),
                Task::Translate,
            ));
            return Ok(());
        }

        let input = TranslateInput {
            source_id: chat.source_id().clone(),
            utterances,
            // Source: resolve `@Languages:` to a concrete code so the
            // backend sees `LanguageSpec::Code("spa")` instead of a bare
            // `PerFile` marker. NLLB / Tencent TMT / Aliyun MT pick the
            // source-side tokenizer / API code from this. Target: the
            // backend pins it at construction (e.g.
            // `GoogleTranslateBackend(target="eng")`); the input field
            // stays as the conventional default so the proto shape
            // doesn't drift.
            source: resolve_per_file_language(chat),
            target: SmolStr::new_static("eng"),
        };

        let output_raw = dispatcher.dispatch(TaskInput::Translate(input)).await?;
        let output: TranslateOutput = output_raw.try_into()?;

        inject_translation_tiers(chat, &output.utterances)?;

        sink.emit(ProgressEvent::stage_injected(
            chat.source_id(),
            Task::Translate,
        ));
        Ok(())
    }
}

/// Read the chat's `@Languages:` header and emit a concrete `LanguageSpec`.
/// Falls back to `PerFile` (a no-op marker) when the header is absent so
/// the backend can do its own per-file resolution. Mirrors the
/// `morphosyntax.rs::resolve_per_file_language` helper — kept local rather
/// than shared because each task crate is meant to be reviewable on its own.
fn resolve_per_file_language(chat: &Chat) -> LanguageSpec {
    if let Some(code) = chat.primary_language() {
        LanguageSpec::Code(SmolStr::new(code))
    } else {
        LanguageSpec::PerFile
    }
}

fn utterance_texts(chat: &Chat) -> Vec<String> {
    let mut out = Vec::new();
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
        let mut text = parts.join(" ");
        // Include the utterance terminator (BA2 feeds the full sentence to the
        // translator, so it capitalizes + punctuates the output to match).
        if let Some(term) = &u.main.content.terminator {
            text.push(' ');
            text.push_str(&term.to_chat_string());
        }
        out.push(text);
    }
    out
}

fn inject_translation_tiers(chat: &mut Chat, translations: &[String]) -> BAResult<()> {
    use talkbank_model::model::dependent_tier::UserDefinedDependentTier;
    use talkbank_model::{DependentTier, NonEmptyString, Span};

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(text) = translations.get(idx) else {
            return Err(BAError::Internal(format!(
                "Translate: missing translation for utterance {idx}"
            )));
        };
        idx += 1;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let label = NonEmptyString::new("xtra")
            .ok_or_else(|| BAError::Internal("xtra label empty".into()))?;
        let content = NonEmptyString::new(trimmed)
            .ok_or_else(|| BAError::Internal("xtra content empty".into()))?;
        u.dependent_tiers
            .push(DependentTier::UserDefined(UserDefinedDependentTier {
                label,
                content,
                span: Span::DUMMY,
            }));
    }
    if idx != translations.len() {
        return Err(BAError::Internal(format!(
            "Translate: utterance/output count mismatch ({idx} vs {})",
            translations.len()
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
