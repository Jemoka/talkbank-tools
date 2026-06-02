//! `UtrTaskRunner` — Utterance Timing Recovery.
//!
//! UTR runs an ASR-shaped backend over the whole audio, then Hirschberg-
//! aligns the CHAT word stream against the ASR token stream to inject
//! per-utterance bullet timings on transcripts that arrived without any.
//! It is a pre-pass for FA on hand-authored or post-edited transcripts.
//!
//! ## Pipeline shape
//!
//! 1. **Skip-when-already-timed.** Walk `chat.ast()`; if every utterance
//!    has a non-zero bullet, emit `StageSkipped` and return. This makes
//!    `Task::Fa::requires() = &[…, Task::Utr]` safe for callers whose
//!    CHAT is already timed (manual transcripts with bullets, FA reruns,
//!    UtSeg output).
//! 2. **Resolve audio.** Reuses the same sibling-audio fallback FA uses.
//! 3. **Dispatch.** Sends `TaskInput::Utr(UtrInput)` (a serde-transparent
//!    newtype over `AsrInput`) to the registered UTR backend. Python ASR
//!    backends opt into UTR by adding the `UTR` marker mixin; their
//!    `call(batch)` sees an `AsrInput`-shaped payload and runs unchanged.
//! 4. **Convert to timing tokens.** Flattens `AsrOutput.segments` into a
//!    single `Vec<AsrTimingToken>` and filters zero-duration tokens at
//!    20ms resolution (parity with tbtbt's `asr_response_to_utr_tokens`).
//! 5. **Strategy.** `select_strategy(chat, None)` picks GlobalUtr or
//!    TwoPassOverlap based on CA / `+<` markers.
//! 6. **Inject bullets** via `Bullet::utr_hint` so downstream FA
//!    overwrites them rather than union-expanding.
//! 7. **Audit.** Writes `%xalign` dependent tiers for zero-duration-
//!    skipped and unmatched utterances via
//!    `talkbank_transform::decisions::inject_decision_tiers`.
//! 8. **Stamp provenance.**

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
use crate::proto::asr::{AsrInput, AsrOptions, AsrOutput, LanguageSpec};
use crate::proto::utr::UtrInput;
use crate::utils::{BAError, BAResult, MediaInput, SourceId, prepare_pcm, stamp_provenance};
use async_trait::async_trait;
use smol_str::SmolStr;
use std::path::Path;
use talkbank_model::Line;
use talkbank_transform::decisions::{ReviewLevel, inject_decision_tiers};

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

/// `true` when every utterance carries a non-zero bullet — the cheap
/// short-circuit that makes UTR safe to include in any pipeline.
fn all_utterances_already_timed(chat: &Chat) -> bool {
    for line in chat.ast().lines.0.iter() {
        if let Line::Utterance(u) = line {
            match u.main.content.bullet.as_ref() {
                Some(b) if b.timing.start_ms < b.timing.end_ms => continue,
                _ => return false,
            }
        }
    }
    // A file with no utterances has nothing to recover — treat as timed.
    true
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

        if all_utterances_already_timed(chat) {
            sink.emit(ProgressEvent {
                source_id: chat.source_id().clone(),
                task: Some(Task::Utr),
                kind: ProgressKind::StageSkipped,
                completed: 0,
                total: 0,
                label: "all utterances already timed".into(),
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
            // ReviewLevel::All — UTR's audit tier should reflect every
            // skipped / unmatched utterance for downstream review.
            inject_decision_tiers(chat.ast_mut(), &result.decisions, ReviewLevel::All);
        }

        progress.finish();

        let engine = dispatcher.engine_name(Task::Utr);
        stamp_provenance(&mut chat.ast_mut().lines.0, engine.as_deref());

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
