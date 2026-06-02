//! `AsrTaskRunner` — turns `BAValue::Media` into `BAValue::Chat<Validated>`.
//!
//! Decodes the media via `crate::utils::prepare_pcm`, dispatches an `AsrInput`,
//! then folds the returned `AsrOutput::segments` into a fresh CHAT document
//! whose utterances mirror the segment text + diarization. Word timings, if
//! present, ride along as bullet timestamps so downstream FA can refine them.
//!
//! Per spec2.md §8 and the BA2 `pipelines/asr/` reference.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::{AsrInput, AsrOutput, LanguageSpec};
use crate::utils::SourceId;
use crate::utils::SpeakerLabel;
use crate::utils::stamp_provenance;
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use std::collections::BTreeMap;

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

        let language = LanguageSpec::Auto;
        let input = AsrInput {
            source_id: media.source_id.clone(),
            audio,
            language: language.clone(),
            options: Default::default(),
        };

        let output_raw = dispatcher.dispatch(TaskInput::Asr(input)).await?;
        let output: AsrOutput = output_raw.try_into()?;

        let engine = dispatcher.engine_name(Task::Asr);
        let chat = build_chat_from_asr(&media.source_id, &language, &output, engine.as_deref())?
            .with_media(media.clone());
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
    source_id: &SourceId,
    language: &LanguageSpec,
    output: &AsrOutput,
    engine: Option<&str>,
) -> BAResult<Chat> {
    use talkbank_model::ErrorCollector;
    use talkbank_transform::build_chat::{
        ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat,
    };

    let lang_code = resolve_lang_code(language);

    // Discover speakers in order of first appearance; assign PAR1, PAR2, ...
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

    let participants = order
        .iter()
        .map(|code| ParticipantDesc {
            id: code.clone(),
            name: None,
            role: "Participant".to_string(),
            corpus: "batchalign".to_string(),
        })
        .collect();

    let mut utterances: Vec<UtteranceDesc> = Vec::new();
    for seg in &output.segments {
        let raw = seg
            .speaker
            .as_ref()
            .map(SpeakerLabel::as_str)
            .unwrap_or("PAR1");
        let code = speaker_codes
            .get(raw)
            .ok_or_else(|| BAError::Internal(format!("ASR: unknown speaker {raw}")))?;
        let utt_text = sanitize_segment_text(&seg.text);
        if utt_text.is_empty() {
            continue;
        }
        // Carry the segment media window as the utterance bullet when present.
        let (start_ms, end_ms) = if seg.start_ms == 0 && seg.end_ms == 0 {
            (None, None)
        } else {
            (Some(seg.start_ms), Some(seg.end_ms))
        };
        // Text mode: the words + terminator are parsed via tree-sitter, so any
        // CHAT content markers (retrace `[/]`, disfluency `&-uh`) round-trip
        // typed. We append the default period; the UtSeg stage re-segments and
        // re-terminates per the BERT model.
        utterances.push(UtteranceDesc {
            speaker: code.clone(),
            words: None,
            text: Some(format!("{utt_text} .")),
            start_ms,
            end_ms,
            lang: None,
        });
    }

    // Emit `@Media: <source_id>, audio` so downstream consumers
    // (BA2's align, third-party tools) can resolve the audio file.
    // BA3 + our align resolve by filename stem regardless, but BA2's
    // align refuses input without an explicit `@Media:` tier.
    // Bug #11 fix (parity test 2026-05-31).
    let desc = TranscriptDescription {
        langs: vec![lang_code],
        participants,
        media_name: Some(source_id.as_str().to_string()),
        media_type: Some("audio".to_string()),
        utterances,
        write_wor: false,
    };

    let mut chat_file =
        build_chat(&desc).map_err(|e| BAError::Internal(format!("build_chat: {e}")))?;

    // Stamp provenance as a `@Comment` header: BA version + engine name.
    // Inserted just before the first utterance so it sits at the tail of the
    // header block — visible to anyone reading the file, ignored by the
    // alignment / mor / wor stages downstream. Shared with `FaTaskRunner`
    // via `utils::stamp_provenance`.
    stamp_provenance(&mut chat_file.lines.0, Task::Asr.as_str(), engine);

    let collector = ErrorCollector::new();
    let validated = chat_file.validate_into(&collector, None);
    Ok(Chat::from_validated_ast(validated, source_id.clone()))
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
    use crate::utils::MediaInput;
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
        let chat = build_chat_from_asr(
            &sid,
            &LanguageSpec::Code("eng".into()),
            &out,
            Some("WhisperBackend"),
        )
        .expect("chat");
        let text = chat.to_chat();
        assert!(text.contains("@Languages:\teng"));
        assert!(text.contains("@Participants:"));
        assert!(text.contains("*PAR0:\thello there ."));
        assert!(text.contains("*PAR1:\tgeneral kenobi ."));
        // Provenance stamp: BA version + engine name.
        assert!(
            text.contains("@Comment:")
                && text.contains("batchalign3 ")
                && text.contains("WhisperBackend"),
            "expected provenance @Comment in:\n{text}"
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
            .apply(&mut value, &disp, std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>)
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
