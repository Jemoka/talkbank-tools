use crate::planning;
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage};
use crate::scheduling::WorkUnitKind;
use crate::store::{RunnerJobSnapshot, unix_now};
use crate::text_batch::TextBatchFileResults;

use super::text_io::{load_text_inputs, write_text_results};
use super::worker_gateway::WorkerGateway;

/// Dispatch a translate job: per-file source-language routing.
///
/// **BA2 parity (2026-05-03 fix).** BA2's translate reads each file's
/// `doc.langs[0]` as the source language for inference
/// (`~/batchalign2-master/batchalign/pipelines/translate/seamless.py:40`).
/// Earlier BA3 used `dispatch_simple_batched_text_job` which pulled one
/// job-level lang and pooled all files into a single inference call —
/// the same shape that caused the 2026-05-03 morphotag incident
/// (English Stanza silently applied to non-English files).
///
/// This dispatch parses each file's `@Languages:` header and resolves the
/// per-file source language via `resolve_per_file_lang`. Files whose
/// header is missing or malformed are recorded as failures and skipped —
/// no silent English fallback. Cross-file pooling is intentionally given
/// up in exchange for per-file lang correctness, per-file failure
/// isolation, and per-file durability — the same trade-off
/// `execution::utseg::dispatch_utseg_job` made (see its module doc for
/// the rationale).
pub(crate) async fn dispatch_translate_job(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    gateway: &dyn WorkerGateway,
    should_merge_abbrev: bool,
) -> Result<(), crate::error::ServerError> {
    let plan = planning::build_job_plan(job).map_err(|error| {
        crate::error::ServerError::Validation(format!("Translate planning failed: {error}"))
    })?;
    let sink = host.sink().clone();
    let started_at = unix_now();

    for file in &job.pending_files {
        FileRunTracker::new(sink.as_ref(), &job.identity.job_id, file.filename.as_ref())
            .begin_first_attempt(WorkUnitKind::BatchInfer, started_at, FileStage::Translating)
            .await;
    }

    let inputs = load_text_inputs(job, host, false).await;
    if inputs.file_texts.is_empty() {
        return Ok(());
    }

    let mut all_results: TextBatchFileResults = Vec::with_capacity(inputs.file_texts.len());
    let parser = crate::chat_parser();
    for file_input in inputs.file_texts {
        if job.cancel_token.is_cancelled() {
            break;
        }

        // Resolve source language from the file's own @Languages header.
        // No silent English fallback — a file with no parseable language
        // becomes a typed Err in this file's batch results, surfaced to
        // the operator via the job's file_statuses.
        let (chat_file, _parse_errors) =
            talkbank_transform::parse::parse_lenient(&parser, file_input.chat_text.as_ref());
        match crate::pipeline::morphosyntax::resolve_per_file_lang(&chat_file) {
            Ok(src_lang) => {
                // Single-file batch at the gateway boundary. This loses
                // cross-file pooling on purpose: per-file lang correctness
                // > batching speedup.
                let mut results = gateway
                    .translate_batch(std::slice::from_ref(&file_input), &src_lang)
                    .await;
                all_results.append(&mut results);
            }
            Err(err) => {
                tracing::warn!(
                    file = %file_input.filename,
                    error = %err,
                    "Translate skipping file: per-file language resolution failed",
                );
                all_results.push(crate::text_batch::TextBatchFileResult::err(
                    file_input.filename.clone(),
                    err.to_string(),
                ));
            }
        }
    }

    write_text_results(
        job,
        host,
        &plan,
        all_results,
        should_merge_abbrev,
        "Translate",
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use crate::chat_ops::morphosyntax_ops::MwtDict;
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{
        CorrelationId, DisplayPath, JobId, LanguageCode3, LanguageSpec, NumSpeakers,
        ReleasedCommand, WorkerLanguage,
    };
    use crate::capability::WorkerCapabilitySnapshot;
    use crate::execution::worker_gateway::MorphotagRuntimeOptions;
    use crate::options::{CommandOptions, CommonOptions, TranslateOptions};
    use crate::store::PendingJobFile;
    use crate::text_batch::{TextBatchFileInput, TextBatchFileResult, TextBatchFileResults};

    #[derive(Default)]
    struct FakeTranslateGateway {
        state: Mutex<FakeTranslateState>,
    }

    #[derive(Default)]
    struct FakeTranslateState {
        batch_calls: usize,
        batch_sizes: Vec<usize>,
    }

    #[async_trait]
    impl WorkerGateway for FakeTranslateGateway {
        async fn ensure_command_capabilities(
            &self,
            _command: ReleasedCommand,
            _lang: WorkerLanguage,
            _engine_overrides: &str,
        ) -> Result<WorkerCapabilitySnapshot, String> {
            unreachable!()
        }

        async fn morphotag_for_compare(
            &self,
            _chat_text: &str,
            _lang: &LanguageCode3,
            _mwt: &MwtDict,
        ) -> Result<String, crate::error::ServerError> {
            unreachable!()
        }

        async fn morphotag_single(
            &self,
            _chat_text: &str,
            _before_text: Option<&str>,
            _lang: &LanguageCode3,
            _options: MorphotagRuntimeOptions,
        ) -> Result<String, crate::error::ServerError> {
            unreachable!()
        }

        async fn utseg_batch(
            &self,
            _files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
            _allow_stanza_fallback: bool,
        ) -> TextBatchFileResults {
            unreachable!()
        }

        async fn translate_batch(
            &self,
            files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
        ) -> TextBatchFileResults {
            let mut state = self.state.lock().unwrap();
            state.batch_calls += 1;
            state.batch_sizes.push(files.len());
            files
                .iter()
                .map(|file| {
                    let translated = file.chat_text.replace("@End", "%xtra:\ttranslated\n@End");
                    TextBatchFileResult::ok(file.filename.clone(), translated)
                })
                .collect()
        }

        async fn coref_batch(
            &self,
            _files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
        ) -> TextBatchFileResults {
            unreachable!()
        }
    }

    fn translate_snapshot(staging_dir: &std::path::Path) -> RunnerJobSnapshot {
        // Translate dispatch resolves the source language *per file* from
        // each file's `@Languages:` header (BA2 parity, see module doc).
        // A missing header is a typed failure, not a silent fallback —
        // so the test fixture must declare a language explicitly.
        // CHAT requires `@Languages` and `@Participants` to appear after
        // `@Begin` for the parser to extract them into the typed model.
        let text = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Subject\n@ID:\teng|test|PAR|||||Subject|||\n*PAR:\thello world .\n@End\n";
        let input_dir = staging_dir.join("input");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::write(input_dir.join("a.cha"), text).unwrap();
        std::fs::write(input_dir.join("b.cha"), text).unwrap();
        RunnerJobSnapshot {
            identity: crate::store::RunnerJobIdentity {
                job_id: JobId::from("job-translate"),
                correlation_id: CorrelationId::from("corr-translate"),
            },
            dispatch: crate::store::RunnerDispatchConfig {
                command: ReleasedCommand::Translate,
                // Translate is a per-file-language command (BA2 parity, see
                // module doc) — submission validation rejects any other
                // `LanguageSpec` for this command.
                lang: LanguageSpec::PerFile,
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Translate(TranslateOptions {
                    common: CommonOptions::default(),
                    merge_abbrev: false.into(),
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            filesystem: crate::store::RunnerFilesystemConfig {
                paths_mode: false,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                before_paths: Vec::new(),
                staging_dir: batchalign_types::paths::ServerPath::new(
                    staging_dir.display().to_string(),
                ),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: batchalign_types::paths::ClientPath::new(
                    staging_dir.display().to_string(),
                ),
            },
            cancel_token: CancellationToken::new(),
            pending_files: vec![
                PendingJobFile {
                    file_index: 0,
                    filename: DisplayPath::from("a.cha"),
                    has_chat: true,
                },
                PendingJobFile {
                    file_index: 1,
                    filename: DisplayPath::from("b.cha"),
                    has_chat: true,
                },
            ],
        }
    }

    fn host() -> DispatchHostContext {
        let (tx, _rx) = tokio::sync::broadcast::channel(crate::ws::BROADCAST_CAPACITY);
        DispatchHostContext::from_store(Arc::new(crate::store::JobStore::new(
            crate::config::ServerConfig::default(),
            None,
            tx,
        )))
    }

    #[tokio::test]
    async fn translate_dispatches_one_gateway_call_per_file() {
        // BA2 parity (2026-05-03): translate dispatches one gateway call per
        // input file, each tagged with that file's per-file source language
        // resolved from the file's `@Languages:` header. Cross-file pooling
        // was removed because it forced a single job-level lang across files
        // — the same shape that caused the morphotag incident. See
        // `dispatch_translate_job` doc for the full rationale.
        let temp = tempfile::tempdir().unwrap();
        let host = host();
        let gateway = FakeTranslateGateway::default();
        let job = translate_snapshot(temp.path());

        dispatch_translate_job(&job, &host, &gateway, false)
            .await
            .expect("translate dispatch");

        let state = gateway.state.lock().unwrap();
        assert_eq!(
            state.batch_calls, 2,
            "one gateway call per file (BA2 parity); pooled batching is gone"
        );
        assert_eq!(
            state.batch_sizes,
            vec![1, 1],
            "each per-file call carries exactly one file"
        );
    }

    #[tokio::test]
    async fn translate_write_path_persists_xtra_output() {
        let temp = tempfile::tempdir().unwrap();
        let host = host();
        let gateway = FakeTranslateGateway::default();
        let job = translate_snapshot(temp.path());

        dispatch_translate_job(&job, &host, &gateway, false)
            .await
            .expect("translate dispatch");

        let output = std::fs::read_to_string(temp.path().join("output").join("a.cha")).unwrap();
        assert!(output.contains("%xtra:\ttranslated"));
    }
}
