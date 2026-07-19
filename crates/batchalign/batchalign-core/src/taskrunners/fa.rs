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
};
use async_trait::async_trait;
use smol_str::SmolStr;
use std::path::Path;
use talkbank_model::Line;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::UtteranceContent;

use super::utr::extraction::split_compound_filler;

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

        // `@Options: NoAlign` is a strict per-file pass-through contract.
        // Check it before media resolution, audio decoding, progress, or
        // backend dispatch so an intentionally unaligned transcript does not
        // even require a sibling media file and remains byte-stable.
        if chat
            .ast()
            .options
            .iter()
            .any(|option| option.skips_alignment())
        {
            return Ok(());
        }

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
        let repairs = enforce_fa_monotonicity(chat);
        if repairs.stripped > 0 || repairs.clamped > 0 {
            tracing::warn!(
                source_id = %chat.source_id(),
                stripped = repairs.stripped,
                clamped = repairs.clamped,
                "repaired non-monotonic FA timing"
            );
        }
        // Ceiling tick — FA bar lands at 100% once the call returns and
        // word timings are injected. A backend that didn't tick still
        // sees the bar move from 0 → 100 here.
        progress.finish();

        // Provenance `@Comment` stamping happens once at end-of-pipeline in
        // `batchalign_engine::pipeline::run_one`; per-runner stamping has
        // been lifted to the pipeline so a single BA-touched file ends up
        // with a single `batchalign3 <sha> | …` comment for the whole run.

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
            if let Some(word) = source_word(&w) {
                for text in split_compound_filler(word) {
                    if text.is_empty() {
                        continue;
                    }
                    words.push(AsrWord {
                        text,
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
    use talkbank_model::model::{Bullet, WorTier};

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(seg) = aligned.get(idx) else {
            return Err(BAError::Internal(format!(
                "FA: missing aligned segment for utterance {idx}"
            )));
        };
        if !seg.words.is_empty() {
            let words = collapse_aligned_words(&u.main.content.content.0, &seg.words)?;
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

#[derive(Debug, Default, PartialEq, Eq)]
struct MonotonicityRepairs {
    stripped: usize,
    clamped: usize,
}

/// Repair timing corruption before the typed output reaches validation.
///
/// A backward utterance anchor is not safely guessable, so its main bullet
/// and `%wor` tier are removed together for a later recovery pass. For the
/// remaining monotonic anchors, an earlier end that crosses the next start is
/// clamped to that start. This mirrors the fork's E362 repair boundary while
/// preserving legitimate cross-speaker overlap starts that still move forward.
fn enforce_fa_monotonicity(chat: &mut Chat) -> MonotonicityRepairs {
    use talkbank_model::DependentTier;

    let mut repairs = MonotonicityRepairs::default();
    let mut last_start_ms = 0;
    for line in &mut chat.ast_mut().lines.0 {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        let Some(start_ms) = utterance
            .main
            .content
            .bullet
            .as_ref()
            .map(|bullet| bullet.timing.start_ms)
        else {
            continue;
        };
        if start_ms < last_start_ms {
            utterance.main.content.bullet = None;
            utterance
                .dependent_tiers
                .retain(|tier| !matches!(tier, DependentTier::Wor(_)));
            repairs.stripped += 1;
        } else {
            last_start_ms = start_ms;
        }
    }

    let timed: Vec<(usize, u64)> = chat
        .ast()
        .lines
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let Line::Utterance(utterance) = line else {
                return None;
            };
            Some((
                index,
                utterance.main.content.bullet.as_ref()?.timing.start_ms,
            ))
        })
        .collect();

    for pair in timed.windows(2) {
        let (previous_index, _) = pair[0];
        let (_, next_start_ms) = pair[1];
        let Line::Utterance(previous) = &mut chat.ast_mut().lines.0[previous_index] else {
            continue;
        };
        let Some(bullet) = previous.main.content.bullet.as_mut() else {
            continue;
        };
        if bullet.timing.end_ms <= next_start_ms {
            continue;
        }
        if next_start_ms <= bullet.timing.start_ms {
            previous.main.content.bullet = None;
            previous
                .dependent_tiers
                .retain(|tier| !matches!(tier, DependentTier::Wor(_)));
            repairs.stripped += 1;
        } else {
            bullet.timing.end_ms = next_start_ms;
            repairs.clamped += 1;
        }
    }

    repairs
}

/// Collapse the backend's expanded FA tokens back onto the source word domain.
/// A compound filler is one CHAT/%wor word even though each underscore-separated
/// component is independently recognizable in the audio.
fn collapse_aligned_words(
    content: &[UtteranceContent],
    aligned: &[AsrWord],
) -> BAResult<Vec<talkbank_model::model::Word>> {
    use talkbank_model::model::{Bullet, Word};

    let mut source_words: Vec<(String, usize)> = Vec::new();
    walk_words(content, None, &mut |item| {
        if let Some(word) = source_word(&item) {
            source_words.push((
                word.cleaned_text().to_string(),
                split_compound_filler(word).len(),
            ));
        }
    });

    let mut cursor = 0usize;
    let mut words = Vec::with_capacity(source_words.len());
    for (text, part_count) in source_words {
        let end = cursor.saturating_add(part_count);
        let parts = aligned.get(cursor..end).ok_or_else(|| {
            BAError::Internal(format!(
                "FA: missing aligned token(s) for source word {text:?} at expanded range {cursor}..{end}"
            ))
        })?;
        cursor = end;

        let timing = parts
            .iter()
            .filter(|part| part.end_ms > part.start_ms)
            .fold(None, |span: Option<(u64, u64)>, part| {
                Some(match span {
                    Some((start, end)) => (start.min(part.start_ms), end.max(part.end_ms)),
                    None => (part.start_ms, part.end_ms),
                })
            });
        let word = Word::simple(text.as_str());
        words.push(match timing {
            Some((start_ms, end_ms)) => word.with_inline_bullet(Bullet::new(start_ms, end_ms)),
            None => word,
        });
    }
    if cursor != aligned.len() {
        return Err(BAError::Internal(format!(
            "FA: expanded word/output count mismatch ({cursor} vs {})",
            aligned.len()
        )));
    }
    Ok(words)
}

fn source_word<'a>(w: &WordItem<'a>) -> Option<&'a talkbank_model::model::Word> {
    match w {
        WordItem::Word(word) => Some(word),
        WordItem::ReplacedWord(r) => Some(&r.word),
        WordItem::Separator(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PanicDispatcher;

    #[async_trait]
    impl Dispatcher for PanicDispatcher {
        async fn dispatch(&self, _input: TaskInput) -> BAResult<crate::base::TaskOutput> {
            panic!("NoAlign must return before backend dispatch")
        }
    }

    const COMPOUND_FILLER_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\t&-you_know today .\n@End\n";

    #[test]
    fn compound_filler_expands_for_fa_dispatch() {
        let chat = Chat::parse(
            COMPOUND_FILLER_CHAT,
            SourceId::try_new("compound").expect("source id"),
        )
        .expect("parse fixture");
        let utterances = extract_utterances_for_fa(&chat);
        let texts: Vec<&str> = utterances[0]
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect();
        assert_eq!(texts, ["you", "know", "today"]);
    }

    #[tokio::test]
    async fn no_align_is_strict_pass_through_without_media_or_dispatch() -> BAResult<()> {
        const NO_ALIGN_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n@Options:\tNoAlign\n@ID:\teng|test|PAR|||||Participant|||\n*PAR:\tdo not align me .\n@End\n";
        let chat = Chat::parse(NO_ALIGN_CHAT, SourceId::try_new("no-align.cha")?)?;
        let before = chat.to_chat();
        let mut value = BAValue::Chat(chat);

        FaTaskRunner
            .apply(
                &mut value,
                &PanicDispatcher,
                std::sync::Arc::new(crate::base::NullSink),
            )
            .await?;

        let BAValue::Chat(chat) = value else {
            panic!("expected CHAT pass-through")
        };
        assert_eq!(chat.to_chat(), before);
        Ok(())
    }

    #[test]
    fn compound_filler_parts_collapse_to_one_source_span() {
        let mut chat = Chat::parse(
            COMPOUND_FILLER_CHAT,
            SourceId::try_new("compound").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 100,
            end_ms: 600,
            text: "you know today".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "you".into(),
                    start_ms: 100,
                    end_ms: 200,
                    confidence: None,
                },
                AsrWord {
                    text: "know".into(),
                    start_ms: 225,
                    end_ms: 350,
                    confidence: None,
                },
                AsrWord {
                    text: "today".into(),
                    start_ms: 400,
                    end_ms: 600,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timings");
        let output = chat.to_chat();
        assert!(
            output.contains("you_know \u{15}100_350\u{15}"),
            "compound span should cover its first through last recognized part: {output}"
        );
        assert_eq!(
            output
                .lines()
                .find(|line| line.starts_with("%wor:"))
                .expect("wor tier")
                .matches('\u{15}')
                .count(),
            4,
            "two source words should carry exactly two bullet pairs"
        );
    }

    #[test]
    fn long_file_monotonicity_repairs_backward_anchor_and_overlap() {
        use std::fmt::Write as _;
        use talkbank_model::DependentTier;

        let mut source = String::from(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n\
             @ID:\teng|test|PAR|||||Participant|||\n",
        );
        for _ in 0..500 {
            writeln!(&mut source, "*PAR:\thello .").expect("write fixture");
        }
        source.push_str("@End\n");
        let mut chat = Chat::parse(
            &source,
            SourceId::try_new("long-monotonicity").expect("source id"),
        )
        .expect("parse long fixture");

        let aligned: Vec<AsrSegment> = (0..500)
            .map(|index| {
                let start_ms = if index == 400 {
                    1_000
                } else {
                    index as u64 * 1_000
                };
                let end_ms = start_ms + if index == 250 { 1_500 } else { 500 };
                AsrSegment {
                    start_ms,
                    end_ms,
                    text: "hello".into(),
                    speaker: None,
                    words: vec![AsrWord {
                        text: "hello".into(),
                        start_ms,
                        end_ms,
                        confidence: None,
                    }],
                }
            })
            .collect();

        inject_word_timings(&mut chat, &aligned).expect("inject timings");
        let repairs = enforce_fa_monotonicity(&mut chat);

        assert_eq!(
            repairs,
            MonotonicityRepairs {
                stripped: 1,
                clamped: 1
            }
        );
        let utterances: Vec<_> = chat
            .ast()
            .lines
            .0
            .iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .collect();
        assert_eq!(
            utterances[250]
                .main
                .content
                .bullet
                .as_ref()
                .expect("clamped bullet")
                .timing
                .end_ms,
            251_000
        );
        assert!(utterances[400].main.content.bullet.is_none());
        assert!(
            !utterances[400]
                .dependent_tiers
                .iter()
                .any(|tier| matches!(tier, DependentTier::Wor(_))),
            "stripped drift anchor must not retain stale word timing"
        );
        Chat::parse(
            &chat.to_chat(),
            SourceId::try_new("long-monotonicity-output").expect("source id"),
        )
        .expect("repaired long-file output must pass the full CHAT validator");
    }
}
