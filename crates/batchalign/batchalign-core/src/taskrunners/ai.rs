//! `AiTaskRunner` — applies generic AI-proposed raw CHAT utterance edits.

use crate::base::BAValue;
use crate::base::BackendProgress;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::ai::{AiInput, AiOutput, AiUtterance};
use crate::utils::{BAError, BAResult, SourceId};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use talkbank_model::Line;
use talkbank_model::WriteChat;

/// Generic AI edit runner.
#[derive(Clone, Debug, Default)]
pub struct AiTaskRunner;

#[async_trait]
impl TaskRunner for AiTaskRunner {
    const TASK: Task = Task::Ai;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let (instruction, chat) = match value {
            BAValue::Ai { instruction, chat } => (instruction.clone(), chat),
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "AiTaskRunner: expected BAValue::Ai, got {}",
                    other.kind()
                )));
            }
        };

        let source_id = chat.source_id().clone();
        sink.emit(ProgressEvent::stage_started(&source_id, Task::Ai));

        let media = chat.media().cloned();
        let lines = chat.ast().lines.0.clone();
        let utterance_indices = utterance_line_indices(&lines);
        let utterances = utterance_indices
            .iter()
            .enumerate()
            .filter_map(|(idx, &line_idx)| {
                let Line::Utterance(utterance) = &lines[line_idx] else {
                    return None;
                };
                Some(AiUtterance {
                    index: idx as u32,
                    chat: utterance.to_chat(),
                    context: context_for(&lines, &utterance_indices, idx),
                })
            })
            .collect();
        let input = AiInput {
            source_id: source_id.clone(),
            instruction,
            utterances,
        };
        let progress_total = input.utterances.len() as u64;

        for utterance in &input.utterances {
            let context = utterance.context.join("\n---\n");
            tracing::debug!(
                target: "batchalign::ai",
                "AI model input\nsource_id: {}\nutterance_index: {}\nutterance_total: {}\ninstruction:\n{}\ncontext:\n{}\ncurrent:\n{}",
                source_id.as_str(),
                utterance.index,
                progress_total,
                input.instruction,
                context,
                utterance.chat,
            );
        }

        if progress_total > 0 {
            sink.emit(ProgressEvent::stage_tick(
                &source_id,
                Task::Ai,
                0,
                progress_total,
            ));
        }
        let progress = Arc::new(AiBackendProgress {
            sink: sink.clone(),
            source_id: source_id.clone(),
        });
        let progress_dyn: Arc<dyn BackendProgress> = progress;
        let output_raw = dispatcher
            .dispatch_with_progress(TaskInput::Ai(input), progress_dyn)
            .await?;
        if progress_total > 0 {
            sink.emit(ProgressEvent::stage_tick(
                &source_id,
                Task::Ai,
                progress_total,
                progress_total,
            ));
        }
        let output: AiOutput = output_raw.try_into()?;
        let revised_indices: BTreeSet<u32> = output
            .revisions
            .iter()
            .map(|revision| revision.index)
            .collect();
        for revision in &output.revisions {
            tracing::debug!(
                target: "batchalign::ai",
                "AI model output\nsource_id: {}\nutterance_index: {}\nrevised:\n{}",
                source_id.as_str(),
                revision.index,
                revision.chat,
            );
        }
        for idx in 0..progress_total {
            if !revised_indices.contains(&(idx as u32)) {
                tracing::debug!(
                    target: "batchalign::ai",
                    "AI model output\nsource_id: {}\nutterance_index: {}\nrevised:\n<no revision>",
                    source_id.as_str(),
                    idx,
                );
            }
        }
        let mut new_chat = apply_revisions(&source_id, lines, utterance_indices, &output)?;
        if let Some(m) = media {
            new_chat = new_chat.with_media(m);
        }
        *value = BAValue::Chat(new_chat);

        sink.emit(ProgressEvent::stage_injected(&source_id, Task::Ai));
        Ok(())
    }
}

struct AiBackendProgress {
    sink: Arc<dyn ProgressSink>,
    source_id: SourceId,
}

impl BackendProgress for AiBackendProgress {
    fn tick(&self, completed: u64, total: u64) {
        self.sink.emit(ProgressEvent::stage_tick(
            &self.source_id,
            Task::Ai,
            completed,
            total,
        ));
    }
}

fn utterance_line_indices(lines: &[Line]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| matches!(line, Line::Utterance(_)).then_some(idx))
        .collect()
}

fn context_for(lines: &[Line], utterance_indices: &[usize], idx: usize) -> Vec<String> {
    let mut out = Vec::new();
    if idx > 0 {
        if let Line::Utterance(utterance) = &lines[utterance_indices[idx - 1]] {
            out.push(utterance.to_chat());
        }
    }
    if idx + 1 < utterance_indices.len() {
        if let Line::Utterance(utterance) = &lines[utterance_indices[idx + 1]] {
            out.push(utterance.to_chat());
        }
    }
    out
}

fn apply_revisions(
    source_id: &SourceId,
    mut lines: Vec<Line>,
    mut utterance_indices: Vec<usize>,
    output: &AiOutput,
) -> BAResult<Chat> {
    let mut revisions: BTreeMap<usize, String> = BTreeMap::new();
    for revision in &output.revisions {
        let idx = revision.index as usize;
        if idx >= utterance_indices.len() {
            return Err(BAError::Worker(format!(
                "AiOutput revision index {idx} out of range"
            )));
        }
        if revisions
            .insert(idx, ensure_trailing_newline(&revision.chat))
            .is_some()
        {
            return Err(BAError::Worker(format!(
                "AiOutput has duplicate revision for index {idx}"
            )));
        }
    }

    let mut accepted = parse_lines(source_id, &lines)?;
    for (utterance_idx, replacement) in revisions.into_iter().rev() {
        let line_idx = utterance_indices[utterance_idx];
        match parse_lines_with_replacement(source_id, &lines, line_idx, &replacement) {
            Ok(parsed) => {
                accepted = parsed;
                lines = accepted.ast().lines.0.clone();
                utterance_indices = utterance_line_indices(&lines);
            }
            Err(err) => {
                tracing::debug!(
                    target: "batchalign::ai",
                    "AI revision rejected: returned CHAT block did not validate\nsource_id: {}\nutterance_index: {}\nerror: {}",
                    source_id.as_str(),
                    utterance_idx,
                    err,
                );
                continue;
            }
        }
    }
    Ok(accepted)
}

fn parse_lines(source_id: &SourceId, lines: &[Line]) -> BAResult<Chat> {
    parse_lines_rendering(source_id, lines, None)
}

fn parse_lines_with_replacement(
    source_id: &SourceId,
    lines: &[Line],
    replacement_line_idx: usize,
    replacement: &str,
) -> BAResult<Chat> {
    parse_lines_rendering(source_id, lines, Some((replacement_line_idx, replacement)))
}

fn parse_lines_rendering(
    source_id: &SourceId,
    lines: &[Line],
    replacement: Option<(usize, &str)>,
) -> BAResult<Chat> {
    let text = lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            if let Some((line_idx, text)) = replacement
                && line_idx == idx
            {
                return ensure_trailing_newline(text);
            }
            match line {
                Line::Header { .. } => format!("{}\n", line.to_chat_string()),
                Line::Utterance(utterance) => utterance.to_chat(),
            }
        })
        .collect::<String>();
    Chat::parse(&text, source_id.clone())
}

fn ensure_trailing_newline(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('\n') {
        trimmed.to_string()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{NullSink, TaskOutput};
    use crate::proto::ai::AiRevision;
    use std::sync::Mutex;

    struct StubDispatcher {
        output: Mutex<Option<AiOutput>>,
        input: Mutex<Option<AiInput>>,
        tick_progress: bool,
    }

    #[async_trait]
    impl Dispatcher for StubDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            match input {
                TaskInput::Ai(ai) => {
                    *self.input.lock().expect("input lock") = Some(ai);
                    let out = self
                        .output
                        .lock()
                        .expect("output lock")
                        .take()
                        .expect("canned AI output");
                    Ok(TaskOutput::Ai(out))
                }
                _ => Err(BAError::Internal("unexpected dispatch".into())),
            }
        }

        async fn dispatch_with_progress(
            &self,
            input: TaskInput,
            progress: std::sync::Arc<dyn crate::base::BackendProgress>,
        ) -> BAResult<TaskOutput> {
            if self.tick_progress {
                progress.tick(1, 2);
            }
            self.dispatch(input).await
        }
    }

    fn chat_from_texts(texts: &[&str]) -> Chat {
        use talkbank_model::ErrorCollector;
        use talkbank_transform::build_chat::{
            ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat,
        };

        let sid = SourceId::try_new("test").expect("source id");
        let utterances = texts
            .iter()
            .enumerate()
            .map(|(idx, text)| UtteranceDesc {
                speaker: "PAR0".to_string(),
                words: None,
                text: Some(text.to_string()),
                start_ms: Some((idx as u64) * 1000),
                end_ms: Some(((idx as u64) + 1) * 1000),
                lang: None,
            })
            .collect();
        let desc = TranscriptDescription {
            langs: vec!["eng".to_string()],
            participants: vec![ParticipantDesc {
                id: "PAR0".to_string(),
                name: None,
                role: "Participant".to_string(),
                corpus: "batchalign".to_string(),
            }],
            media_name: Some("test".to_string()),
            media_type: Some("audio".to_string()),
            utterances,
            write_wor: false,
        };
        let ast = build_chat(&desc).expect("build chat");
        let collector = ErrorCollector::new();
        Chat::from_validated_ast(ast.validate_into(&collector, None), sid)
    }

    #[tokio::test]
    async fn runner_sends_raw_chat_and_applies_valid_revisions() {
        let sid = SourceId::try_new("test").expect("source id");
        let output = AiOutput {
            source_id: sid.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: "*PAR0:\thello there . \u{15}0_1000\u{15}\n".to_string(),
            }],
        };
        let dispatcher = StubDispatcher {
            output: Mutex::new(Some(output)),
            input: Mutex::new(None),
            tick_progress: false,
        };
        let mut value = BAValue::Ai {
            instruction: "fix utterances".to_string(),
            chat: chat_from_texts(&["hello .", "goodbye ."]),
        };

        AiTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("ai apply");

        let sent = dispatcher
            .input
            .lock()
            .expect("input lock")
            .clone()
            .expect("captured input");
        assert_eq!(sent.instruction, "fix utterances");
        assert_eq!(sent.utterances.len(), 2);
        assert!(sent.utterances[0].chat.starts_with("*PAR0:\thello ."));
        assert_eq!(sent.utterances[0].context.len(), 1);
        assert!(sent.utterances[0].context[0].starts_with("*PAR0:\tgoodbye ."));

        let BAValue::Chat(chat) = value else {
            panic!("expected chat");
        };
        let text = chat.to_chat();
        assert!(text.contains("*PAR0:\thello there ."), "{text}");
        assert!(text.contains("*PAR0:\tgoodbye ."), "{text}");
    }

    #[test]
    fn invalid_revision_is_ignored() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = chat_from_texts(&["hello .", "goodbye ."]);
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: "not chat".to_string(),
            }],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(text.contains("*PAR0:\thello ."), "{text}");
        assert!(text.contains("*PAR0:\tgoodbye ."), "{text}");
    }

    #[test]
    fn parsed_utterance_revision_is_replaced_as_typed_chat() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = chat_from_texts(&["hello ?", "goodbye ."]);
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: "*PAR0:\tHello ! \u{15}0_1000\u{15}\n".to_string(),
            }],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(text.contains("*PAR0:\tHello !"), "{text}");
        assert!(!text.contains("*PAR0:\thello ?"), "{text}");
    }

    #[test]
    fn revision_can_split_one_source_utterance_into_many() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = chat_from_texts(&["hello there .", "goodbye ."]);
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: concat!(
                    "*PAR0:\thello . \u{15}0_500\u{15}\n",
                    "*PAR0:\tthere . \u{15}500_1000\u{15}\n",
                )
                .to_string(),
            }],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(text.contains("*PAR0:\thello ."), "{text}");
        assert!(text.contains("*PAR0:\tthere ."), "{text}");
        assert!(text.contains("*PAR0:\tgoodbye ."), "{text}");
        assert_eq!(text.matches("*PAR0:").count(), 3, "{text}");
    }

    #[test]
    fn revision_can_split_untimed_source_without_inventing_bullets() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = Chat::parse(
            concat!(
                "@UTF8\n",
                "@Begin\n",
                "@Languages:\teng\n",
                "@Participants:\tLENO Participant\n",
                "@ID:\teng|batchalign|LENO|||||Participant|||\n",
                "*LENO:\tI never saw my dad put the belt on I only saw him take it off .\n",
                "@End\n",
            ),
            source_id.clone(),
        )
        .expect("chat");
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: concat!(
                    "*LENO:\tI never saw my dad put the belt on .\n",
                    "*LENO:\tI only saw him take it off .\n",
                )
                .to_string(),
            }],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(
            text.contains("*LENO:\tI never saw my dad put the belt on ."),
            "{text}"
        );
        assert!(
            text.contains("*LENO:\tI only saw him take it off ."),
            "{text}"
        );
        assert!(!text.contains("\u{15}"), "{text}");
        assert!(!text.contains("120_240"), "{text}");
        assert_eq!(text.matches("*LENO:").count(), 2, "{text}");
    }

    #[test]
    fn split_revision_does_not_shift_later_original_indices() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = chat_from_texts(&["alpha beta .", "gamma ."]);
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![
                AiRevision {
                    index: 0,
                    chat: concat!(
                        "*PAR0:\talpha . \u{15}0_500\u{15}\n",
                        "*PAR0:\tbeta . \u{15}500_1000\u{15}\n",
                    )
                    .to_string(),
                },
                AiRevision {
                    index: 1,
                    chat: "*PAR0:\tGAMMA . \u{15}1000_2000\u{15}\n".to_string(),
                },
            ],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(text.contains("*PAR0:\talpha ."), "{text}");
        assert!(text.contains("*PAR0:\tbeta ."), "{text}");
        assert!(text.contains("*PAR0:\tGAMMA ."), "{text}");
        assert!(!text.contains("*PAR0:\tgamma ."), "{text}");
        assert_eq!(text.matches("*PAR0:").count(), 3, "{text}");
    }

    #[test]
    fn parsed_utterance_that_breaks_full_chat_validation_is_ignored() {
        let source_id = SourceId::try_new("test").expect("source id");
        let chat = chat_from_texts(&["hello .", "goodbye ."]);
        let lines = chat.ast().lines.0.clone();
        let indices = utterance_line_indices(&lines);
        let output = AiOutput {
            source_id: source_id.clone(),
            revisions: vec![AiRevision {
                index: 0,
                chat: "*BOG:\tHello . \u{15}0_1000\u{15}\n".to_string(),
            }],
        };

        let parsed = apply_revisions(&source_id, lines, indices, &output).expect("parsed");
        let text = parsed.to_chat();
        assert!(text.contains("*PAR0:\thello ."), "{text}");
        assert!(!text.contains("*BOG:\tHello ."), "{text}");
    }

    struct CapturingSink {
        events: Mutex<Vec<ProgressEvent>>,
    }

    impl CapturingSink {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProgressSink for CapturingSink {
        fn emit(&self, event: ProgressEvent) {
            self.events.lock().expect("events lock").push(event);
        }
    }

    #[tokio::test]
    async fn backend_progress_ticks_are_forwarded() {
        let sid = SourceId::try_new("test").expect("source id");
        let output = AiOutput {
            source_id: sid,
            revisions: vec![],
        };
        let dispatcher = StubDispatcher {
            output: Mutex::new(Some(output)),
            input: Mutex::new(None),
            tick_progress: true,
        };
        let mut value = BAValue::Ai {
            instruction: "fix utterances".to_string(),
            chat: chat_from_texts(&["hello .", "goodbye ."]),
        };
        let sink = std::sync::Arc::new(CapturingSink::new());

        AiTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                sink.clone() as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("ai apply");

        let ticks: Vec<(u64, u64)> = sink
            .events
            .lock()
            .expect("events lock")
            .iter()
            .filter(|event| event.total > 0)
            .map(|event| (event.completed, event.total))
            .collect();
        assert_eq!(ticks, vec![(0, 2), (1, 2), (2, 2)]);
    }
}
