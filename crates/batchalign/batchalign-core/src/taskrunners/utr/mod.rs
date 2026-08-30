//! `UtrTaskRunner` — Utterance Timing Recovery: pre-pass for FA on
//! fully-untimed CHATs.
//!
//! Decodes the transcript's sibling audio via `crate::utils::prepare_pcm`,
//! dispatches a `TaskInput::Utr` (a serde-transparent newtype over
//! `AsrInput`) to whichever ASR backend has opted into the `UTR` marker,
//! converts the returned `AsrOutput` segments to a flat
//! `Vec<AsrTimingToken>` stream (with a 20 ms zero-duration filter), and
//! runs the validated global Hirschberg-DP strategy to inject
//! `BulletSource::Utr` utterance bullets on every untimed utterance.
//! Decision provenance remains available for opt-in `%xalign` tiers.
//!
//! Behavioural parity targets the tbtbt UTR stack:
//! - Strategy core: `tbtbt/crates/batchalign/src/chat_ops/fa/utr.rs`
//!   (GlobalUtr — flatten + exact-subseq fast path + Hirschberg fallback +
//!   monotonicity post-pass).
//! - Overlap-aware variant: `chat_ops/fa/utr/two_pass.rs` +
//!   `chat_ops/fa/utr/overlap_markers.rs` (TwoPassOverlapUtr — excludes
//!   `+<` / `⌊`-bearing utterances from pass 1, recovers their timing in
//!   pass 2 via predecessor-window adaptive search).
//! - Token preparation: `runner/dispatch/utr.rs::asr_response_to_utr_tokens`
//!   (20 ms zero-duration filter — Whisper's DTW grid produces them for
//!   short backchannels and they break the DP).
//!
//! The Hirschberg DP itself is the shared
//! `talkbank_transform::dp_align::align` (same Hirschberg implementation
//! tbtbt uses); we did not port tbtbt's local copy. Sample-rate
//! normalization to 16 kHz mono happens at the audio-prep boundary
//! (`utils::prepare_pcm`) so every UTR backend sees the same waveform
//! shape FA receives.
//!
//! ## Pipeline shape
//!
//! 1. **Skip-when-any-already-timed.** Walk `chat.ast()`; if *any*
//!    utterance has a non-zero bullet, emit `StageSkipped` and return.
//!    UTR is meant for fully untimed transcripts; partially-timed
//!    files are handled by FA + interpolation, and running UTR on them
//!    would risk overwriting hand-set bullets with weaker ASR-derived
//!    ones. This makes `Task::Fa::requires() = &[…, Task::Utr]` safe
//!    for any pipeline.
//! 2. **Resolve audio.** Reuses the same sibling-audio fallback FA uses.
//! 3. **Dispatch.** Sends `TaskInput::Utr(UtrInput)` to the registered
//!    UTR backend. Python ASR backends opt in by adding the `UTR` marker
//!    mixin; their `call(batch)` sees an `AsrInput`-shaped payload and
//!    runs unchanged.
//! 4. **Convert to timing tokens** with the 20 ms zero-duration filter.
//! 5. **Strategy.** `select_strategy(chat, None)` uses GlobalUtr. The
//!    experimental two-pass overlap implementation remains available for a
//!    future explicit opt-in but is not selected automatically.
//! 6. **Inject bullets** via `Bullet::utr_hint` so downstream FA
//!    overwrites them rather than union-expanding.
//! 7. **Audit.** Records decisions for zero-duration-skipped and unmatched
//!    utterances without adding experimental review tiers by default.
//! 8. **Clear `, unlinked`** from the `@Media` header — UTR just injected
//!    bullets so the E544-mandated `unlinked` status is now stale.
//! 9. **Stamp provenance** with the registered UTR backend name.

pub mod extraction;
pub mod overlap_markers;
pub mod strategy;
pub mod two_pass;

pub use strategy::{
    AsrTimingToken, GlobalUtr, UtrResult, UtrStrategy, inject_utr_timing, select_strategy,
};
pub use two_pass::{
    CaMarkerPolicy, GroupingContext, TwoPassConfig, TwoPassOverlapUtr, UtrMatchMode,
};

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressKind;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::decisions::{ReviewLevel, inject_decision_tiers};
use crate::proto::asr::{AsrInput, AsrOptions, AsrOutput, LanguageSpec};
use crate::proto::utr::UtrInput;
use crate::utils::{BAError, BAResult, MediaInput, SourceId, clear_media_unlinked, prepare_pcm};
use async_trait::async_trait;
use smol_str::SmolStr;
use std::path::Path;
use talkbank_model::Line;

/// Audio extensions to probe for a CHAT file's sibling media, in priority
/// order. Same list as the FA runner.
const SIBLING_AUDIO_EXTS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "aac", "wma", "mov", "avi", "mpg", "mpeg",
];

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

/// `true` when *any* utterance carries a non-zero bullet.
///
/// UTR is intended for fully untimed transcripts. If even a single
/// utterance already has a bullet, the file is partially timed —
/// downstream FA / interpolation handles those without re-aligning the
/// whole file via ASR. Running UTR on a partially-timed file would also
/// risk overwriting hand-set bullets with weaker ASR-derived ones.
fn any_utterance_already_timed(chat: &Chat) -> bool {
    for line in chat.ast().lines.as_slice().iter() {
        if let Line::Utterance(u) = line {
            if let Some(b) = u.main.content.bullet.as_ref() {
                if b.timing.start_ms < b.timing.end_ms {
                    return true;
                }
            }
        }
    }
    false
}

/// Resolve the chat's `@Languages:` header into a concrete `LanguageSpec`.
fn resolve_per_file_language(chat: &Chat) -> LanguageSpec {
    if let Some(code) = chat.primary_language() {
        LanguageSpec::Code(SmolStr::new(code))
    } else {
        LanguageSpec::PerFile
    }
}

/// Convert an ASR backend response into the flat token stream UTR
/// strategies consume. Filters zero-duration tokens at 20ms resolution
/// (Whisper's DTW grid produces a lot of these for short backchannels
/// and they break the Hirschberg alignment). Parity with tbtbt's
/// `runner/dispatch/utr.rs::asr_response_to_utr_tokens`.
fn asr_output_to_utr_tokens(out: &AsrOutput) -> Vec<AsrTimingToken> {
    const ZERO_DURATION_TOLERANCE_MS: u64 = 20;
    let mut tokens = Vec::new();
    for seg in &out.segments {
        for w in &seg.words {
            if w.end_ms <= w.start_ms || w.end_ms - w.start_ms < ZERO_DURATION_TOLERANCE_MS {
                continue;
            }
            tokens.push(AsrTimingToken {
                text: w.text.clone(),
                start_ms: w.start_ms,
                end_ms: w.end_ms,
            });
        }
    }
    tokens
}

pub struct UtrTaskRunner;

#[async_trait]
impl TaskRunner for UtrTaskRunner {
    const TASK: Task = Task::Utr;

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
                    "UtrTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        if any_utterance_already_timed(chat) {
            // Emit StageStarted+StageInjected (not StageSkipped) so the
            // TUI bridge treats UTR as a clean no-op rather than a
            // terminal per-file skip. StageSkipped is a terminal state
            // in the TUI Task state machine (see
            // `python/batchalign/cli/tui/task.py`) — emitting it here
            // would lock the file's display at "skip" and prevent FA's
            // StageStarted from advancing the task, even though the
            // Rust pipeline continues running FA correctly on disk.
            sink.emit(ProgressEvent::stage_started(chat.source_id(), Task::Utr));
            sink.emit(ProgressEvent {
                source_id: chat.source_id().clone(),
                task: Some(Task::Utr),
                kind: ProgressKind::StageInjected,
                completed: 0,
                total: 0,
                label: "nothing to recover — all utterances already timed".into(),
            });
            return Ok(());
        }

        sink.emit(ProgressEvent::stage_started(chat.source_id(), Task::Utr));

        let media = match chat.media().cloned() {
            Some(m) => m,
            None => sibling_media(chat.source_id()).ok_or_else(|| {
                BAError::Internal(
                    "UtrTaskRunner: chat has no attached media and no sibling audio file found"
                        .into(),
                )
            })?,
        };

        let audio =
            prepare_pcm(&media).map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let asr_input = AsrInput {
            source_id: chat.source_id().clone(),
            audio,
            language: resolve_per_file_language(chat),
            options: AsrOptions::default(),
        };
        let utr_input = UtrInput::from(asr_input);

        let source_id = chat.source_id().clone();
        let progress = std::sync::Arc::new(crate::base::ScaledProgress::new(
            sink.clone(),
            source_id.clone(),
            Task::Utr,
            1,
        ));
        let progress_dyn: std::sync::Arc<dyn crate::base::BackendProgress> = progress.clone();
        progress.start_step();
        let output_raw = dispatcher
            .dispatch_with_progress(TaskInput::Utr(utr_input), progress_dyn)
            .await?;
        let output: crate::proto::utr::UtrOutput = output_raw.try_into()?;
        let asr_output: AsrOutput = output.into_asr();

        let tokens = asr_output_to_utr_tokens(&asr_output);

        let strategy = select_strategy(chat.ast(), None);
        let result = strategy.inject(chat.ast_mut(), &tokens);

        if !result.decisions.is_empty() {
            inject_decision_tiers(chat.ast_mut(), &result.decisions, ReviewLevel::default());
        }

        progress.finish();

        // We just injected bullets — clear `, unlinked` from any
        // `@Media` header so the now-linked state is reflected on disk.
        clear_media_unlinked(chat.ast_mut().lines.as_mut_slice());

        // Provenance `@Comment` stamping happens once at end-of-pipeline in
        // `batchalign_engine::pipeline::run_one` rather than per-runner.

        sink.emit(ProgressEvent {
            source_id: chat.source_id().clone(),
            task: Some(Task::Utr),
            kind: ProgressKind::StageInjected,
            completed: 0,
            total: 0,
            label: format!(
                "injected={} skipped={} unmatched={}",
                result.injected, result.skipped, result.unmatched
            ),
        });
        Ok(())
    }
}
