//! Forced alignment dispatch and per-file FA pipeline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::api::{DisplayPath, DurationMs, EngineVersion, LanguageCode3, NumWorkers, RevAiJobId};
use crate::cache::UtteranceCache;
use crate::options::{CommandOptions, EngineBackend as _};
use crate::params::{AudioContext, FaParams};
use crate::pipeline::PipelineServices;
use crate::runner::DispatchHostContext;
use crate::runner::debug_dumper::DebugDumper;
use crate::scheduling::{FailureCategory, WorkUnitKind};
use crate::types::results::FaResult;
use crate::worker::pool::WorkerPool;
use async_trait::async_trait;
use tracing::{info, warn};

use crate::store::{RunnerJobSnapshot, unix_now};
use crate::types::request::validate_utr_language_support;

use super::super::util::{
    FileRunTracker, FileStage, FileTaskOutcome, RunnerEventSink, compute_audio_identity,
    drain_supervised_file_tasks, get_audio_duration_ms, resolve_audio_for_chat_with_media_dir,
    spawn_progress_forwarder, spawn_supervised_file_task,
};
use super::FaDispatchPlan;
use super::audio_task::{AudioChatTask, run_audio_chat_file_task};
use super::utr::{UtrPassContext, run_utr_pass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignUtrDecision<'a> {
    SkipAllTimed,
    Run(&'a crate::options::UtrEngine),
    ProceedWithoutUtr,
}

fn plan_align_utr_stage<'a>(
    untimed: usize,
    file_lang: &LanguageCode3,
    utr_engine: Option<&'a crate::options::UtrEngine>,
) -> Result<AlignUtrDecision<'a>, crate::types::request::ValidationError> {
    if untimed == 0 {
        return Ok(AlignUtrDecision::SkipAllTimed);
    }

    match utr_engine {
        Some(engine) => {
            validate_utr_language_support(file_lang, engine)?;
            Ok(AlignUtrDecision::Run(engine))
        }
        None => Ok(AlignUtrDecision::ProceedWithoutUtr),
    }
}

/// Shared runtime dependencies for top-level FA dispatch.
///
/// The runner always passes this set together when it hands an `align` job to
/// the server-owned FA pipeline, so the bundle is the real boundary rather
/// than eight separate parameters.
pub(crate) struct FaDispatchRuntime {
    /// Worker pool used for typed V2 FA requests and any worker-owned UTR work.
    pub pool: Arc<WorkerPool>,
    /// Cache used by FA group reuse and worker result persistence.
    pub cache: Arc<UtteranceCache>,
    /// Current engine version string for cache partitioning.
    pub engine_version: EngineVersion,
    /// Optional preflight Rev.AI job ids keyed by original audio path.
    pub rev_job_ids: Arc<HashMap<PathBuf, RevAiJobId>>,
    /// Maximum number of file tasks to run concurrently for this job.
    pub num_workers: NumWorkers,
}

/// Shared per-file FA dependencies.
///
/// Grouping the job snapshot, services, and per-dispatch options here keeps
/// `process_one_fa_file` focused on the file lifecycle rather than repeating a
/// long parameter list for every call site.
struct FaFileContext<'a> {
    /// Immutable runner snapshot for the current job.
    job: &'a RunnerJobSnapshot,
    /// Read-only host/runtime context for media resolution and config access.
    host: DispatchHostContext,
    /// File/job lifecycle sink for runner-side status updates.
    sink: Arc<dyn RunnerEventSink>,
    /// Shared worker/cache services for FA and UTR.
    services: PipelineServices<'a>,
    /// Typed FA parameter bundle.
    fa_params: FaParams,
    /// Whether merge-abbrev should run before writing the result.
    should_merge_abbrev: bool,
    /// Optional before-file path for incremental align reruns.
    before_path: Option<&'a std::path::Path>,
    /// Optional UTR engine for the pre-pass and fallback paths.
    utr_engine: Option<&'a crate::options::UtrEngine>,
    /// Overlap strategy for `+<` utterances during UTR.
    utr_overlap_strategy: crate::options::UtrOverlapStrategy,
    /// Rev.AI preflight job ids keyed by original provider audio path.
    rev_job_ids: &'a HashMap<PathBuf, RevAiJobId>,
    /// Fallback language from job submission, only present when the user
    /// passed an explicit `--lang <iso3>`. The per-file language from
    /// `@Languages:` takes priority — this is consulted only when the
    /// file's header is absent. If `None` (the user passed `--lang auto`)
    /// AND the file has no `@Languages:`, the per-file resolver surfaces
    /// a typed error rather than silently inventing English.
    lang_fallback: Option<&'a LanguageCode3>,
    /// Debug artifact writer for offline replay.
    dumper: DebugDumper,
    /// Custom media directory from `--media-dir`.
    media_dir: Option<&'a str>,
}

/// Return the parent directory of a filename as a `Path`, or `None` if the
/// filename has no directory component (i.e. it is a bare name like `foo.cha`).
///
/// Used when constructing the `infer_client` path for `infer_media_mapping`:
/// joining this onto `source_dir` makes the repo key visible when `source_dir`
/// is a top-level data root and the filename carries a repo-key prefix.
fn filename_parent_dir(filename: &str) -> Option<&Path> {
    Path::new(filename)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
}

fn media_search_subdir(filename: &str, media_subdir: &str) -> String {
    let file_parent = Path::new(filename)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if file_parent.is_empty() {
        media_subdir.to_string()
    } else if media_subdir.is_empty() {
        file_parent
    } else {
        format!("{media_subdir}/{file_parent}")
    }
}

async fn find_media_in_root(root: &Path, subdir: &str, stem: &str) -> Option<PathBuf> {
    let search_dir = if subdir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(subdir)
    };
    for ext in crate::runner::util::KNOWN_MEDIA_EXTENSIONS {
        let candidate = search_dir.join(format!("{stem}.{ext}"));
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{find_media_in_root, media_search_subdir};

    #[test]
    fn media_search_subdir_preserves_mapping_subdir() {
        assert_eq!(
            media_search_subdir("d01oma12a.cha", "French/Newcastle/Discussion/12"),
            "French/Newcastle/Discussion/12"
        );
        assert_eq!(
            media_search_subdir("Discussion/12/d01oma12a.cha", "French/Newcastle"),
            "French/Newcastle/Discussion/12"
        );
    }

    #[tokio::test]
    async fn find_media_in_root_searches_nested_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("French/Newcastle/Discussion/12");
        std::fs::create_dir_all(&nested).unwrap();
        let target = nested.join("d01oma12a.mp3");
        std::fs::write(&target, b"mp3").unwrap();

        let found =
            find_media_in_root(dir.path(), "French/Newcastle/Discussion/12", "d01oma12a").await;

        assert_eq!(found.as_deref(), Some(target.as_path()));
    }
}

struct AlignAudioTask<'a> {
    host: DispatchHostContext,
    job_id: crate::api::JobId,
    file_index: usize,
    filename: String,
    services: PipelineServices<'a>,
    fa_params: FaParams,
    before_path: Option<PathBuf>,
    file_lang: LanguageCode3,
    audio_path: PathBuf,
    audio_identity: crate::chat_ops::fa::AudioIdentity,
    total_audio_ms: Option<u64>,
    chat_file: crate::chat_ops::ChatFile,
    parse_errors: Vec<crate::chat_ops::ParseError>,
    had_unrecovered_untimed: bool,
    utr_fallback_attempted: bool,
    utr_engine: Option<crate::options::UtrEngine>,
    utr_overlap_strategy: crate::options::UtrOverlapStrategy,
    rev_job_id: Option<String>,
    dumper: &'a DebugDumper,
    debug_traces: bool,
    provenance_lang: String,
    incremental_enabled: bool,
}

#[async_trait]
impl AudioChatTask for AlignAudioTask<'_> {
    type AttemptOutput = FaResult;

    async fn run_attempt(
        &mut self,
        progress_tx: crate::runner::util::ProgressSender,
    ) -> Result<Self::AttemptOutput, crate::error::ServerError> {
        let before_text = if let Some(bp) = self.before_path.as_deref() {
            tokio::fs::read_to_string(bp)
                .await
                .map_err(|e| tracing::warn!(path = %bp.display(), error = %e, "failed to read before-file for incremental FA"))
                .ok()
        } else {
            None
        };

        let audio = AudioContext {
            audio_path: &self.audio_path,
            audio_identity: &self.audio_identity,
            total_audio_ms: self.total_audio_ms.map(DurationMs),
        };

        if let Some(ref bt) = before_text {
            let current_text = talkbank_transform::serialize::to_chat_string(&self.chat_file);
            crate::fa::process_fa_incremental(
                bt,
                &current_text,
                &audio,
                &self.file_lang,
                self.services,
                &self.fa_params,
                Some(&progress_tx),
            )
            .await
        } else {
            crate::fa::run_fa_from_ast(
                self.chat_file.clone(),
                self.parse_errors.clone(),
                &audio,
                &self.file_lang,
                self.services,
                &self.fa_params,
                Some(&progress_tx),
            )
            .await
        }
    }

    async fn finalize_success(
        &mut self,
        fa_result: Self::AttemptOutput,
    ) -> Result<String, crate::error::ServerError> {
        let output_text = if self.debug_traces {
            let output_text = fa_result.chat_text.clone();
            let file_traces = crate::types::traces::FileTraces {
                filename: DisplayPath::from(self.filename.as_str()),
                dp_alignments: Vec::new(),
                asr_pipeline: None,
                fa_timeline: Some(fa_result.into_timeline_trace()),
                retokenizations: Vec::new(),
            };
            self.host
                .trace_store()
                .upsert_file(&self.job_id, self.file_index, file_traces)
                .await;
            output_text
        } else {
            fa_result.chat_text
        };

        self.dumper.dump_fa_output(&self.filename, &output_text);
        let provenance = crate::provenance::align_provenance(
            &self.provenance_lang,
            self.services.engine_version.as_ref(),
            None,
            false,
            self.incremental_enabled,
        );
        Ok(crate::provenance::inject_provenance_into_text(
            &output_text,
            &provenance,
        ))
    }

    async fn on_retryable_worker_failure(
        &mut self,
        lifecycle: &FileRunTracker<'_>,
        _error: &crate::error::ServerError,
    ) {
        if self.had_unrecovered_untimed
            && !self.utr_fallback_attempted
            && let Some(utr_engine) = self.utr_engine.as_ref()
        {
            self.utr_fallback_attempted = true;
            info!(
                filename = %self.filename,
                "FA failed with untimed utterances; attempting fallback UTR"
            );
            lifecycle.stage(FileStage::RecoveringTimingFallback).await;

            match run_utr_pass(
                &mut self.chat_file,
                UtrPassContext {
                    audio_path: self.audio_path.as_path(),
                    lang: &self.file_lang,
                    services: self.services,
                    audio_identity: &self.audio_identity,
                    cache_policy: self.fa_params.cache_policy,
                    total_audio_ms: self.total_audio_ms.map(DurationMs),
                    max_group_ms: Some(self.fa_params.max_group_ms),
                    filename: &self.filename,
                    engine: utr_engine,
                    overlap_strategy: self.utr_overlap_strategy,
                    rev_job_id: self.rev_job_id.as_deref(),
                    dumper: self.dumper,
                },
                None,
            )
            .await
            {
                Ok(utr_result) if utr_result.injected > 0 => {
                    self.had_unrecovered_untimed = false;
                    info!(
                        filename = %self.filename,
                        injected = utr_result.injected,
                        "Fallback UTR recovered timing"
                    );
                }
                Ok(utr_result) => {
                    warn!(
                        filename = %self.filename,
                        injected = utr_result.injected,
                        "Fallback UTR ran but injected no timing; proceeding to FA retry without additional anchors"
                    );
                }
                Err(error) => {
                    warn!(
                        filename = %self.filename,
                        error = %error,
                        "Fallback UTR failed with error; proceeding to FA retry without timing recovery"
                    );
                }
            }
        }
    }
}

/// Dispatch FA (forced alignment) via the server-side infer path.
///
/// Unlike morphosyntax/utseg/translate/coref, FA is per-file: each file has
/// its own audio, so there is no cross-file batching. Files are processed
/// concurrently up to `num_workers` at a time, bounded by a semaphore.
/// Within each file, utterances are grouped into time windows and batched
/// to the worker.
pub(crate) async fn dispatch_fa_infer(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    runtime: FaDispatchRuntime,
    plan: FaDispatchPlan,
) {
    let job_id = &job.identity.job_id;
    // The job-level lang field is NOT authoritative for align — each file
    // declares its own language via @Languages. The per-file language is
    // extracted inside process_one_fa_file after parsing. We propagate
    // the *resolved* job lang as a fallback for files that genuinely
    // lack a header; if the job's lang is Auto, the per-file resolver
    // surfaces a typed error rather than silently inventing English.
    let job_lang_fallback: Option<LanguageCode3> = job.dispatch.lang.as_resolved().cloned();
    let sink = host.sink().clone();
    let fa_params = plan.options.fa_params;
    let should_merge_abbrev = plan.options.merge_abbrev.should_merge();
    let utr_engine = plan.options.utr_engine;
    let utr_overlap_strategy = plan.options.utr_overlap_strategy;
    let file_parallelism = runtime
        .num_workers
        .0
        .max(1)
        .min(plan.kernel_plan.file_parallelism_hint.max(1));

    // Read before_paths once for incremental FA
    let before_paths = job.filesystem.before_paths.clone();

    // Process files concurrently, bounded by available workers.
    let file_sem = Arc::new(tokio::sync::Semaphore::new(file_parallelism));
    let mut tasks = Vec::new();

    for file in &job.pending_files {
        // Check cancellation before spawning
        if job.cancel_token.is_cancelled() {
            break;
        }

        let Ok(permit) = file_sem.clone().acquire_owned().await else {
            tracing::warn!("file semaphore closed during shutdown");
            break;
        };
        let host = host.clone();
        let sink = sink.clone();
        let pool = runtime.pool.clone();
        let cache = runtime.cache.clone();
        let job = job.clone();
        let engine_version = runtime.engine_version.clone();
        let file = file.clone();
        let file_index = file.file_index;
        let before_path = if !before_paths.is_empty() && file_index < before_paths.len() {
            Some(before_paths[file_index].assume_shared_filesystem())
        } else {
            None
        };
        let utr_engine = utr_engine.clone();
        let job_lang_fallback = job_lang_fallback.clone();
        let rev_job_ids = runtime.rev_job_ids.clone();
        let filename = file.filename.clone();

        tasks.push(spawn_supervised_file_task(
            filename,
            "align file task",
            async move {
                let _permit = permit;
                let services = PipelineServices::new(&pool, &cache, &engine_version);
                let dumper = DebugDumper::new(job.dispatch.options.common().debug_dir.as_deref());
                let media_dir_str;
                let media_dir_ref = if let CommandOptions::Align(ref opts) = job.dispatch.options {
                    media_dir_str = opts.media_dir.clone();
                    media_dir_str.as_deref()
                } else {
                    None
                };
                let context = FaFileContext {
                    job: &job,
                    host,
                    sink: sink.clone(),
                    services,
                    fa_params,
                    should_merge_abbrev,
                    before_path: before_path.as_ref().map(|p| p.as_path()),
                    utr_engine: utr_engine.as_ref(),
                    utr_overlap_strategy,
                    rev_job_ids: rev_job_ids.as_ref(),
                    lang_fallback: job_lang_fallback.as_ref(),
                    dumper,
                    media_dir: media_dir_ref,
                };
                process_one_fa_file(&file, context).await
            },
        ));
    }

    let abnormal_exits =
        drain_supervised_file_tasks(sink.as_ref(), job_id, &job.cancel_token, tasks).await;
    if abnormal_exits > 0 {
        warn!(
            job_id = %job_id,
            abnormal_exits,
            "Supervised align file tasks exited abnormally"
        );
    }
}

/// Process a single CHAT file through the server-side FA pipeline.
async fn process_one_fa_file(
    file: &crate::store::PendingJobFile,
    context: FaFileContext<'_>,
) -> FileTaskOutcome {
    let FaFileContext {
        job,
        host,
        sink,
        services,
        fa_params,
        should_merge_abbrev,
        before_path,
        utr_engine,
        utr_overlap_strategy: _,
        rev_job_ids,
        lang_fallback,
        ref dumper,
        media_dir,
    } = context;
    let job_id = &job.identity.job_id;
    let file_index = file.file_index;
    let filename = file.filename.as_ref();
    let lifecycle = FileRunTracker::new(sink.as_ref(), job_id, filename);
    let started_at = unix_now();

    lifecycle
        .begin_first_attempt(
            WorkUnitKind::FileForcedAlignment,
            started_at,
            FileStage::Reading,
        )
        .await;

    // Read the CHAT file
    let read_path: PathBuf =
        if job.filesystem.paths_mode && file_index < job.filesystem.source_paths.len() {
            job.filesystem.source_paths[file_index]
                .assume_shared_filesystem()
                .as_path()
                .to_owned()
        } else {
            job.filesystem
                .staging_dir
                .join("input")
                .join(filename)
                .as_path()
                .to_owned()
        };
    let media_mapping = job.filesystem.media_mapping.clone();
    let media_subdir = job.filesystem.media_subdir.clone();
    let source_dir = job.filesystem.source_dir.clone();

    let chat_text = match tokio::fs::read_to_string(&read_path).await {
        Ok(content) => content,
        Err(e) => {
            let err_msg = format!("Failed to read input: {e}");
            lifecycle
                .fail(&err_msg, FailureCategory::InputMissing, unix_now())
                .await;
            return FileTaskOutcome::TerminalStateRecorded;
        }
    };
    lifecycle.stage(FileStage::ResolvingAudio).await;

    // Resolve audio path.
    // Everything is local to the execution host now, but the corpus root and
    // media root can still differ on that host. Search order:
    //   1. explicit --media-dir root replacement using the known corpus subdir
    //   2. paths_mode adjacency (or content-mode source_dir when shared)
    //   3. local media_mappings root replacement on the execution host
    //   4. server media_roots fallback
    //   5. flat --media-dir / staged adjacency fallback
    let stem = Path::new(filename)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mapped_subdir = media_search_subdir(filename, media_subdir.as_str());
    let media_dir_path = media_dir.map(Path::new);

    let mut original_audio_path = None;

    if let Some(root) = media_dir_path
        && let Some(candidate) = find_media_in_root(root, &mapped_subdir, &stem).await
    {
        info!(
            filename,
            media_dir = %root.display(),
            mapped_subdir = %mapped_subdir,
            "Resolved audio via --media-dir root mapping"
        );
        original_audio_path = Some(candidate);
    }

    if original_audio_path.is_none() && job.filesystem.paths_mode {
        original_audio_path = resolve_audio_for_chat_with_media_dir(&read_path, None).await;
    }

    if original_audio_path.is_none() && !source_dir.is_empty() {
        // paths_mode is active here — convert to a ServerPath for I/O.
        let server_source_dir = source_dir.assume_shared_filesystem();
        let source_path =
            server_source_dir.join(Path::new(filename).file_name().unwrap_or_default());
        let source_audio =
            resolve_audio_for_chat_with_media_dir(source_path.as_path(), media_dir.map(Path::new))
                .await;
        if source_audio.is_some() {
            info!(
                filename,
                source_dir = %source_dir,
                "Resolved audio via client source directory"
            );
            original_audio_path = source_audio;
        }
    }

    if original_audio_path.is_none()
        && !media_mapping.is_empty()
        && let Some(root) = host.media_mapping_root(media_mapping.as_str())
        && let Some(candidate) = find_media_in_root(root.as_path(), &mapped_subdir, &stem).await
    {
        info!(
            filename,
            media_mapping = %media_mapping,
            mapped_subdir = %mapped_subdir,
            "Resolved audio via local media mapping"
        );
        original_audio_path = Some(candidate);
    }

    // Auto-detect media mapping from the source path when no explicit
    // mapping was provided.  If the source path contains a known repo
    // name (e.g. "slabank-data" in /Users/operator/chat-data/slabank-data/...),
    // use the corresponding media_mappings root.
    //
    // The subdir within the media volume is computed from the source
    // path: everything after the repo name component.  Example:
    //   source: /Users/operator/chat-data/slabank-data/French/Newcastle/Photos
    //   repo key: slabank-data
    //   subdir within volume: French/Newcastle/Photos
    //   media root: /Volumes/Other/slabank
    //   search: /Volumes/Other/slabank/French/Newcastle/Photos/13/p01ana13.mp3
    // Auto-detect media mapping from the client's source path using typed
    // provenance-tracking path newtypes. `infer_media_mapping()` is a pure
    // string operation on the ClientPath — it extracts the repo name component
    // and repo-relative subdir WITHOUT filesystem I/O. This works for both
    // local daemon (paths_mode) AND remote --server jobs where the client
    // path is NOT on the server's filesystem.
    if original_audio_path.is_none() && media_mapping.is_empty() {
        // Build an infer_client path that contains the repo key as a component
        // so infer_media_mapping can locate the correct media volume.
        //
        // When source_dir is a top-level data root (content-mode / --server,
        // e.g. "/Volumes/data-drive/talkbank-data"), filenames carry the
        // repo key as a prefix ("aphasia-data/English/.../file.cha").
        // Joining the filename's parent onto source_dir exposes the key.
        // When source_dir already embeds a subdir (paths_mode / an operator's case),
        // the join is a no-op for bare filenames and still correct for nested ones.
        //
        // Do NOT join repo_subdir with mapped_subdir afterwards — infer_client
        // already embeds the file parent, so repo_subdir is the complete
        // media-volume-relative path.  Joining again would double-count it.
        let infer_client: Option<batchalign_types::paths::ClientPath> = if !source_dir.is_empty() {
            Some(match filename_parent_dir(filename) {
                Some(parent) => source_dir.join(parent),
                None => source_dir.clone(),
            })
        } else if job.filesystem.paths_mode {
            // read_path is server-local; its parent contains the repo key
            // (e.g. ".../slabank-data/French/Newcastle/Photos").
            read_path
                .as_path()
                .parent()
                .map(|p| batchalign_types::paths::ClientPath::new(p.to_string_lossy()))
        } else {
            None
        };

        if let Some(client_path) = infer_client
            && let Some((_inferred_key, inferred_root, repo_subdir)) =
                batchalign_types::paths::infer_media_mapping(
                    &client_path,
                    &host.config().media_mappings,
                )
        {
            let search_dir = repo_subdir.resolve_on_server(&inferred_root);

            if let Some(candidate) = find_media_in_root(search_dir.as_path(), "", &stem).await {
                info!(
                    filename,
                    inferred_key = %_inferred_key,
                    repo_subdir = %repo_subdir,
                    "Resolved audio via auto-detected media mapping"
                );
                original_audio_path = Some(candidate);
            }
        }
    }

    if original_audio_path.is_none() && !host.media_roots().is_empty() {
        'roots: for root in host.media_roots() {
            if let Some(candidate) = find_media_in_root(root.as_path(), "", &stem).await {
                original_audio_path = Some(candidate);
                break 'roots;
            }
        }
    }

    if original_audio_path.is_none() {
        original_audio_path =
            resolve_audio_for_chat_with_media_dir(&read_path, media_dir.map(Path::new)).await;
    }

    let original_audio_path = match original_audio_path {
        Some(p) => p,
        None => {
            let search_hint = if !source_dir.is_empty() {
                format!(
                    "in shared source directory '{}' or via --media-dir",
                    source_dir
                )
            } else if !media_mapping.is_empty() {
                format!("via local media mapping '{media_mapping}' subdir '{mapped_subdir}'")
            } else if media_dir.is_some() {
                "via --media-dir or alongside the staged .cha file".to_string()
            } else {
                "on a shared filesystem alongside the .cha file (or pass --media-dir)".to_string()
            };
            let err_msg = format!(
                "Cannot find audio file for {filename}. \
                 Searched for media with known extensions (.wav, .mp3, .mp4, etc.) {}.",
                search_hint
            );
            lifecycle
                .fail(&err_msg, FailureCategory::Validation, unix_now())
                .await;
            return FileTaskOutcome::TerminalStateRecorded;
        }
    };

    let rev_job_id = rev_job_ids.get(&original_audio_path).map(|id| &**id);

    // Convert non-WAV media (e.g. mp4) to WAV via ffmpeg if needed.
    // soundfile (Python) cannot read container formats like mp4 directly.
    let audio_path = match crate::ensure_wav::ensure_wav(&original_audio_path, None).await {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Media conversion failed for {filename}: {e}");
            lifecycle
                .fail(&err_msg, FailureCategory::Validation, unix_now())
                .await;
            return FileTaskOutcome::TerminalStateRecorded;
        }
    };

    // Compute audio identity for cache keying: path|mtime|size
    let audio_path_str = audio_path.to_string_lossy();
    let audio_identity = compute_audio_identity(&audio_path_str)
        .await
        .unwrap_or_else(|| {
            // Fallback: use path with zeroed metadata
            crate::chat_ops::fa::AudioIdentity::from_metadata(&audio_path_str, 0, 0)
        });

    // Get total audio duration via ffprobe (optional -- for untimed utterance estimation)
    let total_audio_ms = get_audio_duration_ms(&audio_path_str).await;
    let utr_audio_path = if utr_engine.as_ref().is_some_and(|e| e.is_rust_owned()) {
        original_audio_path.as_path()
    } else {
        audio_path.as_path()
    };

    // Single parse: parse CHAT text into AST once. This ChatFile flows through
    // UTR (in-place mutation) and then directly to FA — no serialize/re-parse.
    let fa_parser = crate::chat_parser();
    let (mut chat_file, parse_errors) =
        talkbank_transform::parse::parse_lenient(&fa_parser, &chat_text);

    // Read the primary language from @Languages, falling back to the
    // job-level lang only if the file has no `@Languages:` header. If
    // the file's header is absent AND the job has no resolved lang
    // (`--lang auto`), surface a typed error rather than silently
    // tagging this file as English.
    let file_lang: LanguageCode3 = match chat_file.languages.0.first() {
        Some(lc) => match LanguageCode3::try_new(lc.0.as_ref()) {
            Ok(code) => code,
            Err(_) => match lang_fallback {
                Some(fallback) => fallback.clone(),
                None => {
                    let msg = format!(
                        "align: file '{}' declares `@Languages: {}` which is not a parseable \
                         ISO 639-3 code, and the job was submitted with `--lang auto` so \
                         there is no fallback. Fix the file's @Languages or pass \
                         `--lang <iso3>`.",
                        filename, lc.0
                    );
                    lifecycle
                        .fail(&msg, FailureCategory::Validation, unix_now())
                        .await;
                    return FileTaskOutcome::TerminalStateRecorded;
                }
            },
        },
        None => match lang_fallback {
            Some(fallback) => fallback.clone(),
            None => {
                let msg = format!(
                    "align: file '{}' has no `@Languages:` header and the job was \
                     submitted with `--lang auto`. Add the header or pass \
                     `--lang <iso3>` so we can stamp `@Languages:` honestly.",
                    filename
                );
                lifecycle
                    .fail(&msg, FailureCategory::Validation, unix_now())
                    .await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
        },
    };

    // UTR pre-pass: if untimed utterances exist and a UTR engine is configured,
    // run ASR to recover utterance-level timing before FA grouping.
    let had_unrecovered_untimed = {
        let (timed, untimed) = crate::chat_ops::fa::count_utterance_timing(&chat_file);

        match plan_align_utr_stage(untimed, &file_lang, utr_engine) {
            Ok(AlignUtrDecision::SkipAllTimed) => {
                info!(filename, timed, "All utterances timed, skipping UTR");
                false
            }
            Ok(AlignUtrDecision::Run(utr_engine)) => {
                lifecycle.stage(FileStage::RecoveringUtteranceTiming).await;

                let utr_progress =
                    spawn_progress_forwarder(sink.clone(), job_id.clone(), filename.to_string());

                match run_utr_pass(
                    &mut chat_file,
                    UtrPassContext {
                        audio_path: utr_audio_path,
                        lang: &file_lang,
                        services,
                        audio_identity: &audio_identity,
                        cache_policy: fa_params.cache_policy,
                        total_audio_ms: total_audio_ms.map(DurationMs),
                        max_group_ms: Some(fa_params.max_group_ms),
                        filename,
                        engine: utr_engine,
                        overlap_strategy: context.utr_overlap_strategy,
                        rev_job_id,
                        dumper,
                    },
                    Some(&utr_progress),
                )
                .await
                {
                    Ok(utr_result) => utr_result.unmatched > 0,
                    Err(_) => true,
                }
            }
            Ok(AlignUtrDecision::ProceedWithoutUtr) => {
                warn!(
                    filename,
                    untimed,
                    "Untimed utterances detected but no UTR engine configured, using interpolation"
                );
                true
            }
            Err(error) => {
                lifecycle
                    .fail(&error.to_string(), FailureCategory::Validation, unix_now())
                    .await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
        }
    };

    // Provenance comments embed the language for auditability. Use the
    // per-file language we just resolved above (`file_lang`), not the
    // job-level placeholder. This matches what gets stamped into
    // `@Languages:` and avoids the silent eng substitution that the
    // 2026-05-03 incident punished.
    let provenance_lang = file_lang.as_ref().to_string();
    let mut task = AlignAudioTask {
        host,
        job_id: job_id.clone(),
        file_index,
        filename: filename.to_string(),
        services,
        fa_params,
        before_path: before_path.map(Path::to_path_buf),
        file_lang,
        audio_path,
        audio_identity,
        total_audio_ms,
        chat_file,
        parse_errors,
        had_unrecovered_untimed,
        utr_fallback_attempted: false,
        utr_engine: utr_engine.cloned(),
        utr_overlap_strategy: context.utr_overlap_strategy,
        rev_job_id: rev_job_id.map(|id| id.to_string()),
        dumper,
        debug_traces: job.dispatch.debug_traces,
        provenance_lang,
        incremental_enabled: !job.filesystem.before_paths.is_empty(),
    };

    run_audio_chat_file_task(
        job,
        sink.clone(),
        file,
        &lifecycle,
        WorkUnitKind::FileForcedAlignment,
        FileStage::Aligning,
        "Alignment",
        should_merge_abbrev,
        &mut task,
    )
    .await
}

#[cfg(test)]
mod auto_detect_tests {
    use super::*;
    use batchalign_types::paths::{ClientPath, MediaMappingKey, ServerPath};
    use std::{collections::BTreeMap, path::Path};

    /// Regression test: when `--server` is used with a top-level data
    /// directory (e.g. `/Volumes/data-drive/talkbank-data`) the filenames
    /// carry a repo-key prefix (`aphasia-data/English/...`).  The old code
    /// passed only `source_dir` to `infer_media_mapping`, which had no repo
    /// key in its path and always returned `None` — every file failed with
    /// "Cannot find audio file".
    ///
    /// The fix: join `source_dir + parent(filename)` before inference so the
    /// repo key is visible, then use `repo_subdir` directly (no double-join).
    #[tokio::test]
    async fn auto_detect_media_mapping_content_mode_filename_has_repo_key_prefix() {
        let dir = tempfile::tempdir().unwrap();
        // Set up: /tmp/X/aphasia/English/Protocol/APROCSA/2256_T4.mp3
        let media_root = dir.path().join("aphasia");
        let nested = media_root.join("English/Protocol/APROCSA");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("2256_T4.mp3"), b"mp3").unwrap();

        let source_dir = ClientPath::new("/Volumes/data-drive/talkbank-data".to_string());
        let filename = "aphasia-data/English/Protocol/APROCSA/2256_T4.cha";
        let stem = "2256_T4";

        // Build mappings: aphasia-data → <tempdir>/aphasia
        let mut mappings: BTreeMap<MediaMappingKey, ServerPath> = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("aphasia-data"),
            ServerPath::new(media_root.to_str().unwrap().to_string()),
        );

        let infer_client = match filename_parent_dir(filename) {
            Some(parent) => source_dir.join(parent),
            None => source_dir.clone(),
        };

        let result = batchalign_types::paths::infer_media_mapping(&infer_client, &mappings);
        assert!(
            result.is_some(),
            "infer_media_mapping should find aphasia-data key in '{}'",
            infer_client.as_str()
        );
        let (_key, inferred_root, repo_subdir) = result.unwrap();

        // repo_subdir already contains the correct path within the volume;
        // do NOT also join mapped_subdir (that would double the path).
        let search_dir = repo_subdir.resolve_on_server(&inferred_root);
        let found = find_media_in_root(search_dir.as_path(), "", stem).await;
        assert!(
            found.is_some(),
            "Should find 2256_T4.mp3 under {}",
            search_dir.as_path().display()
        );
    }

    /// Simulate an operator's scenario: source_dir contains "slabank-data",
    /// media_mappings has slabank-data → /Volumes/Other/slabank.
    /// The auto-detect should compute the full subdir and find the audio.
    #[tokio::test]
    async fn auto_detect_media_mapping_from_source_path() {
        let dir = tempfile::tempdir().unwrap();
        let media_root = dir.path().join("slabank");
        let nested = media_root.join("French/Newcastle/Photos/13");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("p08aul13.mp3"), b"mp3").unwrap();

        // Simulate: source_dir = .../slabank-data/French/Newcastle/Photos
        // filename = 13/p08aul13.cha
        // media_mapping root = <tempdir>/slabank
        let source_dir =
            Path::new("/Users/operator/chat-data/slabank-data/French/Newcastle/Photos");
        let filename = "13/p08aul13.cha";
        let stem = "p08aul13";
        let mapped_subdir = media_search_subdir(filename, "");
        assert_eq!(mapped_subdir, "13");

        // Simulate infer_media_mapping_from_path
        let inferred_key = "slabank-data";
        let inferred_root = media_root.to_str().unwrap();

        // Compute repo-relative subdir
        let path_str = source_dir.to_string_lossy();
        let repo_suffix = path_str
            .split(&format!("/{inferred_key}/"))
            .nth(1)
            .unwrap_or("");
        assert_eq!(repo_suffix, "French/Newcastle/Photos");

        let full_subdir = if repo_suffix.is_empty() {
            mapped_subdir.clone()
        } else if mapped_subdir.is_empty() {
            repo_suffix.to_string()
        } else {
            format!("{repo_suffix}/{mapped_subdir}")
        };
        assert_eq!(full_subdir, "French/Newcastle/Photos/13");

        let found = find_media_in_root(Path::new(inferred_root), &full_subdir, stem).await;
        assert!(
            found.is_some(),
            "Should find p08aul13.mp3 at {}/{}",
            inferred_root,
            full_subdir
        );
    }
}

#[cfg(test)]
mod utr_stage_tests {
    use super::{AlignUtrDecision, plan_align_utr_stage};
    use crate::api::LanguageCode3;
    use crate::options::UtrEngine;

    #[test]
    fn fully_timed_align_skips_utr_even_if_selected_engine_would_not_support_language() {
        let decision =
            plan_align_utr_stage(0, &LanguageCode3::yue(), Some(&UtrEngine::RevAi)).unwrap();
        assert_eq!(decision, AlignUtrDecision::SkipAllTimed);
    }

    #[test]
    fn untimed_align_with_no_utr_engine_uses_interpolation_path() {
        let decision = plan_align_utr_stage(3, &LanguageCode3::eng(), None).unwrap();
        assert_eq!(decision, AlignUtrDecision::ProceedWithoutUtr);
    }

    #[test]
    fn untimed_align_with_supported_utr_runs_selected_backend() {
        let decision =
            plan_align_utr_stage(2, &LanguageCode3::yue(), Some(&UtrEngine::Whisper)).unwrap();
        assert_eq!(decision, AlignUtrDecision::Run(&UtrEngine::Whisper));
    }

    #[test]
    fn untimed_align_with_unsupported_rev_utr_fails_stage_validation() {
        let err =
            plan_align_utr_stage(4, &LanguageCode3::yue(), Some(&UtrEngine::RevAi)).unwrap_err();
        assert!(
            err.to_string()
                .contains("requires utterance timing recovery")
        );
    }
}
