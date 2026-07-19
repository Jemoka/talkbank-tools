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
use talkbank_model::alignment::helpers::{TierDomain, WordItem, counts_for_tier, walk_words};
use talkbank_model::model::UtteranceContent;

use super::utr::extraction::split_compound_filler;

/// Audio container extensions to probe for a transcript's sibling media,
/// in priority order (BA2/ffmpeg accept all of these).
const SIBLING_AUDIO_EXTS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "aac", "wma", "mov", "avi", "mpg", "mpeg",
];

#[cfg(debug_assertions)]
fn fa_debug_trace(label: &str, payload: impl std::fmt::Display) {
    if std::env::var_os("BATCHALIGN_PARITY_TRACE").is_some() {
        eprintln!("[batchalign-fa-debug] {label}\n{payload}");
    }
}

#[cfg(not(debug_assertions))]
fn fa_debug_trace(_label: &str, _payload: impl std::fmt::Display) {}

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

        strip_stale_fa_review_tiers(chat);

        // A completed prior FA run already carries the most precise timing
        // surface we can reuse: one timed `%wor` item per current main-tier
        // word. Refresh its utterance bullets without decoding audio or
        // dispatching the backend again.
        if refresh_complete_wor_alignment(chat) {
            clear_media_unlinked(&mut chat.ast_mut().lines.0);
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

        let reusable_segments = collect_reusable_wor_segments(chat);
        let utterances = extract_utterances_for_fa(chat)
            .into_iter()
            .zip(reusable_segments.iter())
            .filter_map(|(segment, reusable)| reusable.is_none().then_some(segment))
            .collect();

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

        let aligned = merge_reused_and_fresh_segments(reusable_segments, output.utterances)?;
        inject_word_timings(chat, &aligned)?;
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

/// Remove decision tiers produced by an earlier alignment run.
///
/// A clean rerun may produce no replacement decisions, so cleanup cannot be
/// conditional on emitting new review tiers. `NoAlign` files bypass this
/// helper and remain strict pass-throughs.
fn strip_stale_fa_review_tiers(chat: &mut Chat) {
    use talkbank_model::DependentTier;

    for line in &mut chat.ast_mut().lines.0 {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        utterance.dependent_tiers.retain(|tier| {
            !matches!(
                tier,
                DependentTier::UserDefined(user)
                    if matches!(user.label.as_str(), "xalign" | "xrev")
            )
        });
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

/// Refresh every alignable utterance from an exact, fully timed `%wor` tier.
///
/// Returns `false` without mutation when any alignable utterance is missing a
/// word, has different text, or contains an absent/zero-duration word span.
fn refresh_complete_wor_alignment(chat: &mut Chat) -> bool {
    const MAX_REUSABLE_WORD_DURATION_PROPORTION: f64 = 0.4;
    const MIN_WORDS_FOR_DOMINANCE_CHECK: usize = 3;
    const MIN_REUSABLE_WORD_DURATION_MS: u64 = 40;

    let mut refreshed = Vec::new();
    let mut saw_words = false;
    fa_debug_trace("complete-reuse input", chat.to_chat());

    for (line_index, line) in chat.ast().lines.0.iter().enumerate() {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        let mut main_words = Vec::new();
        walk_words(&utterance.main.content.content.0, None, &mut |item| {
            if let Some(word) = source_word(&item)
                && counts_for_tier(word, TierDomain::Wor)
            {
                main_words.push(word.cleaned_text().to_string());
            }
        });
        if main_words.is_empty() {
            continue;
        }
        saw_words = true;

        let Some(wor) = utterance.wor_tier() else {
            fa_debug_trace(
                "complete-reuse rejected",
                format!("line={line_index} reason=missing-wor"),
            );
            return false;
        };
        let wor_words: Vec<_> = wor.words().collect();
        if wor_words.len() != main_words.len()
            || wor_words
                .iter()
                .zip(main_words.iter())
                .any(|(word, expected)| word.cleaned_text() != expected)
        {
            fa_debug_trace(
                "complete-reuse rejected",
                format!("line={line_index} reason=word-mismatch"),
            );
            return false;
        }

        let mut first_start = None;
        let mut last_end = None;
        let mut minimum_start = None;
        let mut maximum_end = None;
        let mut maximum_duration_ms = 0;
        let mut previous_end_ms = None;
        let word_count = wor_words.len();
        for word in wor_words {
            let Some(bullet) = word.inline_bullet.as_ref() else {
                fa_debug_trace(
                    "complete-reuse rejected",
                    format!("line={line_index} reason=untimed-word"),
                );
                return false;
            };
            if bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms)
                < MIN_REUSABLE_WORD_DURATION_MS
            {
                fa_debug_trace(
                    "complete-reuse rejected",
                    format!(
                        "line={line_index} reason=short-word duration_ms={}",
                        bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms)
                    ),
                );
                return false;
            }
            if previous_end_ms.is_some_and(|previous_end| bullet.timing.start_ms < previous_end) {
                fa_debug_trace(
                    "complete-reuse rejected",
                    format!(
                        "line={line_index} reason=backward-word start_ms={}",
                        bullet.timing.start_ms
                    ),
                );
                return false;
            }
            let duration_ms = bullet.timing.end_ms - bullet.timing.start_ms;
            minimum_start = Some(minimum_start.map_or(bullet.timing.start_ms, |start: u64| {
                start.min(bullet.timing.start_ms)
            }));
            maximum_end = Some(maximum_end.map_or(bullet.timing.end_ms, |end: u64| {
                end.max(bullet.timing.end_ms)
            }));
            maximum_duration_ms = maximum_duration_ms.max(duration_ms);
            previous_end_ms = Some(bullet.timing.end_ms);
            first_start.get_or_insert(bullet.timing.start_ms);
            last_end = Some(bullet.timing.end_ms);
        }
        if word_count >= MIN_WORDS_FOR_DOMINANCE_CHECK {
            let utterance_span_ms = maximum_end
                .expect("non-empty word list has a maximum end")
                .saturating_sub(minimum_start.expect("non-empty word list has a minimum start"));
            if utterance_span_ms > 0
                && maximum_duration_ms as f64 / utterance_span_ms as f64
                    > MAX_REUSABLE_WORD_DURATION_PROPORTION
            {
                fa_debug_trace(
                    "complete-reuse rejected",
                    format!(
                        "line={line_index} reason=dominant-word max_duration_ms={maximum_duration_ms} utterance_span_ms={utterance_span_ms}"
                    ),
                );
                return false;
            }
        }
        refreshed.push((
            line_index,
            first_start.expect("non-empty word list has a first timing"),
            last_end.expect("non-empty word list has a last timing"),
        ));
    }

    if !saw_words {
        fa_debug_trace("complete-reuse rejected", "reason=no-alignable-words");
        return false;
    }
    for (line_index, _, end_ms) in &refreshed {
        let next_start_ms =
            chat.ast().lines.0[line_index + 1..]
                .iter()
                .find_map(|line| match line {
                    Line::Utterance(utterance) => utterance
                        .main
                        .content
                        .bullet
                        .as_ref()
                        .map(|bullet| bullet.timing.start_ms),
                    _ => None,
                });
        if next_start_ms.is_some_and(|next_start| *end_ms > next_start) {
            fa_debug_trace(
                "complete-reuse rejected",
                format!(
                    "line={line_index} reason=overruns-next end_ms={end_ms} next_start_ms={}",
                    next_start_ms.expect("checked Some")
                ),
            );
            return false;
        }
    }
    for (line_index, start_ms, end_ms) in refreshed {
        let Line::Utterance(utterance) = &mut chat.ast_mut().lines.0[line_index] else {
            unreachable!("recorded line index must remain an utterance")
        };
        utterance.main.content.bullet = Some(talkbank_model::model::Bullet::new(start_ms, end_ms));
    }
    fa_debug_trace("complete-reuse output", chat.to_chat());
    true
}

/// Collect clean per-utterance `%wor` timing for selective FA reuse.
///
/// The whole-file fast path above handles the all-clean case before media
/// decoding. This per-utterance form lets a mixed file dispatch only stale
/// utterances while preserving trustworthy existing timing for the rest.
fn collect_reusable_wor_segments(chat: &Chat) -> Vec<Option<AsrSegment>> {
    const MAX_REUSABLE_WORD_DURATION_PROPORTION: f64 = 0.4;
    const MIN_WORDS_FOR_DOMINANCE_CHECK: usize = 3;
    const MIN_REUSABLE_WORD_DURATION_MS: u64 = 40;

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
    let mut next_timed_start = None;
    let mut next_starts = vec![None; utterances.len()];
    for index in (0..utterances.len()).rev() {
        next_starts[index] = next_timed_start;
        if let Some(bullet) = utterances[index].main.content.bullet.as_ref() {
            next_timed_start = Some(bullet.timing.start_ms);
        }
    }

    let segments: Vec<_> = utterances
        .into_iter()
        .enumerate()
        .map(|(index, utterance)| {
            let mut main_words = Vec::new();
            walk_words(&utterance.main.content.content.0, None, &mut |item| {
                if let Some(word) = source_word(&item)
                    && counts_for_tier(word, TierDomain::Wor)
                {
                    main_words.push(word.cleaned_text().to_string());
                }
            });
            if main_words.is_empty() {
                return None;
            }
            let wor = utterance.wor_tier()?;
            let wor_words: Vec<_> = wor.words().collect();
            if wor_words.len() != main_words.len()
                || wor_words
                    .iter()
                    .zip(&main_words)
                    .any(|(word, expected)| word.cleaned_text() != expected)
            {
                return None;
            }

            let mut words = Vec::with_capacity(wor_words.len());
            let mut maximum_duration_ms = 0;
            let mut previous_end_ms = None;
            for word in wor_words {
                let bullet = word.inline_bullet.as_ref()?;
                let duration_ms = bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms);
                if duration_ms < MIN_REUSABLE_WORD_DURATION_MS
                    || previous_end_ms
                        .is_some_and(|previous_end| bullet.timing.start_ms < previous_end)
                {
                    return None;
                }
                maximum_duration_ms = maximum_duration_ms.max(duration_ms);
                previous_end_ms = Some(bullet.timing.end_ms);
                words.push(AsrWord {
                    text: word.cleaned_text().to_string(),
                    start_ms: bullet.timing.start_ms,
                    end_ms: bullet.timing.end_ms,
                    confidence: None,
                });
            }
            let start_ms = words.first()?.start_ms;
            let end_ms = words.last()?.end_ms;
            if words.len() >= MIN_WORDS_FOR_DOMINANCE_CHECK {
                let span_ms = end_ms.saturating_sub(start_ms);
                if span_ms > 0
                    && maximum_duration_ms as f64 / span_ms as f64
                        > MAX_REUSABLE_WORD_DURATION_PROPORTION
                {
                    return None;
                }
            }
            if next_starts[index].is_some_and(|next_start| end_ms > next_start) {
                return None;
            }
            Some(AsrSegment {
                start_ms,
                end_ms,
                text: words
                    .iter()
                    .map(|word| word.text.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
                speaker: Some(SpeakerLabel::new(utterance.main.speaker.as_str())),
                words,
            })
        })
        .collect();
    fa_debug_trace(
        "partial-reuse selection",
        format!(
            "reused={} fresh={} total={}",
            segments.iter().filter(|segment| segment.is_some()).count(),
            segments.iter().filter(|segment| segment.is_none()).count(),
            segments.len()
        ),
    );
    segments
}

fn merge_reused_and_fresh_segments(
    reused: Vec<Option<AsrSegment>>,
    fresh: Vec<AsrSegment>,
) -> BAResult<Vec<AsrSegment>> {
    let mut fresh = fresh.into_iter();
    let mut merged = Vec::with_capacity(reused.len());
    for reusable in reused {
        match reusable {
            Some(segment) => merged.push(segment),
            None => merged.push(fresh.next().ok_or_else(|| {
                BAError::Internal("FA: missing fresh segment during partial reuse".into())
            })?),
        }
    }
    if fresh.next().is_some() {
        return Err(BAError::Internal(
            "FA: extra fresh segment during partial reuse".into(),
        ));
    }
    Ok(merged)
}

fn extract_utterances_for_fa(chat: &Chat) -> Vec<AsrSegment> {
    let mut out = Vec::new();
    for line in chat.ast().lines.0.iter() {
        let Line::Utterance(u) = line else { continue };
        let mut words = Vec::new();
        walk_words(&u.main.content.content.0, None, &mut |w| {
            if let Some(word) = source_word(&w)
                && counts_for_tier(word, TierDomain::Wor)
            {
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
    use talkbank_model::model::{Bullet, BulletSource, WorTier};

    fa_debug_trace("injection input CHAT", chat.to_chat());
    fa_debug_trace(
        "injection aligned payload",
        serde_json::to_string_pretty(aligned)
            .unwrap_or_else(|error| format!("<serialization failed: {error}>")),
    );
    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        let Line::Utterance(u) = line else { continue };
        let Some(seg) = aligned.get(idx) else {
            return Err(BAError::Internal(format!(
                "FA: missing aligned segment for utterance {idx}"
            )));
        };
        if !seg.words.is_empty() {
            let had_wor_tier = u
                .dependent_tiers
                .iter()
                .any(|tier| matches!(tier, DependentTier::Wor(_)));
            let mut words = collapse_aligned_words(&u.main.content.content.0, &seg.words)?;
            rebalance_near_zero_words_from_following(&mut words);
            rebalance_near_zero_words_from_preceding(&mut words);
            let has_untimed_leading_filler =
                has_untimed_leading_filler_coverage(&u.main.content.content.0, &words);
            // Carry the utterance's own terminator onto `%wor` (BA2 parity);
            // the typed writer renders the bullets and the terminator.
            let wor = WorTier::from_words(words).with_terminator(u.main.content.terminator.clone());
            // Retag semantics: if FA was already run (or the source CHAT
            // shipped a `%wor:` tier), drop the old one so we don't end up
            // with two `%wor:` lines per utterance. BA2 mutates word timings
            // in place; the typed-tier equivalent is replace-not-append.
            let original_wor_position = u
                .dependent_tiers
                .iter()
                .position(|tier| matches!(tier, DependentTier::Wor(_)));
            u.dependent_tiers
                .retain(|tier| !matches!(tier, DependentTier::Wor(_)));
            match original_wor_position {
                Some(position) => u.dependent_tiers.insert(
                    position.min(u.dependent_tiers.len()),
                    DependentTier::Wor(wor),
                ),
                None => u.dependent_tiers.push(DependentTier::Wor(wor)),
            }
            let timed_word_span = seg
                .words
                .iter()
                .filter(|word| word.end_ms > word.start_ms)
                .fold(None, |span: Option<(u64, u64)>, word| {
                    Some(match span {
                        Some((start_ms, end_ms)) => {
                            (start_ms.min(word.start_ms), end_ms.max(word.end_ms))
                        }
                        None => (word.start_ms, word.end_ms),
                    })
                });
            if let Some((word_start_ms, word_end_ms)) = timed_word_span {
                // BA2 refines the main-tier utterance bullet to span the aligned
                // words (first word start … last word end).
                let (start_ms, end_ms) = match u.main.content.bullet.as_ref() {
                    Some(bullet) if bullet.source == BulletSource::Utr => {
                        (word_start_ms, word_end_ms)
                    }
                    Some(bullet) if bullet.source == BulletSource::Authoritative => {
                        const MAX_AUTHORITATIVE_START_LEAD_MS: u64 = 2_000;
                        let start_lead_ms = word_start_ms.saturating_sub(bullet.timing.start_ms);
                        let start_ms = if had_wor_tier
                            && !has_untimed_leading_filler
                            && start_lead_ms > MAX_AUTHORITATIVE_START_LEAD_MS
                        {
                            word_start_ms
                        } else {
                            bullet.timing.start_ms.min(word_start_ms)
                        };
                        (start_ms, bullet.timing.end_ms.max(word_end_ms))
                    }
                    _ => (word_start_ms, word_end_ms),
                };
                u.main.content.bullet = Some(Bullet::new(start_ms, end_ms));
            } else if u.main.content.bullet.as_ref().is_some_and(|bullet| {
                bullet.source == BulletSource::Authoritative
                    && bullet.timing.end_ms <= bullet.timing.start_ms
            }) {
                // A stale authoritative T_T anchor cannot be retained when FA
                // found no usable word timing; it would fail temporal validation.
                u.main.content.bullet = None;
            }
        }
        idx += 1;
    }
    if idx != aligned.len() {
        return Err(BAError::Internal(format!(
            "FA: utterance/output count mismatch ({idx} vs {})",
            aligned.len()
        )));
    }
    fa_debug_trace("injection output CHAT", chat.to_chat());
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
        if let Some(word) = source_word(&item)
            && counts_for_tier(word, TierDomain::Wor)
        {
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

fn has_untimed_leading_filler_coverage(
    content: &[UtteranceContent],
    aligned_words: &[talkbank_model::model::Word],
) -> bool {
    use talkbank_model::model::WordCategory;

    let mut source_categories = Vec::new();
    walk_words(content, None, &mut |item| {
        if let Some(word) = source_word(&item)
            && counts_for_tier(word, TierDomain::Wor)
        {
            source_categories.push(word.category.clone());
        }
    });

    for (category, aligned_word) in source_categories.iter().zip(aligned_words) {
        if aligned_word
            .inline_bullet
            .as_ref()
            .is_some_and(|bullet| bullet.timing.end_ms > bullet.timing.start_ms)
        {
            break;
        }
        if category.as_ref() == Some(&WordCategory::Filler) {
            return true;
        }
    }
    false
}

fn rebalance_near_zero_words_from_following(words: &mut [talkbank_model::model::Word]) {
    const MIN_WORD_DURATION_MS: u64 = 40;

    for index in 0..words.len().saturating_sub(1) {
        let (before, after) = words.split_at_mut(index + 1);
        let Some(current) = before[index].inline_bullet.as_mut() else {
            continue;
        };
        let Some(next) = after[0].inline_bullet.as_mut() else {
            continue;
        };
        let duration_ms = current
            .timing
            .end_ms
            .saturating_sub(current.timing.start_ms);
        if duration_ms == 0 || duration_ms >= MIN_WORD_DURATION_MS {
            continue;
        }
        let repaired_boundary = current.timing.start_ms + MIN_WORD_DURATION_MS;
        if next.timing.start_ms <= current.timing.end_ms && repaired_boundary < next.timing.end_ms {
            current.timing.end_ms = repaired_boundary;
            next.timing.start_ms = repaired_boundary;
        }
    }
}

fn rebalance_near_zero_words_from_preceding(words: &mut [talkbank_model::model::Word]) {
    const MIN_WORD_DURATION_MS: u64 = 40;

    for index in 1..words.len() {
        let (before, after) = words.split_at_mut(index);
        let Some(previous) = before[index - 1].inline_bullet.as_mut() else {
            continue;
        };
        let Some(current) = after[0].inline_bullet.as_mut() else {
            continue;
        };
        let duration_ms = current
            .timing
            .end_ms
            .saturating_sub(current.timing.start_ms);
        if duration_ms == 0 || duration_ms >= MIN_WORD_DURATION_MS {
            continue;
        }
        let Some(repaired_boundary) = current.timing.end_ms.checked_sub(MIN_WORD_DURATION_MS)
        else {
            continue;
        };
        if current.timing.start_ms <= previous.timing.end_ms
            && repaired_boundary > previous.timing.start_ms
        {
            previous.timing.end_ms = repaired_boundary;
            current.timing.start_ms = repaired_boundary;
        }
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

    #[test]
    fn fa_excludes_untranscribed_words_from_dispatch_and_wor() {
        const UNTRANSCRIBED_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello xxx world . \u{15}100_600\u{15}\n@End\n";
        let mut chat = Chat::parse(
            UNTRANSCRIBED_CHAT,
            SourceId::try_new("untranscribed.cha").expect("source id"),
        )
        .expect("parse fixture");
        let extracted = extract_utterances_for_fa(&chat);
        assert_eq!(
            extracted[0]
                .words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            ["hello", "world"]
        );

        let aligned = vec![AsrSegment {
            start_ms: 100,
            end_ms: 600,
            text: "hello world".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "hello".into(),
                    start_ms: 100,
                    end_ms: 300,
                    confidence: None,
                },
                AsrWord {
                    text: "world".into(),
                    start_ms: 350,
                    end_ms: 600,
                    confidence: None,
                },
            ],
        }];
        inject_word_timings(&mut chat, &aligned).expect("inject timed result");
        let wor_line = chat
            .to_chat()
            .lines()
            .find(|line| line.starts_with("%wor:"))
            .expect("wor tier")
            .to_string();
        assert!(!wor_line.contains("xxx"));
        assert!(wor_line.contains("hello") && wor_line.contains("world"));
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

    #[tokio::test]
    async fn complete_wor_reuse_skips_media_and_backend() -> BAResult<()> {
        const REUSABLE_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\thello world . \u{15}0_999\u{15}\n\
%wor:\thello \u{15}100_200\u{15} world \u{15}300_500\u{15} .\n@End\n";
        let chat = Chat::parse(REUSABLE_CHAT, SourceId::try_new("reusable.cha")?)?;
        let mut value = BAValue::Chat(chat);

        FaTaskRunner
            .apply(
                &mut value,
                &PanicDispatcher,
                std::sync::Arc::new(crate::base::NullSink),
            )
            .await?;

        let BAValue::Chat(chat) = value else {
            panic!("expected reused CHAT")
        };
        assert!(chat.to_chat().contains("\u{15}100_500\u{15}"));
        Ok(())
    }

    #[tokio::test]
    async fn clean_fa_rerun_strips_stale_review_tiers() -> BAResult<()> {
        const STALE_REVIEW_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\thello world . \u{15}100_500\u{15}\n\
%wor:\thello \u{15}100_200\u{15} world \u{15}300_500\u{15} .\n\
%xalign:\tfa:old_decision old_reason\n\
%xrev:\t[ok]\n@End\n";
        let chat = Chat::parse(STALE_REVIEW_CHAT, SourceId::try_new("stale-review.cha")?)?;
        let mut value = BAValue::Chat(chat);

        FaTaskRunner
            .apply(
                &mut value,
                &PanicDispatcher,
                std::sync::Arc::new(crate::base::NullSink),
            )
            .await?;

        let BAValue::Chat(chat) = value else {
            panic!("expected reused CHAT")
        };
        let output = chat.to_chat();
        assert!(!output.contains("%xalign:"));
        assert!(!output.contains("%xrev:"));
        Ok(())
    }

    #[test]
    fn complete_wor_reuse_rejects_span_past_next_utterance_start() {
        const OVERRUN_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\thello world . \u{15}100_900\u{15}\n\
%wor:\thello \u{15}100_200\u{15} world \u{15}300_1200\u{15} .\n\
*PAR:\tgoodbye . \u{15}1000_1300\u{15}\n\
%wor:\tgoodbye \u{15}1000_1200\u{15} .\n@End\n";
        let mut chat = Chat::parse(
            OVERRUN_CHAT,
            SourceId::try_new("overrun.cha").expect("source id"),
        )
        .expect("parse fixture");

        assert!(!refresh_complete_wor_alignment(&mut chat));
        assert!(chat.to_chat().contains("\u{15}100_900\u{15}"));
    }

    #[test]
    fn mixed_file_reuses_only_clean_wor_utterances() {
        const MIXED_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello world . \u{15}100_500\u{15}\n\
%wor:\thello \u{15}100_200\u{15} world \u{15}300_500\u{15} .\n\
*PAR:\tgoodbye friend . \u{15}1000_1500\u{15}\n\
%wor:\tgoodbye \u{15}1000_1200\u{15} .\n@End\n";
        let chat = Chat::parse(
            MIXED_CHAT,
            SourceId::try_new("mixed-reuse.cha").expect("source id"),
        )
        .expect("parse fixture");

        let reusable = collect_reusable_wor_segments(&chat);
        assert_eq!(reusable.len(), 2);
        assert!(reusable[0].is_some());
        assert!(reusable[1].is_none());

        let fresh = AsrSegment {
            start_ms: 1_000,
            end_ms: 1_500,
            text: "goodbye friend".into(),
            speaker: Some(SpeakerLabel::new("PAR")),
            words: vec![
                AsrWord {
                    text: "goodbye".into(),
                    start_ms: 1_000,
                    end_ms: 1_200,
                    confidence: None,
                },
                AsrWord {
                    text: "friend".into(),
                    start_ms: 1_250,
                    end_ms: 1_500,
                    confidence: None,
                },
            ],
        };
        let merged = merge_reused_and_fresh_segments(reusable, vec![fresh])
            .expect("merge one reused and one fresh utterance");
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "hello world");
        assert_eq!(merged[1].text, "goodbye friend");
    }

    #[test]
    fn complete_wor_reuse_rejects_near_zero_word_span() {
        const COLLAPSED_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\thello tiny world . \u{15}100_900\u{15}\n\
%wor:\thello \u{15}100_250\u{15} tiny \u{15}300_330\u{15} world \u{15}400_600\u{15} .\n@End\n";
        let mut chat = Chat::parse(
            COLLAPSED_CHAT,
            SourceId::try_new("collapsed.cha").expect("source id"),
        )
        .expect("parse fixture");

        assert!(!refresh_complete_wor_alignment(&mut chat));
        assert!(chat.to_chat().contains("\u{15}100_900\u{15}"));
    }

    #[test]
    fn complete_wor_reuse_rejects_dominant_word_span() {
        const DOMINANT_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\tone dominant word . \u{15}100_1000\u{15}\n\
%wor:\tone \u{15}100_250\u{15} dominant \u{15}250_800\u{15} word \u{15}800_900\u{15} .\n@End\n";
        let mut chat = Chat::parse(
            DOMINANT_CHAT,
            SourceId::try_new("dominant.cha").expect("source id"),
        )
        .expect("parse fixture");

        assert!(!refresh_complete_wor_alignment(&mut chat));
        assert!(chat.to_chat().contains("\u{15}100_1000\u{15}"));
    }

    #[test]
    fn complete_wor_reuse_rejects_backward_word_timing() {
        const BACKWARD_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\tmissing, audio\n\
*PAR:\thello world . \u{15}100_900\u{15}\n\
%wor:\thello \u{15}500_600\u{15} world \u{15}400_480\u{15} .\n@End\n";
        let mut chat = Chat::parse(
            BACKWARD_CHAT,
            SourceId::try_new("backward-wor.cha").expect("source id"),
        )
        .expect("parse fixture");

        assert!(!refresh_complete_wor_alignment(&mut chat));
        assert!(chat.to_chat().contains("\u{15}100_900\u{15}"));
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
    fn untimed_fa_result_clears_zero_duration_authoritative_bullet() {
        const ZERO_BULLET_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\tz@l . \u{15}245000_246000\u{15}\n@End\n";
        let mut chat = Chat::parse(
            ZERO_BULLET_CHAT,
            SourceId::try_new("zero-bullet.cha").expect("source id"),
        )
        .expect("parse fixture");
        let utterance = chat
            .ast_mut()
            .lines
            .0
            .iter_mut()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .expect("utterance");
        let bullet = utterance.main.content.bullet.as_mut().expect("main bullet");
        bullet.timing.start_ms = 245_986;
        bullet.timing.end_ms = 245_986;
        let aligned = vec![AsrSegment {
            start_ms: 245_986,
            end_ms: 245_986,
            text: "z".into(),
            speaker: None,
            words: vec![AsrWord {
                text: "z".into(),
                start_ms: 0,
                end_ms: 0,
                confidence: None,
            }],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject untimed result");

        let utterance = chat
            .ast()
            .lines
            .0
            .iter()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .expect("utterance");
        assert!(utterance.main.content.bullet.is_none());
    }

    #[test]
    fn timed_fa_words_overwrite_provisional_utr_window() {
        const UTR_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello world . \u{15}800_3000\u{15}\n@End\n";
        let mut chat = Chat::parse(
            UTR_CHAT,
            SourceId::try_new("utr-window.cha").expect("source id"),
        )
        .expect("parse fixture");
        let utterance = chat
            .ast_mut()
            .lines
            .0
            .iter_mut()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance),
                _ => None,
            })
            .expect("utterance");
        utterance
            .main
            .content
            .bullet
            .as_mut()
            .expect("main bullet")
            .source = talkbank_model::model::BulletSource::Utr;
        let aligned = vec![AsrSegment {
            start_ms: 800,
            end_ms: 3_000,
            text: "hello world".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "hello".into(),
                    start_ms: 1_000,
                    end_ms: 1_500,
                    confidence: None,
                },
                AsrWord {
                    text: "world".into(),
                    start_ms: 1_500,
                    end_ms: 2_000,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        assert!(chat.to_chat().contains("\u{15}1000_2000\u{15}"));
        assert!(!chat.to_chat().contains("\u{15}800_3000\u{15}"));
    }

    #[test]
    fn fa_replaces_wor_tier_at_its_original_position() {
        const ORDERED_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello . \u{15}100_500\u{15}\n\
%wor:\thello \u{15}100_500\u{15} .\n\
%mor:\tintj|hello .\n@End\n";
        let mut chat = Chat::parse(
            ORDERED_CHAT,
            SourceId::try_new("wor-order.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 100,
            end_ms: 500,
            text: "hello".into(),
            speaker: None,
            words: vec![AsrWord {
                text: "hello".into(),
                start_ms: 150,
                end_ms: 450,
                confidence: None,
            }],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        let output = chat.to_chat();
        assert!(output.find("%wor:").expect("wor tier") < output.find("%mor:").expect("mor tier"));
        assert!(output.contains("%wor:\thello \u{15}150_450\u{15} ."));
    }

    #[test]
    fn fa_rebalances_near_zero_word_from_following_span() {
        const SHORT_WORD_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\ta boat . \u{15}100_500\u{15}\n@End\n";
        let mut chat = Chat::parse(
            SHORT_WORD_CHAT,
            SourceId::try_new("short-word.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 100,
            end_ms: 500,
            text: "a boat".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "a".into(),
                    start_ms: 100,
                    end_ms: 120,
                    confidence: None,
                },
                AsrWord {
                    text: "boat".into(),
                    start_ms: 120,
                    end_ms: 500,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        let output = chat.to_chat();
        assert!(output.contains("%wor:\ta \u{15}100_140\u{15} boat \u{15}140_500\u{15} ."));
    }

    #[test]
    fn fa_rebalances_near_zero_word_from_preceding_span() {
        const SHORT_FINAL_WORD_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\t&-um I . \u{15}100_520\u{15}\n@End\n";
        let mut chat = Chat::parse(
            SHORT_FINAL_WORD_CHAT,
            SourceId::try_new("short-final-word.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 100,
            end_ms: 520,
            text: "um I".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "um".into(),
                    start_ms: 100,
                    end_ms: 500,
                    confidence: None,
                },
                AsrWord {
                    text: "I".into(),
                    start_ms: 500,
                    end_ms: 520,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        let output = chat.to_chat();
        assert!(
            output.contains("%wor:\tum \u{15}100_480\u{15} I \u{15}480_520\u{15} ."),
            "expected preceding-span repair in:\n{output}"
        );
    }

    #[test]
    fn timed_fa_words_preserve_authoritative_bullet_envelope() {
        const AUTHORITATIVE_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello world . \u{15}800_3000\u{15}\n@End\n";
        let mut chat = Chat::parse(
            AUTHORITATIVE_CHAT,
            SourceId::try_new("authoritative-window.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 900,
            end_ms: 2_200,
            text: "hello world".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "hello".into(),
                    start_ms: 1_000,
                    end_ms: 1_500,
                    confidence: None,
                },
                AsrWord {
                    text: "world".into(),
                    start_ms: 1_500,
                    end_ms: 2_000,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        assert!(chat.to_chat().contains("\u{15}800_3000\u{15}"));
        assert!(!chat.to_chat().contains("\u{15}900_2200\u{15}"));
    }

    #[test]
    fn timed_fa_words_create_main_bullet_from_word_span() {
        const UNTIMED_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thello world .\n@End\n";
        let mut chat = Chat::parse(
            UNTIMED_CHAT,
            SourceId::try_new("untimed-window.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 800,
            end_ms: 3_000,
            text: "hello world".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "hello".into(),
                    start_ms: 1_000,
                    end_ms: 1_500,
                    confidence: None,
                },
                AsrWord {
                    text: "world".into(),
                    start_ms: 1_500,
                    end_ms: 2_000,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        assert!(chat.to_chat().contains("\u{15}1000_2000\u{15}"));
        assert!(!chat.to_chat().contains("\u{15}800_3000\u{15}"));
    }

    #[test]
    fn fa_rerun_discards_large_stale_authoritative_start() {
        const STALE_START_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\thow this happened ? \u{15}2000_9970\u{15}\n\
%wor:\thow this happened ?\n@End\n";
        let mut chat = Chat::parse(
            STALE_START_CHAT,
            SourceId::try_new("stale-start.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 2_000,
            end_ms: 9_970,
            text: "how this happened".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "how".into(),
                    start_ms: 9_443,
                    end_ms: 9_643,
                    confidence: None,
                },
                AsrWord {
                    text: "this".into(),
                    start_ms: 9_643,
                    end_ms: 9_783,
                    confidence: None,
                },
                AsrWord {
                    text: "happened".into(),
                    start_ms: 9_783,
                    end_ms: 9_970,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        assert!(chat.to_chat().contains("\u{15}9443_9970\u{15}"));
        assert!(!chat.to_chat().contains("\u{15}2000_9970\u{15}"));
    }

    #[test]
    fn fa_rerun_preserves_start_for_untimed_leading_filler() {
        const LEADING_FILLER_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n\
*PAR:\t&-um happened . \u{15}2000_9970\u{15}\n\
%wor:\t&-um happened .\n@End\n";
        let mut chat = Chat::parse(
            LEADING_FILLER_CHAT,
            SourceId::try_new("leading-filler.cha").expect("source id"),
        )
        .expect("parse fixture");
        let aligned = vec![AsrSegment {
            start_ms: 2_000,
            end_ms: 9_970,
            text: "um happened".into(),
            speaker: None,
            words: vec![
                AsrWord {
                    text: "um".into(),
                    start_ms: 0,
                    end_ms: 0,
                    confidence: None,
                },
                AsrWord {
                    text: "happened".into(),
                    start_ms: 9_443,
                    end_ms: 9_970,
                    confidence: None,
                },
            ],
        }];

        inject_word_timings(&mut chat, &aligned).expect("inject timed result");

        let bullet = chat
            .ast()
            .lines
            .0
            .iter()
            .find_map(|line| match line {
                Line::Utterance(utterance) => utterance.main.content.bullet.as_ref(),
                _ => None,
            })
            .expect("main bullet");
        assert_eq!(bullet.timing.start_ms, 2_000);
        assert_eq!(bullet.timing.end_ms, 9_970);
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
