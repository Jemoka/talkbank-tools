//! `UtSegTaskRunner` — splits blob ASR utterances into sub-utterances.
//!
//! Strategy: walk the validated CHAT in `BAValue::Chat`. For every utterance,
//! call the UtSeg backend with the utterance's spoken text wrapped as an
//! `AsrSegment`; the backend returns `UtteranceSpan`s (each its own
//! sub-utterance, carrying the BERT-predicted terminator). The runner then
//! rebuilds the document via the typed `talkbank_transform::build_chat`
//! constructor — no CHAT text is assembled by hand and nothing round-trips
//! through `to_chat()`/`parse` (building/inspecting CHAT as strings is
//! forbidden; see `CLAUDE.md`).
//!
//! Per spec2.md §5 / §8 and the BA2 `pipelines/utterance/` reference.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::{AsrSegment, AsrWord, LanguageSpec};
use crate::proto::utseg::{UtSegInput, UtSegOutput};
use crate::utils::SourceId;
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use std::collections::{BTreeSet, HashMap};
use talkbank_model::alignment::helpers::TierDomain;

/// UtSeg runner — `Task::UtSeg` entry point.
pub struct UtSegTaskRunner;

#[async_trait]
impl TaskRunner for UtSegTaskRunner {
    const TASK: Task = Task::UtSeg;

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
                    "UtSegTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };
        let source_id = chat.source_id().clone();
        sink.emit(ProgressEvent::stage_started(&source_id, Task::UtSeg));

        // Preserve any attached media + the file's languages across the rebuild.
        let media = chat.media().cloned();
        let existing_media_header = chat.ast().media.as_deref().cloned();
        let media_name = existing_media_header
            .as_ref()
            .map(|header| header.filename.to_string())
            .or_else(|| {
                media
                    .as_ref()
                    .map(|media| media.path.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| source_id.as_str().to_string());
        let media_type = existing_media_header
            .as_ref()
            .map(|header| header.media_type.as_str().to_string());
        let langs: Vec<String> = chat
            .ast()
            .languages
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();

        // Per-utterance text (typed extraction via `walk_words`) → dispatch →
        // collect the re-segmented sub-utterances.
        let rows: Vec<UtteranceRow> = collect_utterance_rows(chat);
        let total = rows.len() as u64;
        let mut new_utts: Vec<NewUtterance> = Vec::new();
        let mut assignment_map: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut requires_rebuild = false;
        for (idx, row) in rows.iter().enumerate() {
            let input = UtSegInput {
                source_id: source_id.clone(),
                segments: vec![row.as_segment()],
                // Language is per-file from `@Languages:`; the backend pins its
                // own model at construction.
                language: LanguageSpec::PerFile,
                stanza_fallback: false,
            };
            let out_raw = dispatcher.dispatch(TaskInput::UtSeg(input.clone())).await?;
            let out: UtSegOutput = out_raw.try_into()?;
            match assignments_from_spans(row, &out) {
                Some(assignments) if !assignments.is_empty() => {
                    assignment_map.insert(idx, assignments);
                }
                Some(_) => {}
                None => requires_rebuild = true,
            }
            collect_split(row, &input, &out, &mut new_utts);
            sink.emit(ProgressEvent::stage_tick(
                &source_id,
                Task::UtSeg,
                (idx + 1) as u64,
                total,
            ));
        }

        if requires_rebuild {
            // Backends that change tokenization (rather than only assigning
            // existing words to utterances) still need the legacy rebuild.
            // Cantonese word segmentation is the primary example.
            let mut new_chat = build_chat_from_utterances(
                &source_id,
                &langs,
                &media_name,
                media_type.as_deref(),
                &new_utts,
            )?;
            if let Some(m) = media {
                new_chat = new_chat.with_media(m);
            }
            *value = BAValue::Chat(new_chat);
        } else {
            // Boundary-only backends return the original words partitioned
            // into spans. Apply those assignments to the existing typed AST
            // so case, CHAT structure, dependent tiers, and the parent's
            // utterance bullet are preserved exactly. The shared transform
            // deliberately attaches that bullet to the final child only.
            talkbank_transform::utseg::apply_utseg_results(chat.ast_mut(), &assignment_map);
        }
        sink.emit(ProgressEvent::stage_injected(&source_id, Task::UtSeg));
        Ok(())
    }
}

/// One utterance's pre-split metadata: speaker + spoken text + media window.
#[derive(Debug, Clone)]
struct UtteranceRow {
    speaker: String,
    text: String,
    start_ms: u64,
    end_ms: u64,
    words: Vec<AsrWord>,
}

impl UtteranceRow {
    fn as_segment(&self) -> AsrSegment {
        AsrSegment {
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            text: self.text.clone(),
            speaker: None,
            words: self.words.clone(),
        }
    }
}

/// A sub-utterance to emit after segmentation: speaker, its CHAT content
/// (including the BERT-predicted terminator), and an optional media window.
#[derive(Debug, Clone)]
struct NewUtterance {
    speaker: String,
    text: String,
    bullet: Option<(u64, u64)>,
    words: Vec<AsrWord>,
    debug: UtSegDebug,
}

/// Debug-only provenance for an utterance produced from one UtSeg model call.
#[derive(Debug, Clone)]
struct UtSegDebug {
    source_utterance: String,
    model_input: UtSegInput,
    model_output: UtSegOutput,
}

/// Pull `(speaker, spoken text, media window)` for each utterance in document
/// order, reading the typed AST directly (no `to_chat()` round-trip).
fn collect_utterance_rows(chat: &Chat) -> Vec<UtteranceRow> {
    chat.ast()
        .utterances()
        .map(|u| {
            let speaker = u.main.speaker.as_str().to_string();
            let wor_timings: Vec<Option<(u64, u64)>> = u
                .wor_tier()
                .map(|tier| tier.words().map(word_timing).collect())
                .unwrap_or_default();
            let mut extracted = Vec::new();
            talkbank_transform::extract::collect_utterance_content(
                &u.main.content.content,
                TierDomain::Mor,
                &mut extracted,
            );
            let words: Vec<AsrWord> = extracted
                .iter()
                .enumerate()
                .map(|(index, word)| {
                    asr_word_from_text(
                        word.text.as_str(),
                        wor_timings.get(index).and_then(|timing| *timing),
                    )
                })
                .collect();
            let (start_ms, end_ms) = u
                .main
                .content
                .bullet
                .as_ref()
                .map(|b| (b.timing.start_ms, b.timing.end_ms))
                .unwrap_or((0, 0));
            UtteranceRow {
                speaker,
                text: words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                start_ms,
                end_ms,
                words,
            }
        })
        .collect()
}

/// Recover the backend's typed group assignment from its word-preserving spans.
///
/// `Some(vec![])` means the backend returned no spans and the source utterance
/// should remain unchanged. `None` means the backend changed tokenization, so
/// the caller must use the legacy text rebuild path.
fn assignments_from_spans(row: &UtteranceRow, out: &UtSegOutput) -> Option<Vec<usize>> {
    if out.utterances.is_empty() {
        return Some(Vec::new());
    }

    let source_words: Vec<&str> = row.words.iter().map(|word| word.text.as_str()).collect();
    let output_words: Vec<&str> = out
        .utterances
        .iter()
        .flat_map(|span| span.words.iter().map(|word| word.text.as_str()))
        .collect();
    if source_words != output_words {
        return None;
    }

    Some(
        out.utterances
            .iter()
            .enumerate()
            .flat_map(|(group, span)| std::iter::repeat_n(group, span.words.len()))
            .collect(),
    )
}

fn word_timing(word: &talkbank_model::model::Word) -> Option<(u64, u64)> {
    word.inline_bullet
        .as_ref()
        .map(|b| (b.timing.start_ms, b.timing.end_ms))
}

fn asr_word_from_text(text: &str, timing: Option<(u64, u64)>) -> AsrWord {
    let (start_ms, end_ms) = timing.unwrap_or((0, 0));
    AsrWord {
        text: text.to_string(),
        start_ms,
        end_ms,
        confidence: None,
    }
}

/// Append the segmenter's sub-utterances for one row. Falls back to the
/// original (single) utterance when the backend returns no spans.
fn collect_split(
    row: &UtteranceRow,
    input: &UtSegInput,
    out: &UtSegOutput,
    sink: &mut Vec<NewUtterance>,
) {
    let debug = UtSegDebug {
        source_utterance: row.text.clone(),
        model_input: input.clone(),
        model_output: out.clone(),
    };
    if out.utterances.is_empty() {
        if !row.text.trim().is_empty() {
            // No split: keep the blob as one utterance with a default period.
            sink.push(NewUtterance {
                speaker: row.speaker.clone(),
                text: format!("{} .", row.text.trim()),
                bullet: None,
                words: row.words.clone(),
                debug,
            });
        }
        return;
    }
    for span in &out.utterances {
        let text = span.text.trim();
        if text.is_empty() {
            continue;
        }
        let bullet = if span.start_ms == 0 && span.end_ms == 0 {
            None
        } else {
            Some((span.start_ms, span.end_ms))
        };
        // The span text usually carries the BERT-predicted terminator
        // (`.`/`?`/`!`). When the model leaves the final fragment unterminated,
        // default to a period — BA2 terminates every utterance (and a CHAT main
        // tier requires a terminator).
        let text = if text.ends_with(['.', '?', '!']) {
            text.to_string()
        } else {
            format!("{text} .")
        };
        sink.push(NewUtterance {
            speaker: row.speaker.clone(),
            text,
            bullet,
            words: span.words.clone(),
            debug: debug.clone(),
        });
    }
}

/// Rebuild a typed CHAT document from the re-segmented utterances via the
/// official `build_chat` constructor (text mode parses each utterance's content
/// through tree-sitter, so disfluency/retrace markers round-trip typed).
fn build_chat_from_utterances(
    source_id: &SourceId,
    langs: &[String],
    media_name: &str,
    media_type: Option<&str>,
    utts: &[NewUtterance],
) -> BAResult<Chat> {
    use talkbank_model::ErrorCollector;
    use talkbank_transform::build_chat::{
        ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat,
    };

    // Participants: unique speaker codes in first-appearance order.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut codes: Vec<String> = Vec::new();
    for u in utts {
        if seen.insert(u.speaker.clone()) {
            codes.push(u.speaker.clone());
        }
    }
    if codes.is_empty() {
        codes.push("PAR1".to_string());
    }
    let participants = codes
        .iter()
        .map(|code| ParticipantDesc {
            id: code.clone(),
            name: None,
            role: "Participant".to_string(),
            corpus: "batchalign".to_string(),
        })
        .collect();

    let utterances = utts
        .iter()
        .map(|u| UtteranceDesc {
            speaker: u.speaker.clone(),
            words: None,
            text: Some(u.text.clone()),
            start_ms: u.bullet.map(|b| b.0),
            end_ms: u.bullet.map(|b| b.1),
            lang: None,
        })
        .collect();

    // Preserve @Media on the rebuilt CHAT — UtSeg fires AFTER ASR.
    // Without forwarding it here, the rebuilt CHAT loses the @Media line
    // and downstream BA2 align refuses to consume our output.
    let desc = TranscriptDescription {
        langs: if langs.is_empty() {
            vec!["eng".to_string()]
        } else {
            langs.to_vec()
        },
        participants,
        media_name: Some(media_name.to_string()),
        media_type: media_type.map(str::to_string),
        utterances,
        write_wor: false,
    };

    let chat_file = build_chat(&desc).map_err(|e| {
        for (idx, utt) in utts.iter().enumerate() {
            let single = TranscriptDescription {
                langs: if langs.is_empty() {
                    vec!["eng".to_string()]
                } else {
                    langs.to_vec()
                },
                participants: vec![ParticipantDesc {
                    id: utt.speaker.clone(),
                    name: None,
                    role: "Participant".to_string(),
                    corpus: "batchalign".to_string(),
                }],
                media_name: Some(media_name.to_string()),
                media_type: media_type.map(str::to_string),
                utterances: vec![UtteranceDesc {
                    speaker: utt.speaker.clone(),
                    words: None,
                    text: Some(utt.text.clone()),
                    start_ms: utt.bullet.map(|b| b.0),
                    end_ms: utt.bullet.map(|b| b.1),
                    lang: None,
                }],
                write_wor: false,
            };
            if build_chat(&single).is_err() {
                let model_input = serde_json::to_string_pretty(&utt.debug.model_input)
                    .unwrap_or_else(|_| format!("{:#?}", utt.debug.model_input));
                let model_output = serde_json::to_string_pretty(&utt.debug.model_output)
                    .unwrap_or_else(|_| format!("{:#?}", utt.debug.model_output));
                tracing::debug!(
                    target: "batchalign::utseg",
                    "UtSeg re-parse failure\nsource_id: {}\nutterance_index: {}\nspeaker: {}\nrebuilt_utterance: {}\nsource_utterance: {}\nmodel_input:\n{}\nmodel_output:\n{}\nerror: {}",
                    source_id.as_str(),
                    idx,
                    utt.speaker,
                    utt.text,
                    utt.debug.source_utterance,
                    model_input,
                    model_output,
                    e,
                );
                break;
            }
        }
        BAError::Internal(format!("build_chat: {e}"))
    })?;
    let collector = ErrorCollector::new();
    let validated = chat_file.validate_into(&collector, None);
    let mut chat = Chat::from_validated_ast(validated, source_id.clone());
    inject_word_timings(&mut chat, utts)?;
    Ok(chat)
}

fn inject_word_timings(chat: &mut Chat, utts: &[NewUtterance]) -> BAResult<()> {
    use talkbank_model::DependentTier;
    use talkbank_model::model::{Bullet, WorTier, Word};

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let talkbank_model::Line::Utterance(u) = line else {
            continue;
        };
        let Some(src) = utts.get(idx) else {
            break;
        };
        if src.words.iter().any(|w| w.end_ms > w.start_ms) {
            let words = src
                .words
                .iter()
                .map(|w| {
                    let word = Word::simple(w.text.as_str());
                    if w.end_ms > w.start_ms {
                        word.with_inline_bullet(Bullet::new(w.start_ms, w.end_ms))
                    } else {
                        word
                    }
                })
                .collect();
            let wor = WorTier::from_words(words).with_terminator(u.main.content.terminator.clone());
            u.dependent_tiers
                .retain(|t| !matches!(t, DependentTier::Wor(_)));
            u.dependent_tiers.push(DependentTier::Wor(wor));
        }
        idx += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::base::TaskOutput;
    use crate::proto::utseg::UtteranceSpan;
    use std::sync::Mutex;

    struct ScriptedDispatcher {
        outputs: Mutex<Vec<UtSegOutput>>,
        assert_input_words: bool,
    }

    #[async_trait]
    impl Dispatcher for ScriptedDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            match input {
                TaskInput::UtSeg(input) => {
                    if self.assert_input_words {
                        let words = &input.segments[0].words;
                        assert_eq!(words.len(), 4, "UtSegInput should carry word timings");
                        assert_eq!((words[1].start_ms, words[1].end_ms), (100, 200));
                    }
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
            assert_input_words: false,
            outputs: Mutex::new(vec![UtSegOutput {
                source_id: sid.clone(),
                utterances: vec![
                    // Spans carry the BERT-predicted terminator, as in production.
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "hello there .".into(),
                        words: vec![],
                    },
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "general kenobi .".into(),
                        words: vec![],
                    },
                ],
            }]),
        };
        UtSegTaskRunner
            .apply(
                &mut value,
                &disp,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
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

    #[tokio::test]
    async fn typed_split_preserves_case_and_puts_parent_bullet_on_last_child() {
        const CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR1 Participant\n@ID:\teng|batchalign|PAR1|||||Participant|||\n\
@Media:\tsession, audio\n*PAR1:\t&-uh Yeah that's mine . \u{15}10_500\u{15}\n@End\n";

        let sid = SourceId::try_new("session").expect("sid");
        let chat = Chat::parse(CHAT, sid.clone()).expect("parse typed CHAT");
        let mut value = BAValue::Chat(chat);
        let disp = ScriptedDispatcher {
            assert_input_words: false,
            outputs: Mutex::new(vec![UtSegOutput {
                source_id: sid,
                utterances: vec![
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "Yeah".into(),
                        words: vec![AsrWord {
                            text: "Yeah".into(),
                            start_ms: 0,
                            end_ms: 0,
                            confidence: None,
                        }],
                    },
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 0,
                        text: "that's mine".into(),
                        words: vec![
                            AsrWord {
                                text: "that's".into(),
                                start_ms: 0,
                                end_ms: 0,
                                confidence: None,
                            },
                            AsrWord {
                                text: "mine".into(),
                                start_ms: 0,
                                end_ms: 0,
                                confidence: None,
                            },
                        ],
                    },
                ],
            }]),
        };

        UtSegTaskRunner
            .apply(
                &mut value,
                &disp,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("apply");
        let BAValue::Chat(chat) = value else {
            panic!("expected chat");
        };
        let output = chat.to_chat();
        let main_lines: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with("*PAR1:"))
            .collect();
        assert_eq!(
            main_lines,
            [
                "*PAR1:\t&-uh Yeah .",
                "*PAR1:\tthat's mine . \u{15}10_500\u{15}"
            ]
        );
    }

    #[tokio::test]
    async fn preserves_video_media_type_through_rebuild() {
        const VIDEO_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR1 Participant\n@ID:\teng|batchalign|PAR1|||||Participant|||\n\
@Media:\tsession, video\n*PAR1:\thello there . \u{15}1_100\u{15}\n@End\n";

        let sid = SourceId::try_new("session").expect("sid");
        let chat = Chat::parse(VIDEO_CHAT, sid.clone()).expect("parse video CHAT");
        let mut value = BAValue::Chat(chat);
        let disp = ScriptedDispatcher {
            assert_input_words: false,
            outputs: Mutex::new(vec![UtSegOutput {
                source_id: sid,
                utterances: vec![UtteranceSpan {
                    start_ms: 1,
                    end_ms: 100,
                    text: "hello there .".into(),
                    words: vec![],
                }],
            }]),
        };

        UtSegTaskRunner
            .apply(
                &mut value,
                &disp,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("apply");
        let BAValue::Chat(chat) = value else {
            panic!("expected chat");
        };
        assert!(
            chat.to_chat().contains("@Media:\tsession, video"),
            "UtSeg must preserve the typed video @Media header"
        );
    }

    const TIMED_BLOB_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR1 Participant\n@ID:\teng|batchalign|PAR1|||||Participant|||\n\
*PAR1:\thello there general kenobi .\n\
%wor:\thello \u{15}0_100\u{15} there \u{15}100_200\u{15} general \u{15}200_300\u{15} kenobi \u{15}300_400\u{15} .\n\
@End\n";

    #[tokio::test]
    async fn preserves_word_timings_through_split() {
        let sid = SourceId::try_new("ut").expect("sid");
        let chat = Chat::parse(TIMED_BLOB_CHAT, sid.clone()).expect("parse timed blob");
        let mut value = BAValue::Chat(chat);
        let disp = ScriptedDispatcher {
            assert_input_words: true,
            outputs: Mutex::new(vec![UtSegOutput {
                source_id: sid.clone(),
                utterances: vec![
                    UtteranceSpan {
                        start_ms: 0,
                        end_ms: 200,
                        text: "hello there .".into(),
                        words: vec![
                            AsrWord {
                                text: "hello".into(),
                                start_ms: 0,
                                end_ms: 100,
                                confidence: None,
                            },
                            AsrWord {
                                text: "there".into(),
                                start_ms: 100,
                                end_ms: 200,
                                confidence: None,
                            },
                        ],
                    },
                    UtteranceSpan {
                        start_ms: 200,
                        end_ms: 400,
                        text: "general kenobi .".into(),
                        words: vec![
                            AsrWord {
                                text: "general".into(),
                                start_ms: 200,
                                end_ms: 300,
                                confidence: None,
                            },
                            AsrWord {
                                text: "kenobi".into(),
                                start_ms: 300,
                                end_ms: 400,
                                confidence: None,
                            },
                        ],
                    },
                ],
            }]),
        };
        UtSegTaskRunner
            .apply(
                &mut value,
                &disp,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("apply");
        let BAValue::Chat(c) = value else {
            panic!("expected chat");
        };
        let text = c.to_chat();
        assert!(text.contains("%wor:"), "expected %wor tiers, got {text}");
        assert!(
            text.contains("\u{15}100_200\u{15}"),
            "expected fixed timing, got {text}"
        );
        assert!(
            text.contains("\u{15}300_400\u{15}"),
            "expected fixed timing, got {text}"
        );
    }
}
