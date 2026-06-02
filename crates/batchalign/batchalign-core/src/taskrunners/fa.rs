//! `FaTaskRunner` — refines word timings on an existing CHAT against its audio.
//!
//! Decodes the transcript's sibling audio via `crate::utils::prepare_pcm`,
//! builds an `FaInput` whose `utterances` carry the existing main-tier text
//! plus each utterance's media-bullet window, dispatches the input, and folds
//! the returned per-word timings back onto each utterance as a typed `%wor`
//! tier (one `Word` per token, each carrying an inline `\x15start_end\x15`
//! bullet). The main-tier bullet is refined to span the first-aligned-word
//! start … last-aligned-word end.
//!
//! Behavioral parity targets `batchalign2/batchalign/pipelines/fa/wave2vec_fa.py`
//! (FA backend = MMS_FA; ~15 s utterance grouping; char-DP remap from
//! MMS_FA output words back to source words; post-correction that, when the
//! next item is untimed, extends the end by ~500 ms and bounds by the
//! utterance window). Sample-rate normalization to 16 kHz mono happens at
//! the audio-prep boundary (`utils::prepare_pcm`) so every FA backend sees
//! the same waveform shape BA2's `audio_io.load` produced.
//!
//! Per spec2.md §9 and the BA2 `pipelines/fa/` reference.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::asr::{AsrSegment, AsrWord, LanguageSpec};
use crate::proto::fa::{FaInput, FaOutput};
use crate::utils::{
    BAError, BAResult, MediaInput, SourceId, SpeakerLabel, clear_media_unlinked, prepare_pcm,
    stamp_provenance,
};
use async_trait::async_trait;
use smol_str::SmolStr;
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
        sink: std::sync::Arc<dyn ProgressSink>,
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
            // Resolve `@Languages:` here so language-aware FA backends
            // (Qwen3, …) get a concrete code without re-parsing the CHAT.
            // Language-agnostic backends (MMS_FA, Whisper) simply ignore
            // this field. Mirrors the morphosyntax runner's pattern
            // (`taskrunners/morphosyntax.rs::resolve_per_file_language`).
            language: resolve_per_file_language(chat),
        };

        // Progress: FA dispatches the whole file in one bulk call, so
        // this runner has no outer loop to tick. Outer total is 1 step;
        // the FA backend reports per-audio-group ticks (~15s wav2vec /
        // ~20s whisper chunks) through `progress.tick(i, n)`. The
        // wrapper rescales those into the 0..SCALE band so the bar
        // advances inside the single outer step.
        let source_id = chat.source_id().clone();
        let progress = std::sync::Arc::new(crate::base::ScaledProgress::new(
            sink.clone(),
            source_id.clone(),
            Task::Fa,
            1,
        ));
        let progress_dyn: std::sync::Arc<dyn crate::base::BackendProgress> = progress.clone();
        progress.start_step();
        let output_raw = dispatcher
            .dispatch_with_progress(TaskInput::Fa(input), progress_dyn)
            .await?;
        let output: FaOutput = output_raw.try_into()?;

        inject_word_timings(chat, &output.utterances)?;
        // Ceiling tick — FA bar lands at 100% once the call returns and
        // word timings are injected. A backend that didn't tick still
        // sees the bar move from 0 → 100 here.
        progress.finish();

        // Stamp the file with BA version + engine name (parity with
        // `asr.rs::build_chat_from_asr`'s provenance `@Comment`). The shared
        // `stamp_provenance` helper dedupes any prior stamp so reruns don't
        // accrete one `@Comment` per invocation.
        let engine = dispatcher.engine_name(Task::Fa);
        stamp_provenance(&mut chat.ast_mut().lines.0, engine.as_deref());

        // FA just injected bullets — if the input was tagged `, unlinked`
        // (the E544-required marker for transcripts with no timing), that
        // tag is now stale. Drop it so the output advertises its newly-
        // linked state and downstream tools honour the timing.
        clear_media_unlinked(&mut chat.ast_mut().lines.0);

        sink.emit(ProgressEvent::stage_injected(chat.source_id(), Task::Fa));
        Ok(())
    }
}

/// Read the chat's `@Languages:` header and emit a concrete `LanguageSpec`.
/// Falls back to `PerFile` (a no-op marker) when the header is absent so
/// backends can do their own fallback. Same pattern as
/// `taskrunners/morphosyntax.rs::resolve_per_file_language`.
fn resolve_per_file_language(chat: &Chat) -> LanguageSpec {
    if let Some(code) = chat.primary_language() {
        LanguageSpec::Code(SmolStr::new(code))
    } else {
        LanguageSpec::PerFile
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
///
/// Progress: this is a fast post-processing loop that runs after the
/// (slow) FA backend call returns. It no longer emits ticks — the
/// runner's `ScaledProgress` reflects the actual alignment work via
/// backend-side ticks during the dispatch, not the trailing
/// in-memory tier-attachment loop.
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
                    let word = Word::simple(w.text.as_str());
                    // An untimed word (FA found no span) renders as a bare word
                    // with no bullet — matching BA2, which omits the timing.
                    if w.start_ms == 0 && w.end_ms == 0 {
                        word
                    } else {
                        word.with_inline_bullet(Bullet::new(w.start_ms, w.end_ms))
                    }
                })
                .collect();
            // Carry the utterance's own terminator onto `%wor` (BA2 parity);
            // the typed writer renders the bullets and the terminator.
            let wor = WorTier::from_words(words).with_terminator(u.main.content.terminator.clone());
            // Retag semantics: if FA was already run (or the source CHAT
            // shipped a `%wor:` tier), drop the old one so we don't end up
            // with two `%wor:` lines per utterance. BA2 mutates word timings
            // in place; the typed-tier equivalent is replace-not-append.
            u.dependent_tiers
                .retain(|t| !matches!(t, DependentTier::Wor(_)));
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
