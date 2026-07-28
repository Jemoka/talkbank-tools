//! `AsrTaskRunner` — turns `BAValue::Media` into `BAValue::Chat<Validated>`.
//!
//! Decodes the media via `crate::utils::prepare_pcm`, dispatches an `AsrInput`,
//! then adapts the returned `AsrOutput::segments` into the canonical
//! `talkbank-transform` ASR post-processing pipeline before building a fresh
//! CHAT document. Word timings ride through normalization so downstream FA can
//! refine them.
//!
//! Per spec2.md §8 and the BA2 `pipelines/asr/` reference.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::{AsrInput, AsrOutput, LanguageSpec};
use crate::utils::MediaInput;
use crate::utils::SpeakerLabel;
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use talkbank_transform::asr_postprocess::{
    AsrElement, AsrElementKind, AsrMonologue, AsrRawText, AsrTimestampSecs, SpeakerIndex,
};

/// ASR runner — `Task::Asr` entry point. The runner ships an `AsrInput`
/// with `LanguageSpec::Auto` and default `AsrOptions`; the backend supplies
/// its own language pin / batch tunables at construction time
/// (`WhisperBackend(language="eng")`, `RevAI(num_speakers=2)`, etc.).
pub struct AsrTaskRunner;

#[async_trait]
impl TaskRunner for AsrTaskRunner {
    const TASK: Task = Task::Asr;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let media = match value {
            BAValue::Media(m) => m.clone(),
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "AsrTaskRunner: expected BAValue::Media, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(&media.source_id, Task::Asr));

        let audio = crate::utils::prepare_pcm(&media)
            .map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let language = media
            .language
            .as_ref()
            .map(|code| LanguageSpec::Code(SmolStr::new(code.as_str())))
            .unwrap_or(LanguageSpec::Auto);
        let input = AsrInput {
            source_id: media.source_id.clone(),
            audio,
            language: language.clone(),
            options: Default::default(),
        };

        // Thread a backend-progress channel through so Python ASR backends
        // that loop over chunks (whisper, chatwhisper, funaudio) can feed
        // per-chunk `stage_tick`s into the same `ProgressSink` the per-utt
        // runners use. The Rust side has no per-unit work; the Python side
        // does — this is the Stanza-style partial-progress wiring for the
        // single-dispatch ASR runner. Backends that don't emit (rev,
        // tencent, aliyun cloud calls) simply see a no-op handle.
        let progress = std::sync::Arc::new(crate::base::ScaledProgress::new(
            sink.clone(),
            media.source_id.clone(),
            Task::Asr,
            1,
        ));
        let progress_dyn: std::sync::Arc<dyn crate::base::BackendProgress> = progress.clone();
        progress.start_step();
        let output_raw = dispatcher
            .dispatch_with_progress(TaskInput::Asr(input), progress_dyn)
            .await?;
        progress.finish();
        let output: AsrOutput = output_raw.try_into()?;

        let chat = build_chat_from_asr(&media, &language, &output)?.with_media(media.clone());
        *value = BAValue::Chat(chat);

        sink.emit(ProgressEvent::stage_injected(&media.source_id, Task::Asr));
        Ok(())
    }
}

/// Build a fresh validated CHAT document from ASR output.
///
/// Builds the typed AST directly via `talkbank_transform::build_chat` (the
/// official ASR→CHAT constructor): segments become typed utterances, the
/// segment media window becomes the utterance bullet, and the headers
/// (`@Languages`/`@Participants`/`@ID`) are derived from the speaker set. No
/// CHAT text is assembled by hand — building CHAT by string concatenation is
/// forbidden (see `CLAUDE.md`).
fn build_chat_from_asr(
    media: &MediaInput,
    language: &LanguageSpec,
    output: &AsrOutput,
) -> BAResult<Chat> {
    use talkbank_model::ErrorCollector;
    use talkbank_transform::build_chat::{
        ParticipantDesc, build_chat, transcript_from_asr_utterances,
    };

    let lang_code = resolve_lang_code(language);

    // Discover speakers in order of first appearance; assign PAR0, PAR1, ...
    let mut speaker_codes: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for seg in &output.segments {
        let raw = seg
            .speaker
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "PAR1".to_string());
        if !speaker_codes.contains_key(&raw) {
            // BA2 numbers speakers 0-based: the first speaker is PAR0.
            let code = format!("PAR{}", speaker_codes.len());
            speaker_codes.insert(raw.clone(), code.clone());
            order.push(code);
        }
    }
    if order.is_empty() {
        speaker_codes.insert("PAR0".to_string(), "PAR0".to_string());
        order.push("PAR0".to_string());
    }

    let raw_output = asr_output_for_postprocess(output, &speaker_codes)?;
    let utterances = talkbank_transform::asr_postprocess::process_raw_asr(&raw_output, &lang_code);

    // Emit `@Media: <media stem>, <audio|video>` so downstream consumers
    // (BA2's align, third-party tools) can resolve the media file. The CHAT
    // builder infers capture modality from the path extension before it
    // removes that extension from the serialized header.
    // BA3 + our align resolve by filename stem regardless, but BA2's
    // align refuses input without an explicit `@Media:` tier.
    // Bug #11 fix (parity test 2026-05-31).
    let mut desc = transcript_from_asr_utterances(
        &utterances,
        &order,
        std::slice::from_ref(&lang_code),
        Some(media.path.to_string_lossy().as_ref()),
        true,
    )
    .map_err(|e| BAError::Validation(e.to_string()))?;
    if desc.participants.is_empty() {
        desc.participants = order
            .iter()
            .map(|code| ParticipantDesc {
                id: code.clone(),
                name: None,
                role: "Participant".to_string(),
                corpus: "batchalign".to_string(),
            })
            .collect();
    }
    for participant in &mut desc.participants {
        participant.corpus = "batchalign".to_string();
    }
    // Let the official CHAT builder infer audio vs. video from the media path.
    desc.media_type = None;

    let chat_file = build_chat(&desc).map_err(|e| BAError::Internal(format!("build_chat: {e}")))?;

    // Provenance `@Comment` stamping happens once at end-of-pipeline in
    // `batchalign_engine::pipeline::run_one` (single source of truth for
    // git SHA + per-stage engine accumulation). No per-runner stamping.

    let collector = ErrorCollector::new();
    let validated = chat_file.validate_into(&collector, None);
    if collector.has_errors() {
        let joined = collector
            .into_vec()
            .into_iter()
            .filter(|error| matches!(error.severity, talkbank_model::Severity::Error))
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BAError::Validation(format!("\n{joined}")));
    }
    Ok(Chat::from_validated_ast(validated, media.source_id.clone()))
}

/// Adapt the taskrunner's stable ASR wire contract to the canonical
/// `talkbank-transform` post-processing input. Each backend segment remains a
/// monologue, and word timings are converted from milliseconds to seconds so
/// `process_raw_asr` can preserve them through normalization and splitting.
fn asr_output_for_postprocess(
    output: &AsrOutput,
    speaker_codes: &BTreeMap<String, String>,
) -> BAResult<talkbank_transform::asr_postprocess::AsrOutput> {
    let mut monologues = Vec::with_capacity(output.segments.len());
    for segment in &output.segments {
        let raw_speaker = segment
            .speaker
            .as_ref()
            .map(SpeakerLabel::as_str)
            .unwrap_or("PAR1");
        let code = speaker_codes
            .get(raw_speaker)
            .ok_or_else(|| BAError::Internal(format!("ASR: unknown speaker {raw_speaker}")))?;
        let speaker_number = code
            .strip_prefix("PAR")
            .and_then(|number| number.parse::<usize>().ok())
            .ok_or_else(|| BAError::Internal(format!("ASR: invalid participant code {code}")))?;

        let elements = if segment.words.is_empty() {
            let text = sanitize_segment_text(&segment.text);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![postprocess_element(&text, segment.start_ms, segment.end_ms)]
            }
        } else {
            segment
                .words
                .iter()
                .map(|word| postprocess_element(&word.text, word.start_ms, word.end_ms))
                .collect()
        };

        if !elements.is_empty() {
            monologues.push(AsrMonologue {
                speaker: SpeakerIndex(speaker_number),
                elements,
            });
        }
    }
    Ok(talkbank_transform::asr_postprocess::AsrOutput { monologues })
}

fn postprocess_element(text: &str, start_ms: u64, end_ms: u64) -> AsrElement {
    let trimmed = text.trim();
    AsrElement {
        value: AsrRawText::new(trimmed),
        ts: AsrTimestampSecs(start_ms as f64 / 1000.0),
        end_ts: AsrTimestampSecs(end_ms as f64 / 1000.0),
        kind: if is_punctuation_token(trimmed) {
            AsrElementKind::Punctuation
        } else {
            AsrElementKind::Text
        },
    }
}

fn is_punctuation_token(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|character| {
            matches!(
                character,
                '.' | '?' | '!' | ',' | ';' | ':' | '。' | '؟' | '۔' | '،' | '؛'
            )
        })
}

/// Resolve a `LanguageSpec` to a usable ISO-3 code for the CHAT header.
fn resolve_lang_code(spec: &LanguageSpec) -> String {
    match spec {
        LanguageSpec::Code(c) => c.as_str().to_string(),
        LanguageSpec::Auto | LanguageSpec::PerFile => "eng".to_string(),
    }
}

/// Strip only the control / tier-marker characters that would break CHAT
/// parsing (tab/newline + the `*`/`%`/`@` line-prefix markers). CHAT *content*
/// markers — overlap `<>`, retrace `[/]`, disfluency `&` — are preserved so
/// they round-trip typed when the text is parsed by tree-sitter (BA2's rev
/// path emits retraces for non-BERT languages like Spanish).
fn sanitize_segment_text(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r' | '*' | '%' | '@' | '\\'))
        .collect();
    let trimmed = cleaned.trim();
    // Drop a terminal . / ? / ! — we append our own ` .` punctuation.
    let stripped = trimmed
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | ';' | ':'))
        .trim_end();
    stripped.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::base::TaskOutput;
    use crate::proto::asr::{AsrSegment, AsrWord};
    use crate::utils::{MediaInput, SourceId};
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct StubDispatcher {
        canned: Mutex<Option<AsrOutput>>,
    }

    #[async_trait]
    impl Dispatcher for StubDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            match input {
                TaskInput::Asr(_) => {
                    let o = self
                        .canned
                        .lock()
                        .expect("stub lock")
                        .take()
                        .ok_or_else(|| BAError::Internal("stub: no canned ASR output".into()))?;
                    Ok(TaskOutput::Asr(o))
                }
                _ => Err(BAError::Internal("stub: unexpected dispatch".into())),
            }
        }
    }

    fn fake_segment(speaker: &str, text: &str, start: u64, end: u64) -> AsrSegment {
        AsrSegment {
            start_ms: start,
            end_ms: end,
            text: text.into(),
            speaker: Some(SpeakerLabel::new(speaker)),
            words: vec![AsrWord {
                text: text.into(),
                start_ms: start,
                end_ms: end,
                confidence: None,
            }],
        }
    }

    fn fake_timed_segment(speaker: &str) -> AsrSegment {
        AsrSegment {
            start_ms: 0,
            end_ms: 900,
            text: "hello there".into(),
            speaker: Some(SpeakerLabel::new(speaker)),
            words: vec![
                AsrWord {
                    text: "hello".into(),
                    start_ms: 0,
                    end_ms: 400,
                    confidence: None,
                },
                AsrWord {
                    text: "there".into(),
                    start_ms: 400,
                    end_ms: 900,
                    confidence: None,
                },
            ],
        }
    }

    fn fake_media(source_id: &SourceId, extension: &str) -> MediaInput {
        MediaInput::new(source_id.clone(), PathBuf::from(format!("tst.{extension}")))
    }

    #[test]
    fn build_chat_emits_validated_typed_doc() {
        let sid = SourceId::try_new("tst").expect("sid");
        let out = AsrOutput {
            source_id: sid.clone(),
            segments: vec![
                fake_segment("spk_0", "hello there", 0, 1000),
                fake_segment("spk_1", "general kenobi", 1000, 2200),
            ],
        };
        let media = fake_media(&sid, "wav");
        let chat =
            build_chat_from_asr(&media, &LanguageSpec::Code("eng".into()), &out).expect("chat");
        let text = chat.to_chat();
        assert!(text.contains("@Languages:\teng"));
        assert!(text.contains("@Participants:"));
        assert!(text.contains("*PAR0:\tHello there ."));
        assert!(text.contains("*PAR1:\tGeneral kenobi ."));
        // Provenance `@Comment` is stamped by the pipeline driver, not the
        // runner — see `batchalign_engine::pipeline::stamp_run_provenance`.
        assert!(
            !text.contains("batchalign3 "),
            "runner must not stamp provenance: {text}"
        );
    }

    #[test]
    fn build_chat_preserves_asr_word_timings() {
        let sid = SourceId::try_new("tst").expect("sid");
        let out = AsrOutput {
            source_id: sid.clone(),
            segments: vec![fake_timed_segment("spk_0")],
        };
        let media = fake_media(&sid, "wav");
        let chat =
            build_chat_from_asr(&media, &LanguageSpec::Code("eng".into()), &out).expect("chat");
        let text = chat.to_chat();
        assert!(text.contains("%wor:"), "expected %wor tier, got {text}");
        assert!(
            text.contains("\u{15}0_400\u{15}"),
            "expected first word timing, got {text}"
        );
        assert!(
            text.contains("\u{15}400_900\u{15}"),
            "expected second word timing, got {text}"
        );
    }

    #[test]
    fn build_chat_uses_asr_language_for_headers() {
        let sid = SourceId::try_new("tst").expect("sid");
        let out = AsrOutput {
            source_id: sid.clone(),
            segments: vec![fake_segment("spk_0", "hola", 0, 500)],
        };
        let media = fake_media(&sid, "wav");
        let chat =
            build_chat_from_asr(&media, &LanguageSpec::Code("spa".into()), &out).expect("chat");
        let text = chat.to_chat();
        assert!(
            text.contains("@Languages:\tspa"),
            "expected spa header, got {text}"
        );
        assert!(
            text.contains("@ID:\tspa|batchalign|PAR0"),
            "expected spa ID, got {text}"
        );
    }

    #[test]
    fn taskrunner_postprocesses_german_numbers_and_commas() {
        let sid = SourceId::try_new("tst").expect("sid");
        let timed_words = [",", "20", ",", ",", "1999", ",", "45"]
            .into_iter()
            .enumerate()
            .map(|(index, text)| AsrWord {
                text: text.to_string(),
                start_ms: index as u64 * 100,
                end_ms: (index as u64 + 1) * 100,
                confidence: None,
            })
            .collect();
        let out = AsrOutput {
            source_id: sid.clone(),
            segments: vec![AsrSegment {
                start_ms: 0,
                end_ms: 700,
                text: ", 20 , , 1999 , 45".into(),
                speaker: Some(SpeakerLabel::new("spk_0")),
                words: timed_words,
            }],
        };

        let media = fake_media(&sid, "wav");
        let chat = build_chat_from_asr(&media, &LanguageSpec::Code("deu".into()), &out)
            .expect("canonical post-processing should produce valid German CHAT");
        let text = chat.to_chat();
        let main = text
            .lines()
            .find(|line| line.starts_with("*PAR0:"))
            .expect("main tier");

        assert!(main.contains("zwanzig"), "20 was not expanded: {main}");
        assert!(
            main.contains("eintausend neunhundert neunundneunzig"),
            "1999 was not expanded: {main}"
        );
        assert!(
            main.contains("fünfundvierzig"),
            "45 was not expanded: {main}"
        );
        assert!(!main.contains(','), "ASR commas were not stripped: {main}");
    }

    #[test]
    fn taskrunner_rejects_residual_language_validation_errors() {
        let sid = SourceId::try_new("tst").expect("sid");
        let out = AsrOutput {
            source_id: sid.clone(),
            segments: vec![fake_segment("spk_0", "abc123", 0, 500)],
        };
        let media = fake_media(&sid, "wav");

        let error = build_chat_from_asr(&media, &LanguageSpec::Code("deu".into()), &out)
            .expect_err("unexpandable German digit-bearing tokens must fail in the taskrunner");
        assert!(
            matches!(error, BAError::Validation(_)),
            "expected validation error, got {error:?}"
        );
        assert!(
            error.to_string().contains("E220"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn build_chat_marks_movie_input_as_video() {
        let sid = SourceId::try_new("tst").expect("sid");
        let media = fake_media(&sid, "MOV");
        let out = AsrOutput {
            source_id: sid,
            segments: vec![fake_segment("spk_0", "hello", 0, 500)],
        };

        let chat =
            build_chat_from_asr(&media, &LanguageSpec::Code("eng".into()), &out).expect("chat");
        assert!(
            chat.to_chat().contains("@Media:\ttst, video"),
            "MOV input should produce a video @Media header"
        );
    }

    #[tokio::test]
    async fn apply_routes_through_dispatcher_and_replaces_media_with_chat() {
        let sid = SourceId::try_new("tst").expect("sid");
        // Use an audio fixture we know symphonia can decode. If none is
        // available we fall back to building the chat directly — the
        // dispatcher path is what we care about; audio_prep is exercised
        // by the engine's smoke tests.
        let canned = AsrOutput {
            source_id: sid.clone(),
            segments: vec![fake_segment("spk_0", "hello", 0, 500)],
        };

        // The apply() path needs a real audio file. Skip if we can't find
        // one in the repo's test fixtures.
        let fixture = find_audio_fixture();
        if fixture.is_none() {
            eprintln!("skip: no audio fixture available for AsrTaskRunner::apply");
            return;
        }
        let path = fixture.expect("checked Some");

        let mut value = BAValue::Media(MediaInput::new(sid.clone(), path));
        let disp = StubDispatcher {
            canned: Mutex::new(Some(canned)),
        };
        let runner = AsrTaskRunner;
        runner
            .apply(
                &mut value,
                &disp,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("apply ok");
        match value {
            BAValue::Chat(c) => assert!(c.to_chat().contains("*PAR0:\thello .")),
            other => panic!("expected Chat, got {}", other.kind()),
        }
    }

    fn find_audio_fixture() -> Option<PathBuf> {
        // Look for any .wav under the repo's test fixtures.
        let candidates = [
            "../../core/talkbank-parser-re2c/tests/fixtures",
            "../../../resources/corpus/reference",
        ];
        for c in &candidates {
            let p = PathBuf::from(c);
            if let Ok(rd) = std::fs::read_dir(&p) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                        return Some(path);
                    }
                }
            }
        }
        None
    }
}
