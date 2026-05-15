//! Transport adapter for forced-alignment worker inference.
//!
//! The FA pipeline delegates worker interaction through this module so the
//! orchestration code can ask for "timings for these miss groups" without
//! depending on the concrete worker-protocol V2 request-building details.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::api::{DurationMs, WorkerLanguage};
use crate::chat_ops::fa::{FaEngineType, FaGroup, FaInferItem, FaTimingMode, WordTiming};
use crate::error::ServerError;
use crate::pipeline::PipelineServices;
use crate::types::traces::FaFallbackEventTrace;
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::fa_result_v2::parse_forced_alignment_result_v2;
use crate::worker::request_builder_v2::{
    ForcedAlignmentBuildInputV2, ForcedAlignmentRequestBuildErrorV2, PreparedFaRequestIdsV2,
    build_forced_alignment_request_v2,
};
use tracing::warn;

static NEXT_FA_REQUEST_NAMESPACE: AtomicU64 = AtomicU64::new(1);

/// Shared FA worker batch input independent of the concrete worker transport.
pub(crate) struct FaWorkerBatch<'a> {
    /// Precomputed cleaned word texts keyed by group index.
    pub word_texts: &'a [Vec<String>],
    /// FA groups for the current file.
    pub groups: &'a [FaGroup],
    /// Indices of groups that still need worker inference.
    pub miss_indices: &'a [usize],
    /// Source audio path for the current file.
    pub audio_path: &'a Path,
    /// Worker-runtime language hint for FA model bootstrap.
    pub worker_lang: WorkerLanguage,
    /// FA backend selected by the Rust control plane.
    pub engine: FaEngineType,
    /// Timing mode selected by the Rust control plane.
    pub timing_mode: FaTimingMode,
}

/// Parsed FA timings for one inferred miss group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FaWorkerGroupResult {
    /// Original group index inside the current file.
    pub group_index: usize,
    /// Parsed timings in the established Rust FA timing domain.
    pub timings: Vec<Option<WordTiming>>,
    /// Fallback event metadata when this group had to retry with another engine.
    pub fallback_event: Option<FaFallbackEventTrace>,
}

/// Narrow transport adapter for FA worker inference.
#[derive(Clone, Copy)]
pub(crate) enum FaWorkerTransport<'a> {
    /// Live typed worker-protocol V2 transport using prepared artifacts.
    V2 {
        /// Shared pipeline services with worker-pool access.
        services: PipelineServices<'a>,
    },
}

impl<'a> FaWorkerTransport<'a> {
    /// Return the production FA worker transport.
    pub(crate) fn production(services: PipelineServices<'a>) -> Self {
        Self::V2 { services }
    }

    /// Infer timings for the requested FA miss groups.
    pub(crate) async fn infer_groups(
        self,
        batch: FaWorkerBatch<'_>,
    ) -> Result<Vec<FaWorkerGroupResult>, ServerError> {
        match self {
            Self::V2 { services } => infer_groups_v2(services, batch).await,
        }
    }
}

/// Dispatch staged worker-protocol V2 requests for each FA miss group and
/// parse successful results back into the established Rust timing domain.
async fn infer_groups_v2(
    services: PipelineServices<'_>,
    batch: FaWorkerBatch<'_>,
) -> Result<Vec<FaWorkerGroupResult>, ServerError> {
    let request_namespace = NEXT_FA_REQUEST_NAMESPACE.fetch_add(1, Ordering::Relaxed);
    let artifacts = PreparedArtifactRuntimeV2::new("fa_v2").map_err(|error| {
        ServerError::Validation(format!("failed to create FA V2 artifact runtime: {error}"))
    })?;

    let mut parsed_results = Vec::with_capacity(batch.miss_indices.len());
    for group_index in batch.miss_indices.iter().copied() {
        let group = &batch.groups[group_index];
        let response = match dispatch_group_request(
            services,
            &artifacts,
            &batch,
            request_namespace,
            group_index,
            batch.engine,
        )
        .await
        {
            Ok(response) => response,
            Err(ServerError::EmptyFaAudioSegment {
                start_ms,
                end_ms,
                ref path,
            }) => {
                // The FA group's audio window is past the end of the file.
                // Leave this group's words unaligned rather than failing the
                // whole file — the transcript is still useful without timing.
                warn!(
                    group = group_index,
                    start_ms = start_ms.0,
                    end_ms = end_ms.0,
                    path,
                    "FA group has no audio (segment past end of file); leaving words unaligned"
                );
                parsed_results.push(unaligned_group_result(group_index, group));
                continue;
            }
            // A worker process crash (SIGKILL / OOM / C-extension SIGSEGV) is
            // caused by this group's specific audio + word content — not by a
            // broken environment.  The crash is deterministic on the same group:
            // retrying with a fresh worker on the same input reproduces it.
            // Other groups have different audio and words and are unaffected.
            //
            // The correct recovery is a group-level skip (leave words unaligned,
            // continue to the next group) rather than a file-level abort.  A
            // file abort followed by FA retry just respawns workers that crash
            // on the same group — the retry loop in `fa_pipeline.rs` confirmed
            // this for job 1020067a-85f (3 files × 3 retries, all within 4 s).
            Err(ref e) if is_worker_process_crash(e) => {
                warn!(
                    group = group_index,
                    start_ms = group.audio_start_ms(),
                    end_ms = group.audio_end_ms(),
                    error = %e,
                    "Worker process crash on FA group (signal/OOM); leaving words unaligned"
                );
                parsed_results.push(unaligned_group_result(group_index, group));
                continue;
            }
            Err(other) => return Err(other),
        };

        match parse_group_response(&response, group_index, group, batch.timing_mode) {
            Ok(parsed) => parsed_results.push(parsed),
            Err(error) => {
                let Some(reason) = whisper_fallback_reason(batch.engine, &error) else {
                    // Before propagating to the file level, check whether this is a
                    // data-driven RuntimeFailure. RuntimeFailure means the model failed
                    // on this group's specific input — other groups are unaffected, so
                    // the correct recovery is a group-level skip, not a file abort.
                    if is_fa_runtime_failure(&error) {
                        warn!(
                            group = group_index,
                            start_ms = group.audio_start_ms(),
                            end_ms = group.audio_end_ms(),
                            error = %error,
                            "FA group failed with model RuntimeFailure (data-driven); \
                             leaving words unaligned"
                        );
                        parsed_results.push(unaligned_group_result(group_index, group));
                        continue;
                    }
                    return Err(error);
                };
                warn!(
                    group = group_index,
                    start_ms = group.audio_start_ms(),
                    end_ms = group.audio_end_ms(),
                    reason,
                    "Wave2Vec FA hit recoverable target constraint; retrying group with Whisper FA"
                );
                let fallback_namespace = NEXT_FA_REQUEST_NAMESPACE.fetch_add(1, Ordering::Relaxed);
                let fallback_response = dispatch_group_request(
                    services,
                    &artifacts,
                    &batch,
                    fallback_namespace,
                    group_index,
                    FaEngineType::WhisperFa,
                )
                .await?;
                match parse_group_response(
                    &fallback_response,
                    group_index,
                    group,
                    batch.timing_mode,
                ) {
                    Ok(parsed) => {
                        parsed_results.push(parsed.with_fallback_event(build_fallback_event(
                            group_index,
                            group,
                            batch.engine,
                            FaEngineType::WhisperFa,
                            reason,
                        )))
                    }
                    // The Whisper model is not loaded in this worker (capability
                    // gap, not a data error).  Leave the group's words unaligned
                    // rather than aborting the whole file — the surrounding
                    // utterances still have valid timing.
                    Err(ref error) if is_whisper_model_unavailable(error) => {
                        warn!(
                            group = group_index,
                            start_ms = group.audio_start_ms(),
                            end_ms = group.audio_end_ms(),
                            "Whisper FA unavailable (worker has no Whisper model loaded); \
                             leaving group words unaligned"
                        );
                        parsed_results.push(unaligned_group_result(group_index, group));
                    }
                    // The Whisper fallback itself hit a data-driven RuntimeFailure
                    // (e.g. the group is still too long for Whisper's CTC context
                    // after the Wave2Vec → Whisper retry).  Same treatment: leave
                    // the group unaligned rather than aborting the file.
                    Err(ref error) if is_fa_runtime_failure(error) => {
                        warn!(
                            group = group_index,
                            start_ms = group.audio_start_ms(),
                            end_ms = group.audio_end_ms(),
                            error = %error,
                            "Whisper FA fallback also failed with model RuntimeFailure; \
                             leaving group words unaligned"
                        );
                        parsed_results.push(unaligned_group_result(group_index, group));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }

    Ok(parsed_results)
}

/// Construct a group result with every word timing left unaligned (`None`).
///
/// All group-level skip paths (empty audio, model unavailability, data-driven
/// RuntimeFailure) return this same shape so the orchestrator can continue to
/// the next group without aborting the file.
fn unaligned_group_result(group_index: usize, group: &FaGroup) -> FaWorkerGroupResult {
    FaWorkerGroupResult {
        group_index,
        timings: vec![None; group.words.len()],
        fallback_event: None,
    }
}

async fn dispatch_group_request(
    services: PipelineServices<'_>,
    artifacts: &PreparedArtifactRuntimeV2,
    batch: &FaWorkerBatch<'_>,
    request_namespace: u64,
    group_index: usize,
    engine: FaEngineType,
) -> Result<crate::types::worker_v2::ExecuteResponseV2, ServerError> {
    let infer_item = build_fa_infer_item(batch, group_index);
    let request_ids = build_fa_request_ids(request_namespace, group_index);
    let request = build_forced_alignment_request_v2(
        artifacts.store(),
        ForcedAlignmentBuildInputV2 {
            ids: &request_ids,
            infer_item: &infer_item,
            engine,
        },
    )
    .await
    .map_err(|error| match error {
        // Empty audio is a skip signal, not a fatal failure.  Propagate as a
        // dedicated error so the caller can leave the group unaligned instead
        // of failing the whole file.
        ForcedAlignmentRequestBuildErrorV2::EmptyAudioSegment {
            path,
            start_ms,
            end_ms,
        } => ServerError::EmptyFaAudioSegment {
            path,
            start_ms: DurationMs(start_ms),
            end_ms: DurationMs(end_ms),
        },
        other => ServerError::Validation(format!(
            "failed to build worker protocol V2 FA request for group {group_index}: {other}"
        )),
    })?;

    services
        .pool
        .dispatch_execute_v2(&batch.worker_lang, &request)
        .await
        .map_err(ServerError::Worker)
}

fn parse_group_response(
    response: &crate::types::worker_v2::ExecuteResponseV2,
    group_index: usize,
    group: &FaGroup,
    timing_mode: FaTimingMode,
) -> Result<FaWorkerGroupResult, ServerError> {
    let timings = parse_forced_alignment_result_v2(
        response,
        &group.words,
        DurationMs(group.audio_start_ms()),
        timing_mode,
    )
    .map_err(|error| {
        ServerError::Validation(format!(
            "failed to parse worker protocol V2 FA response for group {group_index} ({}..{} ms): {error}",
            group.audio_start_ms(),
            group.audio_end_ms(),
        ))
    })?;

    Ok(FaWorkerGroupResult {
        group_index,
        timings,
        fallback_event: None,
    })
}

impl FaWorkerGroupResult {
    fn with_fallback_event(mut self, fallback_event: FaFallbackEventTrace) -> Self {
        self.fallback_event = Some(fallback_event);
        self
    }
}

fn build_fallback_event(
    group_index: usize,
    group: &FaGroup,
    from_engine: FaEngineType,
    to_engine: FaEngineType,
    reason: &str,
) -> FaFallbackEventTrace {
    FaFallbackEventTrace {
        group_index,
        from_engine: fa_engine_name(from_engine).to_string(),
        to_engine: fa_engine_name(to_engine).to_string(),
        reason: reason.to_string(),
        audio_start_ms: DurationMs(group.audio_start_ms()),
        audio_end_ms: DurationMs(group.audio_end_ms()),
    }
}

fn fa_engine_name(engine: FaEngineType) -> &'static str {
    match engine {
        FaEngineType::Wave2Vec => "wave2vec",
        FaEngineType::WhisperFa => "whisper-fa",
    }
}

/// Returns `true` when the FA group error is a data-driven `RuntimeFailure` —
/// the worker received and understood the request but the model raised a Python
/// exception on this specific input (token overflow, shape mismatch, OOM, etc.).
///
/// # Why this is always group-local
///
/// A `RuntimeFailure` means the worker successfully parsed the request and
/// attempted inference, then the model failed. The failure is caused by the
/// content of *this* group's words and audio. Other groups have different
/// words and different audio; they will not trigger the same failure. The error
/// is therefore inherently group-scoped and should be demoted to a group-level
/// warning (leave words unaligned, continue) rather than propagating as a
/// file-level failure.
///
/// # Contrast with infrastructure failures
///
/// `ProcessExited` (worker crash) and `Protocol` (IPC deserialization failure)
/// are not data-driven: if the worker crashed or the protocol is broken, every
/// subsequent call will also fail. Those errors still propagate to the file
/// level so the retry loop and fallback UTR path can attempt recovery.
///
/// # Detection
///
/// The substring `"RuntimeFailure:"` is inserted by
/// `parse_forced_alignment_result_v2()` when formatting a
/// `ProtocolErrorCodeV2::RuntimeFailure` response from the Python worker.
/// It does not appear in `ModelUnavailable`, `Protocol`, or IPC parse errors,
/// so the match is specific to data-driven model failures.
fn is_fa_runtime_failure(error: &ServerError) -> bool {
    matches!(
        error,
        ServerError::Validation(msg) if msg.contains("RuntimeFailure:")
    )
}

/// Returns `true` when `error` is a worker process crash — the Python child
/// process was killed by a signal (`exit code: None` = SIGKILL from the kernel
/// OOM-killer, or SIGSEGV/SIGABRT from a C-extension crash in torchaudio).
///
/// # Why this is always group-local
///
/// A process crash is triggered by the *content* of the request: a specific
/// combination of audio length and word count can push the Wave2Vec or Whisper
/// model into OOM or cause a C-extension assertion failure.  The crash is
/// deterministic on the same group — retrying with a fresh worker on the same
/// group will produce the same crash.  Other groups have different audio and
/// words; they will not trigger the crash and can be processed normally by the
/// replacement worker that the pool automatically spawns.
///
/// The correct recovery is therefore **group-level skip**, not file-level abort
/// followed by retry (which just respawns workers that crash on the same group).
///
/// # Contrast with infrastructure failures
///
/// `WorkerError::SpawnFailed`, `Protocol`, and `NoWorker` are environmental
/// failures that affect every subsequent dispatch.  Those still propagate to
/// the file level so the retry loop and fallback UTR path can attempt recovery.
/// `ProcessExited` is different: it is caused by this specific group's input,
/// not by a broken environment.
fn is_worker_process_crash(error: &ServerError) -> bool {
    matches!(
        error,
        ServerError::Worker(crate::worker::error::WorkerError::ProcessExited { .. })
    )
}

/// Returns true when `error` was produced because the worker that handled the
/// Whisper FA fallback request does not have a Whisper FA model loaded.
///
/// This is a **worker capability gap**, not a data quality failure.  When this
/// is true the FA group should be left unaligned (same as an empty audio
/// segment) rather than propagating a file-level error.
///
/// The distinctive substring comes from the Python worker's
/// `execute_forced_alignment_request_v2` in `crates/batchalign-pyo3/src/worker_fa_exec.rs`,
/// which returns `ProtocolErrorCodeV2::ModelUnavailable` with message
/// `"no whisper FA host loaded for worker protocol V2"` when
/// `whisper_runner` is `None`.  `parse_forced_alignment_result_v2` formats
/// this as `"… with ModelUnavailable: no whisper FA host loaded …"`.
fn is_whisper_model_unavailable(error: &ServerError) -> bool {
    matches!(
        error,
        ServerError::Validation(msg) if msg.contains("ModelUnavailable: no whisper FA host loaded")
    )
}

fn whisper_fallback_reason(engine: FaEngineType, error: &ServerError) -> Option<&'static str> {
    if engine != FaEngineType::Wave2Vec {
        return None;
    }

    match error {
        ServerError::Validation(message)
            if message.contains("targets length is too long for CTC") =>
        {
            Some("targets length is too long for CTC")
        }
        ServerError::Validation(message)
            if message.contains("targets Tensor shouldn't contain blank index") =>
        {
            Some("targets Tensor shouldn't contain blank index")
        }
        // Wave2Vec MMS_FA has 7 conv layers (kernels [10,3,3,3,3,2,2], strides
        // [5,2,2,2,2,2,2]).  A group shorter than ~400 samples (25ms @ 16 kHz)
        // can produce fewer than 2 samples after layer 6 so layer 7 (kernel=2)
        // crashes with "Kernel size can't be greater than actual input size".
        // Whisper pads all input to 30 seconds before computing features and
        // can handle any non-zero audio length.
        ServerError::Validation(message)
            if message.contains("Kernel size can't be greater than actual input size") =>
        {
            Some("audio segment too short for Wave2Vec feature extractor")
        }
        _ => None,
    }
}

/// Build one production-domain `FaInferItem` from the transport-neutral batch
/// view.
fn build_fa_infer_item(batch: &FaWorkerBatch<'_>, group_index: usize) -> FaInferItem {
    let group = &batch.groups[group_index];
    FaInferItem {
        words: batch.word_texts[group_index].clone(),
        word_ids: group.words.iter().map(|word| word.stable_id()).collect(),
        word_utterance_indices: group
            .words
            .iter()
            .map(|word| word.utterance_index.raw())
            .collect(),
        word_utterance_word_indices: group
            .words
            .iter()
            .map(|word| word.utterance_word_index.raw())
            .collect(),
        audio_path: batch.audio_path.to_string_lossy().into_owned(),
        audio_start_ms: group.audio_start_ms(),
        audio_end_ms: group.audio_end_ms(),
        timing_mode: batch.timing_mode,
    }
}

/// Build unique request and artifact ids for one FA V2 request.
///
/// The request namespace is allocated once per `infer_groups_v2` call so two
/// concurrent files cannot collide on `fa-v2-request-0`, `fa-v2-request-1`,
/// and so on while sharing the same GPU worker.
fn build_fa_request_ids(request_namespace: u64, group_index: usize) -> PreparedFaRequestIdsV2 {
    PreparedFaRequestIdsV2::new(
        format!("fa-v2-request-{request_namespace}-{group_index}"),
        format!("fa-v2-payload-{request_namespace}-{group_index}"),
        format!("fa-v2-audio-{request_namespace}-{group_index}"),
    )
}

#[cfg(test)]
mod tests {
    use crate::chat_ops::fa::{FaWord, TimeSpan};
    use crate::chat_ops::{UtteranceIdx, WordIdx};

    use super::*;
    use crate::api::DurationSeconds;
    use crate::types::worker_v2::{
        ExecuteOutcomeV2, ExecuteResponseV2, TaskResultV2, TranslationItemResultV2,
        TranslationResultV2, WorkerRequestIdV2,
    };
    use crate::worker::error::WorkerError;

    /// Build a small FA word for transport unit tests.
    fn make_word(index: usize, text: &str) -> FaWord {
        FaWord {
            utterance_index: UtteranceIdx(0),
            utterance_word_index: WordIdx(index),
            text: text.into(),
        }
    }

    #[test]
    fn builds_fa_infer_item_from_transport_neutral_batch() {
        let word_texts = vec![vec!["hello".to_string(), "world".to_string()]];
        let groups = vec![FaGroup {
            audio_span: TimeSpan::new(100, 900),
            words: vec![make_word(0, "hello"), make_word(1, "world")],
            utterance_indices: vec![UtteranceIdx(0)],
        }];
        let batch = FaWorkerBatch {
            word_texts: &word_texts,
            groups: &groups,
            miss_indices: &[0],
            audio_path: Path::new("/tmp/input.wav"),
            worker_lang: WorkerLanguage::from(crate::api::LanguageCode3::eng()),
            engine: FaEngineType::WhisperFa,
            timing_mode: FaTimingMode::WithPauses,
        };

        let item = build_fa_infer_item(&batch, 0);
        assert_eq!(item.words, vec!["hello".to_string(), "world".to_string()]);
        assert_eq!(
            item.word_ids,
            vec!["u0:w0".to_string(), "u0:w1".to_string()]
        );
        assert_eq!(item.word_utterance_indices, vec![0, 0]);
        assert_eq!(item.word_utterance_word_indices, vec![0, 1]);
        assert_eq!(item.audio_path, "/tmp/input.wav");
        assert_eq!(item.audio_start_ms, 100);
        assert_eq!(item.audio_end_ms, 900);
        assert_eq!(item.timing_mode, FaTimingMode::WithPauses);
    }

    #[test]
    fn builds_namespaced_v2_request_ids_from_group_index() {
        let ids = build_fa_request_ids(42, 7);
        assert_eq!(&*ids.request_id, "fa-v2-request-42-7");
        assert_eq!(&*ids.payload_ref_id, "fa-v2-payload-42-7");
        assert_eq!(&*ids.audio_ref_id, "fa-v2-audio-42-7");
    }

    #[test]
    fn namespaces_v2_request_ids_across_concurrent_files() {
        let first = build_fa_request_ids(1, 0);
        let second = build_fa_request_ids(2, 0);

        assert_ne!(first.request_id, second.request_id);
        assert_ne!(first.payload_ref_id, second.payload_ref_id);
        assert_ne!(first.audio_ref_id, second.audio_ref_id);
    }

    #[test]
    fn parse_group_response_reports_parser_failure_with_group_context() {
        let group = FaGroup {
            audio_span: TimeSpan::new(100, 900),
            words: vec![make_word(0, "hello")],
            utterance_indices: vec![UtteranceIdx(0)],
        };
        let response = ExecuteResponseV2 {
            request_id: WorkerRequestIdV2::from("req-fa-v2-bad"),
            outcome: ExecuteOutcomeV2::Success,
            result: Some(TaskResultV2::TranslationResult(TranslationResultV2 {
                items: vec![TranslationItemResultV2 {
                    raw_translation: Some("hola".into()),
                    error: None,
                }],
            })),
            elapsed_s: DurationSeconds(0.01),
        };

        let error = parse_group_response(&response, 13, &group, FaTimingMode::Continuous)
            .expect_err("non-FA payload should fail immediately");

        assert!(
            error
                .to_string()
                .contains("failed to parse worker protocol V2 FA response for group 13")
        );
        assert!(error.to_string().contains("translation data"));
    }

    #[test]
    fn whisper_fallback_triggers_for_known_wave2vec_target_failures() {
        let overflow = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 13 (175765..176365 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             targets length is too long for CTC"
                .into(),
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::Wave2Vec, &overflow),
            Some("targets length is too long for CTC")
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::WhisperFa, &overflow),
            None
        );

        let blank_index = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 56 (754285..767165 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             ValueError: targets Tensor shouldn't contain blank index. Found tensor([[20, 5, 10, 10]])"
                .into(),
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::Wave2Vec, &blank_index),
            Some("targets Tensor shouldn't contain blank index")
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::WhisperFa, &blank_index),
            None
        );

        // Wave2Vec conv layer 7 (kernel=2) crashes when input shrinks to 1 sample.
        // Observed: group 27 (296480..296500 ms = 20 ms window = 320 samples),
        // job 2afcc302-bfd, file 28-NM-63-4.cha.
        let kernel_too_large = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 27 (296480..296500 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             RuntimeError: Calculated padded input size per channel: (1). \
             Kernel size: (2). Kernel size can't be greater than actual input size"
                .into(),
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::Wave2Vec, &kernel_too_large),
            Some("audio segment too short for Wave2Vec feature extractor")
        );
        assert_eq!(
            whisper_fallback_reason(FaEngineType::WhisperFa, &kernel_too_large),
            None
        );

        let other = ServerError::Validation("some other parse failure".into());
        assert_eq!(
            whisper_fallback_reason(FaEngineType::Wave2Vec, &other),
            None
        );
    }

    /// a user's bug (2026-04-08): `batchalign3 align` silently drops files
    /// when Wave2Vec falls back to Whisper FA but the worker has no Whisper
    /// model loaded.
    ///
    /// Repro:
    ///   batchalign3 align ~/ba_data/input ~/ba_data/output
    ///   → Job submitted for 4 files; 45-3.cha and 86-3.cha missing from output
    ///
    /// Server log:
    ///   Wave2Vec FA hit recoverable target constraint; retrying group with Whisper FA
    ///   FA error (raw): ModelUnavailable: no whisper FA host loaded for worker protocol V2
    ///
    /// Root cause: `infer_groups_v2` dispatches the Whisper fallback, but when
    /// the worker returns `ModelUnavailable` (because `whisper_runner = None`),
    /// `parse_group_response` wraps the error into a `ServerError::Validation`
    /// that looks identical to a fatal data error.  The `?` on the fallback call
    /// propagates it as a **file-level** failure instead of leaving the group
    /// unaligned the way empty audio segments are handled.
    ///
    /// Fix: implement `is_whisper_model_unavailable` so `infer_groups_v2` can
    /// detect the capability gap and leave the group with `None` timings.
    #[test]
    fn whisper_fallback_model_unavailable_is_detectable_as_capability_gap_not_data_error() {
        // This is the exact error produced in production:
        // parse_group_response wraps the ModelUnavailable worker response into
        // a ServerError::Validation with this message.
        let model_unavailable = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 24 (379515..381395 ms): \
             worker protocol V2 forced-alignment request failed with ModelUnavailable: \
             no whisper FA host loaded for worker protocol V2"
                .into(),
        );

        // The test asserts that is_whisper_model_unavailable can distinguish
        // this worker-capability error from an ordinary data-quality error.
        // Without this predicate, infer_groups_v2 has no way to leave the
        // group unaligned instead of killing the file.
        //
        // This assertion is currently RED: is_whisper_model_unavailable always
        // returns false (stub).  Implementing it makes the test GREEN.
        assert!(
            is_whisper_model_unavailable(&model_unavailable),
            "ModelUnavailable from Whisper fallback should be detectable \
             so infer_groups_v2 can leave the group unaligned"
        );

        // A plain data-quality error must NOT be mistaken for a capability gap.
        let data_error = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 5 (1000..2000 ms): \
             some parse error from the model output"
                .into(),
        );
        assert!(
            !is_whisper_model_unavailable(&data_error),
            "ordinary data errors must not be treated as ModelUnavailable"
        );
    }

    /// Any `RuntimeFailure` from the FA worker is data-driven: the model
    /// failed on *this group's* specific input. Other groups have different
    /// words and audio and will not trigger the same failure. These errors
    /// must be demoted to group-level skips, never propagated as file-level
    /// failures. This test verifies the detection predicate covers:
    ///
    /// - The exact 448-token overflow seen on `biling-data/DiazCollazos`
    /// - Generic RuntimeFailure variants (shape errors, OOM, etc.)
    /// - Does NOT fire on infrastructure errors (IPC parse, model unavailable)
    #[test]
    fn fa_runtime_failure_is_detectable_as_data_driven_group_error() {
        // Exact message from production: biling-data/DiazCollazos/09.cha, job ad9eb6ba,
        // group 0 (0..10970 ms), 2043 chars > 448 token limit.
        let overflow_448 = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 0 (0..10970 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             ValueError: Labels' sequence length 2043 cannot exceed the maximum \
             allowed length of 448 tokens."
                .into(),
        );
        assert!(
            is_fa_runtime_failure(&overflow_448),
            "448-token Whisper CTC overflow must be detectable as a RuntimeFailure"
        );

        // Generic RuntimeFailure (shape error, device mismatch, etc.) — also group-local.
        let shape_error = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 3 (50000..70000 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             RuntimeError: Expected all tensors to be on the same device"
                .into(),
        );
        assert!(
            is_fa_runtime_failure(&shape_error),
            "generic RuntimeFailure must also be detectable"
        );

        // Infrastructure error (IPC parse failure, no RuntimeFailure code) — must NOT match.
        let ipc_error = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 5 (0..5000 ms): \
             translation data (not FA)"
                .into(),
        );
        assert!(
            !is_fa_runtime_failure(&ipc_error),
            "non-RuntimeFailure IPC error must not be mistaken for a data-driven group error"
        );

        // ModelUnavailable — a capability gap, not a RuntimeFailure.
        let model_unavail = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 24 (379515..381395 ms): \
             worker protocol V2 forced-alignment request failed with ModelUnavailable: \
             no whisper FA host loaded for worker protocol V2"
                .into(),
        );
        assert!(
            !is_fa_runtime_failure(&model_unavail),
            "ModelUnavailable must not be mistaken for a data-driven RuntimeFailure"
        );
    }

    /// When Wave2Vec falls back to Whisper FA and Whisper itself hits a RuntimeFailure
    /// (e.g., the group is too long even for Whisper), that fallback error must also be
    /// detectable and demoted to a group-level skip rather than a file-level abort.
    #[test]
    fn fa_runtime_failure_from_whisper_fallback_is_detectable() {
        let whisper_overflow = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 7 (100000..111000 ms): \
             worker protocol V2 forced-alignment request failed with RuntimeFailure: \
             ValueError: Labels' sequence length 512 cannot exceed the maximum \
             allowed length of 448 tokens."
                .into(),
        );
        assert!(is_fa_runtime_failure(&whisper_overflow));
        // Must not be confused with the capability-gap path.
        assert!(!is_whisper_model_unavailable(&whisper_overflow));
    }

    /// Worker process crashes (SIGKILL, C-extension SIGSEGV) must be detectable
    /// as a group-local signal so `infer_groups_v2` can leave the crashing group
    /// unaligned and continue processing remaining groups rather than aborting
    /// the entire file.
    ///
    /// Root cause confirmed for job 1020067a-85f: one GPU worker (pid=12656)
    /// crashed with `exit code: None` (SIGKILL) while processing groups from 3
    /// concurrent files.  Every retry hit the same crashing group, died within
    /// 1–4 s, and all 3 files failed.  The fix: treat `ProcessExited` as a
    /// group-level skip, identical to `EmptyFaAudioSegment` and `RuntimeFailure`.
    #[test]
    fn worker_process_crash_is_detectable_as_group_local_signal() {
        // exit code: None = SIGKILL (kernel OOM-killer) or C-extension crash
        // that kills the process before Python's exception handling can run.
        let sigkill = ServerError::Worker(WorkerError::ProcessExited {
            code: None,
            stderr: None,
        });
        assert!(
            is_worker_process_crash(&sigkill),
            "SIGKILL (exit code None) must be detectable as a process crash"
        );

        // Explicit non-zero exit code (e.g., SIGSEGV = 139, SIGABRT = 134)
        // still classifies as a crash — the model died on this group's content.
        let sigsegv = ServerError::Worker(WorkerError::ProcessExited {
            code: Some(139),
            stderr: Some("Segmentation fault: 11".to_string()),
        });
        assert!(
            is_worker_process_crash(&sigsegv),
            "Non-zero exit code (SIGSEGV=139) must also be detectable as a process crash"
        );

        // Infrastructure failures that are NOT process crashes must not be
        // conflated — they should still propagate to the file level.
        let data_error = ServerError::Validation(
            "failed to parse worker protocol V2 FA response for group 5 (0..5000 ms): \
             some parse error from the model output"
                .into(),
        );
        assert!(
            !is_worker_process_crash(&data_error),
            "a Validation error must not be mistaken for a process crash"
        );
    }

    #[test]
    fn build_fallback_event_captures_group_and_engine_metadata() {
        let group = FaGroup {
            audio_span: TimeSpan::new(175_765, 176_365),
            words: vec![make_word(0, "hello")],
            utterance_indices: vec![UtteranceIdx(0)],
        };

        let event = build_fallback_event(
            13,
            &group,
            FaEngineType::Wave2Vec,
            FaEngineType::WhisperFa,
            "targets length is too long for CTC",
        );

        assert_eq!(event.group_index, 13);
        assert_eq!(event.from_engine, "wave2vec");
        assert_eq!(event.to_engine, "whisper-fa");
        assert_eq!(event.reason, "targets length is too long for CTC");
        assert_eq!(event.audio_start_ms.0, 175_765);
        assert_eq!(event.audio_end_ms.0, 176_365);
    }
}
