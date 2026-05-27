//! `UtSegTaskRunner` — splits blob ASR utterances into sub-utterances.
//!
//! Strategy: walk the validated CHAT in `BAValue::Chat`. For every utterance,
//! call the UtSeg backend with the utterance's spoken text wrapped as an
//! `AsrSegment`; the backend returns `UtteranceSpan`s that define how to
//! cut the blob. Rewrite the CHAT by emitting one main-tier line per span
//! and reparse via `Chat::parse` to keep the typestate invariants.
//!
//! Per spec2.md §5 / §8 and the BA2 `pipelines/utterance/` reference.

use crate::base::Chat;
use crate::utils::{BAError, BAResult};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::{AsrSegment, LanguageSpec};
use crate::proto::utseg::{UtSegInput, UtSegOutput};
use crate::base::Task;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::TaskInput;
use crate::base::{BAValue};
use crate::utils::SourceId;
use async_trait::async_trait;

/// UtSeg runner — `Task::UtSeg` entry point.
pub struct UtSegTaskRunner;

#[async_trait]
impl TaskRunner for UtSegTaskRunner {
    const TASK: Task = Task::UtSeg;

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
                    "UtSegTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };
        let source_id = chat.source_id().clone();
        sink.emit(ProgressEvent::stage_started(&source_id, Task::UtSeg));

        // Collect per-utterance "rows" to dispatch.
        let rows: Vec<UtteranceRow> = collect_utterance_rows(chat);

        let text_before = chat.to_chat();
        let mut new_text = String::with_capacity(text_before.len());

        let mut row_iter = rows.into_iter();
        for line in text_before.lines() {
            if is_main_tier_line(line) {
                // Pair with the corresponding row.
                let Some(row) = row_iter.next() else {
                    new_text.push_str(line);
                    new_text.push('\n');
                    continue;
                };
                let input = UtSegInput {
                    source_id: source_id.clone(),
                    segments: vec![row.as_segment()],
                    // Language: per-file from `@Languages:`. Stanza fallback:
                    // off here; if you want it, wire it on the backend
                    // (`StanzaUtSegBackend(...)` or similar).
                    language: LanguageSpec::PerFile,
                    stanza_fallback: false,
                };
                let out_raw = dispatcher.dispatch(TaskInput::UtSeg(input)).await?;
                let out: UtSegOutput = out_raw.try_into()?;
                emit_split_lines(&row, &out, &mut new_text);
            } else {
                new_text.push_str(line);
                new_text.push('\n');
            }
        }

        let new_chat = Chat::parse(&new_text, source_id.clone())?;
        *value = BAValue::Chat(new_chat);
        sink.emit(ProgressEvent::stage_injected(&source_id, Task::UtSeg));
        Ok(())
    }
}

/// One utterance's pre-split metadata, used to build the UtSeg request and
/// later reformat the emitted lines.
#[derive(Debug, Clone)]
struct UtteranceRow {
    speaker: String,
    text: String,
    start_ms: u64,
    end_ms: u64,
}

impl UtteranceRow {
    fn as_segment(&self) -> AsrSegment {
        AsrSegment {
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            text: self.text.clone(),
            speaker: None,
            words: Vec::new(),
        }
    }
}

/// Pull `(speaker, text, span)` for each utterance in document order.
fn collect_utterance_rows(chat: &Chat) -> Vec<UtteranceRow> {
    chat.ast()
        .utterances()
        .map(|u| {
            let speaker = u.main.speaker.to_string();
            // `to_chat()` round-trips the tier content faithfully.
            let line = u.to_chat();
            let text = extract_text_after_speaker(&line);
            UtteranceRow {
                speaker,
                text,
                start_ms: 0,
                end_ms: 0,
            }
        })
        .collect()
}

fn extract_text_after_speaker(line: &str) -> String {
    // Line shape: `*SPK:\t<content>\n`. Strip header, terminator, and any
    // trailing bullet so UtSeg sees pure text.
    let body = match line.find('\t') {
        Some(i) => &line[i + 1..],
        None => line,
    };
    let body = body.trim_end_matches('\n');
    // Strip trailing bullet `\u{15}..\u{15}`.
    let no_bullet = if let Some(i) = body.find('\u{15}') {
        body[..i].trim_end()
    } else {
        body
    };
    no_bullet
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?'))
        .trim()
        .to_string()
}

fn is_main_tier_line(line: &str) -> bool {
    line.starts_with('*') && line.contains(':')
}

fn emit_split_lines(row: &UtteranceRow, out: &UtSegOutput, sink: &mut String) {
    if out.utterances.is_empty() {
        // No split — fall back to a single line preserving original text.
        sink.push_str(&format!("*{}:\t{} .\n", row.speaker, row.text));
        return;
    }
    for span in &out.utterances {
        // Preserve the terminator the segmenter chose (`.`/`?`/`!`) so
        // questions/exclamations survive — matches BA2, whose utterance model
        // predicts sentence-final punctuation. Default to `.`.
        let terminator = terminator_of(&span.text);
        let text = sanitize_text(&span.text);
        if text.is_empty() {
            continue;
        }
        let bullet = if span.start_ms == 0 && span.end_ms == 0 {
            String::new()
        } else {
            format!(" \u{15}{}_{}\u{15}", span.start_ms, span.end_ms)
        };
        sink.push_str(&format!("*{}:\t{} {}{}\n", row.speaker, text, terminator, bullet));
    }
}

/// The sentence-final terminator the segmenter chose, as a CHAT token.
/// Reads the last non-space char; `?`/`!` pass through, everything else
/// (including `.`) maps to `.`.
fn terminator_of(s: &str) -> &'static str {
    match s.trim_end().chars().last() {
        Some('?') => "?",
        Some('!') => "!",
        _ => ".",
    }
}

fn sanitize_text(s: &str) -> String {
    // Keep CHAT content markers (`< > [ ] /`) so retrace (`[/]`, `<a b>`)
    // added by the cleanup stage survives; strip only line-structural chars.
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r' | '*' | '%' | '@' | '\\'))
        .collect();
    cleaned
        .trim()
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | ',' | ';' | ':'))
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::proto::utseg::UtteranceSpan;
    use crate::base::TaskOutput;
    use std::sync::Mutex;

    struct ScriptedDispatcher {
        outputs: Mutex<Vec<UtSegOutput>>,
    }

    #[async_trait]
    impl Dispatcher for ScriptedDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            match input {
                TaskInput::UtSeg(_) => {
                    let mut q = self.outputs.lock().expect("lock");
                    if q.is_empty() {
                        return Err(BAError::Internal("scripted: drained".into()));
                    }
                    Ok(TaskOutput::UtSeg(q.remove(0)))
                }
                _ => Err(BAError::Internal("scripted: unexpected".into())),
            }
        }
    }

    const BLOB_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR1 Participant\n@ID:\teng|batchalign|PAR1|||||Participant|||\n*PAR1:\thello there general kenobi .\n@End\n";

    #[tokio::test]
    async fn splits_blob_utterance_into_two() {
        let sid = SourceId::try_new("ut").expect("sid");
        let chat = Chat::parse(BLOB_CHAT, sid.clone()).expect("parse blob");
        let mut value = BAValue::Chat(chat);
        let disp = ScriptedDispatcher {
            outputs: Mutex::new(vec![UtSegOutput {
                source_id: sid.clone(),
                utterances: vec![
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "hello there".into(),
                        words: vec![],
                    },
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "general kenobi".into(),
                        words: vec![],
                    },
                ],
            }]),
        };
        UtSegTaskRunner
            .apply(&mut value, &disp, &NullSink)
            .await
            .expect("apply");
        let BAValue::Chat(c) = value else {
            panic!("expected chat");
        };
        let text = c.to_chat();
        let main_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("*PAR1:")).collect();
        assert_eq!(main_lines.len(), 2, "expected 2 utterances, got {text}");
        assert!(main_lines[0].contains("hello there"));
        assert!(main_lines[1].contains("general kenobi"));
    }
}
