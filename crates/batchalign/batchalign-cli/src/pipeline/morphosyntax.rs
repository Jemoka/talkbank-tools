//! Morphosyntax pipeline built on the internal stage runner.

use crate::chat_ops::morphosyntax_ops::{
    BatchItemWithPosition, MultilingualPolicy, MwtDict, TokenizationMode, clear_morphosyntax,
    collect_payloads, declared_languages, inject_results, l2,
    remove_empty_morphosyntax_placeholders, validate_mor_alignment,
};
use crate::chat_ops::nlp::UdResponse;
use crate::chat_ops::{ChatFile, LanguageCode};
use talkbank_transform::parse::{is_ca, is_no_align, parse_lenient};
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};
use tracing::warn;

use crate::api::LanguageCode3;
use crate::error::ServerError;
use crate::morphosyntax::infer_batch;
use crate::pipeline::PipelineServices;
use crate::pipeline::plan::{PipelinePlan, StageFuture, StageId, StageSpec, run_plan};

/// Per-file morphosyntax pipeline state.
pub(crate) struct MorphosyntaxPipelineContext<'a> {
    /// Shared services for the run.
    pub services: PipelineServices<'a>,
    /// Original chat text.
    pub chat_text: &'a str,
    /// Job-level language received from dispatch / `MorphosyntaxParams.lang`.
    ///
    /// **Not used for inference, payload collection, or provenance** in the
    /// morphotag pipeline — those use `resolved_lang`, populated per-file from
    /// the parsed `@Languages:` header. Retained on the context only because
    /// the shared `MorphosyntaxParams` struct (used by translate/coref/
    /// transcribe-embedded-morphosyntax) carries it. See the 2026-05-03
    /// regression in the file's module-doc for why this field is no longer
    /// authoritative for morphotag.
    #[allow(
        dead_code,
        reason = "retained for symmetry with shared MorphosyntaxParams struct; see field doc"
    )]
    pub lang: &'a LanguageCode3,
    /// Injection tokenization mode.
    pub tokenization_mode: TokenizationMode,
    /// Multilingual payload collection policy.
    pub multilingual_policy: MultilingualPolicy,
    /// MWT lexicon for retokenization overrides.
    pub mwt: &'a MwtDict,
    /// [Experimental] Route @s words to secondary language Stanza models.
    pub l2_morphotag: bool,
    /// Parsed chat file.
    pub chat_file: Option<ChatFile>,
    /// Structured parse errors from lenient parse; drives the L0 pre-validation gate.
    pub parse_errors: Vec<crate::chat_ops::ParseError>,
    /// Whether the file carries `@Options: CA` (Conversation Analysis
    /// transcript). `@Options: CA` literally means "morphotag is not
    /// to be run on this file." CA files are pass-through for
    /// morphosyntax — no clear, no infer, no inject, no provenance.
    pub is_ca: bool,
    /// Whether the file carries `@Options: NoAlign`. **Informational
    /// only** for morphotag — NoAlign literally means "the `align`
    /// command in batchalign3 is not to run on this file." It is
    /// scoped to the FA (forced-alignment) command, which uses audio
    /// bullets to attach word-level timing. Morphotag is a text-tier
    /// transform with no relationship to audio timing or to the
    /// `align` command, so it MUST run on NoAlign files. The
    /// pre-2026-05-07 behavior treated NoAlign as a global "don't do
    /// anything" flag (a copy-paste from the FA pipeline, where it's
    /// appropriate), silently leaving 297 corpus files with stale
    /// `%mor`/`%gra` from prior morphotag runs and no path to fix
    /// them via rerun. Kept as a field for diagnostics; NOT
    /// consulted by `should_skip_inference` or by any other
    /// morphotag stage.
    pub is_no_align: bool,
    /// Per-file primary language resolved from the parsed `@Languages:` header
    /// (or the BA2-parity `eng` fallback when the header is absent). Set by
    /// `stage_parse`; consumed by every downstream stage that previously read
    /// `ctx.lang`. `None` before `stage_parse` runs.
    pub resolved_lang: Option<LanguageCode3>,
    /// Collected worker payloads.
    pub batch_items: Vec<BatchItemWithPosition>,
    /// Inferred worker responses.
    pub ud_responses: Vec<UdResponse>,
    /// Final serialized output.
    pub final_chat_text: Option<String>,
}

impl<'a> MorphosyntaxPipelineContext<'a> {
    fn new(
        chat_text: &'a str,
        lang: &'a LanguageCode3,
        services: PipelineServices<'a>,
        tokenization_mode: TokenizationMode,
        multilingual_policy: MultilingualPolicy,
        mwt: &'a MwtDict,
        l2_morphotag: bool,
    ) -> Self {
        Self {
            services,
            chat_text,
            lang,
            tokenization_mode,
            multilingual_policy,
            mwt,
            l2_morphotag,
            chat_file: None,
            parse_errors: Vec::new(),
            is_ca: false,
            is_no_align: false,
            resolved_lang: None,
            batch_items: Vec::new(),
            ud_responses: Vec::new(),
            final_chat_text: None,
        }
    }

    /// Returns the per-file resolved language, or a `Validation` error if
    /// `stage_parse` has not yet populated it. All inference, payload
    /// collection, injection, and provenance code paths must read the lang
    /// through this accessor — never `ctx.lang` (see field doc).
    fn require_resolved_lang(&self) -> Result<&LanguageCode3, ServerError> {
        self.resolved_lang.as_ref().ok_or_else(|| {
            ServerError::Validation(
                "morphotag: per-file resolved_lang missing (stage_parse must run first)".into(),
            )
        })
    }

    /// True when the file should bypass all morphosyntax inference stages
    /// (parse + serialize round-trip only). Set by `stage_parse` based on
    /// `@Options: CA`. Unsupported-primary-language files no longer
    /// pass-through; `stage_parse` returns a typed `Validation` error
    /// for those so the operator sees a per-file failure with an
    /// actionable message (rather than a silent "completed in 8 ms with
    /// no work done" that the dashboard surfaces as success).
    fn should_skip_inference(&self) -> bool {
        // `is_no_align` is intentionally NOT consulted; see `is_no_align` field doc.
        self.is_ca
    }
}

/// Run the morphosyntax pipeline for a single CHAT file.
pub(crate) async fn run_morphosyntax_pipeline(
    chat_text: &str,
    lang: &LanguageCode3,
    services: PipelineServices<'_>,
    tokenization_mode: TokenizationMode,
    multilingual_policy: MultilingualPolicy,
    mwt: &MwtDict,
    l2_morphotag: bool,
) -> Result<String, ServerError> {
    let plan = morphosyntax_plan();
    let mut ctx = MorphosyntaxPipelineContext::new(
        chat_text,
        lang,
        services,
        tokenization_mode,
        multilingual_policy,
        mwt,
        l2_morphotag,
    );
    let _ = run_plan("morphotag", &plan, &mut ctx, None).await?;
    ctx.final_chat_text.ok_or_else(|| {
        ServerError::Validation("morphotag pipeline completed without output".to_string())
    })
}

fn morphosyntax_plan<'a>() -> PipelinePlan<MorphosyntaxPipelineContext<'a>> {
    PipelinePlan::new(vec![
        StageSpec::new(StageId::Parse, vec![], always_enabled, stage_parse),
        StageSpec::new(
            StageId::PreValidate,
            vec![StageId::Parse],
            always_enabled,
            stage_prevalidate,
        ),
        StageSpec::new(
            StageId::ClearExisting,
            vec![StageId::PreValidate],
            always_enabled,
            stage_clear_existing,
        ),
        StageSpec::new(
            StageId::CollectPayloads,
            vec![StageId::ClearExisting],
            always_enabled,
            stage_collect_payloads,
        ),
        StageSpec::new(
            StageId::Infer,
            vec![StageId::CollectPayloads],
            always_enabled,
            stage_infer,
        ),
        StageSpec::new(
            StageId::ApplyResults,
            vec![StageId::Infer],
            always_enabled,
            stage_apply_results,
        ),
        StageSpec::new(
            StageId::PostValidate,
            vec![StageId::ApplyResults],
            always_enabled,
            stage_postvalidate,
        ),
        StageSpec::new(
            StageId::Serialize,
            vec![StageId::PostValidate],
            always_enabled,
            stage_serialize,
        ),
    ])
}

fn always_enabled(_: &MorphosyntaxPipelineContext<'_>) -> bool {
    true
}

/// Resolve the per-file morphotag language from the parsed `@Languages:`
/// header.
///
/// Returns a typed error when the header is absent or the declared
/// language is not a parseable ISO 639-3 code. **No silent fallback to
/// English** — falling back would either tag a non-English file as English
/// (the 2026-05-03 incident) or stamp a falsified `@Languages:` value into
/// the output. The caller (`stage_parse`) records the error against the
/// file's job-status entry; the file is returned unchanged.
///
/// Earlier BA2 code defaulted missing headers to `["eng"]`. That parity
/// shortcut was a known correctness hazard — see the 2026-05-03 incident.
/// We deliberately diverge.
pub(crate) fn resolve_per_file_lang(chat_file: &ChatFile) -> Result<LanguageCode3, ServerError> {
    let raw = chat_file.languages.0.first().ok_or_else(|| {
        ServerError::Validation(
            "morphotag: file has no `@Languages:` header. Add the header (e.g. \
             `@Languages: eng`) and re-run; per-file language is required for \
             honest %mor/%gra provenance."
                .to_string(),
        )
    })?;
    LanguageCode3::try_from(raw.as_str()).map_err(|err| {
        ServerError::Validation(format!(
            "morphotag: file's `@Languages:` declares '{raw}', which is not a \
             parseable ISO 639-3 code: {err}. Fix the header and re-run."
        ))
    })
}

/// Returns a per-file error message when the primary `@Languages` code is
/// not in Stanza's supported set.
///
/// Pre-2026-05-10 this function's `Some(...)` return drove a silent
/// pass-through (the file was returned unchanged with no `%mor`/`%gra`
/// injected, and the job reported `completed`). That was dishonest UX:
/// operators submitting a file with a typo'd or unsupported language
/// got back their input unchanged, with no surface signal that nothing
/// happened. The dashboard's failure column never lit up.
///
/// Post-2026-05-10 the caller (`stage_parse`) converts a `Some(...)`
/// return into a `ServerError::Validation`, which propagates up as a
/// per-file failure with the message visible in the dashboard. The
/// operator can then fix the `@Languages` header and re-run.
///
/// `@Options: CA` files still pass-through (handled by `is_ca`, a
/// separate flag) — that is a legitimate "morphotag not applicable to
/// this transcript convention" case, not a typo to surface.
pub(crate) fn unsupported_primary_language_error(chat_file: &ChatFile) -> Option<String> {
    if let Some(primary) = chat_file.languages.0.first() {
        if !crate::chat_ops::morphosyntax_ops::is_stanza_supported(primary) {
            return Some(format!(
                "morphotag: primary @Languages '{}' is not supported by Stanza. \
                 Fix the @Languages header to use a supported ISO-639-3 code and re-run. \
                 Supported codes: {}.",
                primary,
                talkbank_transform::morphosyntax::supported_iso3_codes().join(", ")
            ));
        }
    }
    None
}

fn stage_parse<'a, 'ctx>(ctx: &'a mut MorphosyntaxPipelineContext<'ctx>) -> StageFuture<'a> {
    Box::pin(async move {
        let parser = crate::chat_parser();
        let (chat_file, parse_errors) = parse_lenient(&parser, ctx.chat_text);
        if !parse_errors.is_empty() {
            warn!(
                num_errors = parse_errors.len(),
                "Parse errors in morphosyntax input (continuing with recovery)"
            );
        }
        ctx.parse_errors = parse_errors;
        ctx.is_ca = is_ca(&chat_file);
        // Recorded for diagnostics; morphotag does not act on NoAlign.
        // See field doc.
        ctx.is_no_align = is_no_align(&chat_file);

        if !ctx.is_ca {
            if let Some(error_msg) = unsupported_primary_language_error(&chat_file) {
                warn!(reason = %error_msg, "Morphotag rejected unsupported primary language");
                return Err(ServerError::Validation(error_msg));
            }
        }

        // Resolve the per-file language from the parsed `@Languages:` header.
        // After this line, `ctx.lang` (the job-level dispatch lang) MUST NOT
        // be read by any morphotag stage — use `ctx.require_resolved_lang()`
        // instead. See `resolve_per_file_lang` for the 2026-05-03 incident.
        //
        // `resolve_per_file_lang` errors when the header is missing or
        // declares a non-parseable code. For files already flagged as
        // `is_ca` / `is_no_align` / unsupported-by-Stanza, downstream stages
        // bypass inference (`should_skip_inference()`); we tolerate a
        // missing header on those files because the pipeline is going to
        // pass them through unchanged anyway. Anything else is a real
        // missing-language error and must surface to the operator.
        if !ctx.should_skip_inference() {
            ctx.resolved_lang = Some(resolve_per_file_lang(&chat_file)?);
        }

        ctx.chat_file = Some(chat_file);
        Ok(())
    })
}

fn stage_prevalidate<'a, 'ctx>(ctx: &'a mut MorphosyntaxPipelineContext<'ctx>) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.should_skip_inference() {
            return Ok(());
        }
        let chat_file = ctx.chat_file.as_ref().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before morphotag pre-validation".into())
        })?;
        if let Err(errors) =
            validate_to_level(chat_file, &ctx.parse_errors, ValidityLevel::MainTierValid)
        {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(ServerError::Validation(format!(
                "morphotag pre-validation failed: {}",
                msgs.join("; ")
            )));
        }
        Ok(())
    })
}

fn stage_clear_existing<'a, 'ctx>(
    ctx: &'a mut MorphosyntaxPipelineContext<'ctx>,
) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.should_skip_inference() {
            return Ok(());
        }
        let chat_file = ctx.chat_file.as_mut().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before clearing morphosyntax".into())
        })?;
        clear_morphosyntax(chat_file);
        Ok(())
    })
}

fn stage_collect_payloads<'a, 'ctx>(
    ctx: &'a mut MorphosyntaxPipelineContext<'ctx>,
) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.should_skip_inference() {
            return Ok(());
        }
        let primary_lang = LanguageCode::new(ctx.require_resolved_lang()?.as_ref());
        let chat_file = ctx.chat_file.as_ref().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before payload collection".into())
        })?;
        let langs = declared_languages(chat_file, &primary_lang);
        let collected = collect_payloads(chat_file, &primary_lang, &langs, ctx.multilingual_policy);
        ctx.batch_items = collected.batch_items;
        // Wave 5: thread collected.not_applicable into per-file outcome reporting.
        Ok(())
    })
}

fn stage_infer<'a, 'ctx>(ctx: &'a mut MorphosyntaxPipelineContext<'ctx>) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.batch_items.is_empty() {
            return Ok(());
        }
        let lang_code = ctx.require_resolved_lang()?.clone();
        let retokenize = ctx.tokenization_mode == TokenizationMode::StanzaRetokenize;
        ctx.ud_responses = infer_batch(
            ctx.services.pool,
            &ctx.batch_items,
            &lang_code,
            ctx.mwt,
            retokenize,
            None,
        )
        .await?;
        Ok(())
    })
}

fn stage_apply_results<'a, 'ctx>(
    ctx: &'a mut MorphosyntaxPipelineContext<'ctx>,
) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.batch_items.is_empty() {
            return Ok(());
        }

        // Extract L2 deferred positions BEFORE inject_results takes
        // ownership of items/responses (same pattern as batch.rs).
        let l2_deferred = if ctx.l2_morphotag {
            l2::extract_l2_deferred_positions(&ctx.batch_items, &ctx.ud_responses)
        } else {
            Vec::new()
        };

        let resolved_lang = ctx.require_resolved_lang()?.clone();
        let chat_file = ctx.chat_file.as_mut().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before result injection".into())
        })?;
        let lang_code = LanguageCode::new(resolved_lang.as_ref());
        let parser = crate::chat_parser();
        let _injection_result = inject_results(
            &parser,
            chat_file,
            std::mem::take(&mut ctx.batch_items),
            std::mem::take(&mut ctx.ud_responses),
            &lang_code,
            ctx.tokenization_mode,
            ctx.mwt,
        )
        .map_err(|e| ServerError::Validation(format!("Result injection failed: {e}")))?;

        // Secondary L2 dispatch (experimental): route @s words to
        // secondary language workers and splice real morphology.
        if !l2_deferred.is_empty() {
            crate::morphosyntax::dispatch_secondary_l2(
                chat_file,
                &l2_deferred,
                ctx.services,
                "single-file",
            )
            .await;
        }

        let alignment_warnings = validate_mor_alignment(chat_file);
        for warning in &alignment_warnings {
            warn!(warning = %warning, "Morphosyntax alignment mismatch");
        }
        Ok(())
    })
}

fn stage_postvalidate<'a, 'ctx>(ctx: &'a mut MorphosyntaxPipelineContext<'ctx>) -> StageFuture<'a> {
    Box::pin(async move {
        if ctx.should_skip_inference() {
            return Ok(());
        }
        let chat_file = ctx.chat_file.as_ref().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before morphotag post-validation".into())
        })?;
        if let Err(errors) = validate_output(chat_file, "morphotag") {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            warn!(errors = ?msgs, "morphotag post-validation warnings (non-fatal)");
        }
        Ok(())
    })
}

fn stage_serialize<'a, 'ctx>(ctx: &'a mut MorphosyntaxPipelineContext<'ctx>) -> StageFuture<'a> {
    Box::pin(async move {
        // CA pass-through: serialize as-is, no provenance, no
        // placeholder sweep. `is_no_align` is intentionally NOT
        // consulted; see `is_no_align` field doc. Unsupported-primary-
        // language files no longer reach this stage — `stage_parse`
        // returns a typed `Validation` error for them, surfacing as a
        // per-file failure rather than a silent pass-through.
        if ctx.is_ca {
            let chat_file = ctx.chat_file.as_mut().ok_or_else(|| {
                ServerError::Validation("Parsed chat missing before morphotag serialize".into())
            })?;
            ctx.final_chat_text = Some(to_chat_string(chat_file));
            return Ok(());
        }

        // Pull immutable values from ctx BEFORE taking the mutable chat_file
        // borrow, so we don't fight Rust over overlapping borrows. The lang
        // here is the per-file resolved value — NOT the job-level dispatch
        // lang — so a Czech file gets `lang=ces`, not `lang=eng`. See
        // `resolve_per_file_lang` doc.
        let resolved_lang = ctx.require_resolved_lang()?.clone();
        let engine_version = ctx.services.engine_version.clone();
        let retokenize = ctx.tokenization_mode
            == crate::chat_ops::morphosyntax_ops::TokenizationMode::StanzaRetokenize;

        let chat_file = ctx.chat_file.as_mut().ok_or_else(|| {
            ServerError::Validation("Parsed chat missing before morphotag serialize".into())
        })?;

        let provenance = crate::provenance::morphotag_provenance(
            resolved_lang.as_ref(),
            engine_version.as_ref(),
            retokenize,
            false, // incremental is handled separately
        );
        crate::provenance::inject_provenance(chat_file, &provenance);

        // Sweep any unfilled %mor/%gra placeholders left by clear_morphosyntax
        // for utterances whose response produced no UD sentence. Empty
        // placeholders preserve tier order during injection; this cleanup
        // prevents them from leaking into the serialized output.
        remove_empty_morphosyntax_placeholders(chat_file);

        ctx.final_chat_text = Some(to_chat_string(chat_file));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    //! Tests for per-file morphotag pass-through decisions. The full pipeline
    //! is exercised by integration tests against a worker pool; these unit
    //! tests cover the local predicate and pass-through serialization logic.
    use super::{run_morphosyntax_pipeline, unsupported_primary_language_error};
    use crate::api::EngineVersion;
    use crate::cache::UtteranceCache;
    use crate::chat_ops::morphosyntax_ops::{MultilingualPolicy, MwtDict, TokenizationMode};
    use crate::pipeline::PipelineServices;
    use crate::worker::pool::{PoolConfig, WorkerPool};
    use talkbank_transform::parse::parse_lenient;
    use talkbank_transform::serialize::to_chat_string;

    fn parse(text: &str) -> talkbank_model::model::ChatFile {
        let parser = crate::chat_parser();
        let (chat_file, _) = parse_lenient(&parser, text);
        chat_file
    }

    #[test]
    fn unsupported_primary_language_returns_actionable_error_message() {
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\tsrp\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\tsrp|test|CHI||female|||Target_Child|||\n\
                    *CHI:\tnešto .\n\
                    @End\n";
        let chat_file = parse(chat);
        let msg = unsupported_primary_language_error(&chat_file)
            .expect("unsupported primary language must produce an error message");
        assert!(
            msg.contains("srp"),
            "error must name the unsupported lang: {msg}"
        );
        assert!(
            msg.contains("not supported by Stanza"),
            "error must explicitly call out unsupported-by-Stanza so the \
             operator sees the cause in the dashboard: {msg}"
        );
        assert!(
            msg.contains("Fix the @Languages header"),
            "error must be actionable — tell the operator what to do: {msg}"
        );
    }

    #[test]
    fn supported_primary_language_passes() {
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI||female|||Target_Child|||\n\
                    *CHI:\thello .\n\
                    @End\n";
        let chat_file = parse(chat);
        assert!(
            unsupported_primary_language_error(&chat_file).is_none(),
            "eng must pass the Stanza-supported gate",
        );
    }

    #[test]
    fn empty_languages_header_passes_for_ba2_compat() {
        // BA2 defaulted to ["eng"] when no @Languages was present and proceeded.
        // The gate intentionally allows this — files lacking @Languages are not
        // hard-errored; they fall through to the dispatch's default-lang path.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI||female|||Target_Child|||\n\
                    *CHI:\thello .\n\
                    @End\n";
        let chat_file = parse(chat);
        assert!(
            unsupported_primary_language_error(&chat_file).is_none(),
            "missing @Languages must NOT hard-error (BA2 parity: defaults to eng)",
        );
    }

    #[test]
    fn resolve_per_file_lang_uses_primary_languages_header() {
        // Regression test for the 2026-05-03 morning incident: the morphotag
        // pipeline took its lang from the job-level CommandProfile sentinel
        // ("eng") instead of the file's @Languages header, so every Czech /
        // Spanish / Polish / etc. file got tagged with English Stanza and a
        // falsified `lang=eng` provenance comment.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\tces\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\tces|test|CHI||female|||Target_Child|||\n\
                    *CHI:\tahoj .\n\
                    @End\n";
        let chat_file = parse(chat);
        let resolved =
            super::resolve_per_file_lang(&chat_file).expect("Czech header must resolve cleanly");
        assert_eq!(
            resolved.as_ref(),
            "ces",
            "Czech file must resolve to ces, not the job-level sentinel",
        );
    }

    #[test]
    fn resolve_per_file_lang_errors_when_languages_absent() {
        // No silent eng fallback — a CHAT file with no `@Languages:` header
        // is a real provenance failure. Surface a typed error so the
        // operator fixes the header and re-runs.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI||female|||Target_Child|||\n\
                    *CHI:\thello .\n\
                    @End\n";
        let chat_file = parse(chat);
        let err = super::resolve_per_file_lang(&chat_file)
            .expect_err("missing @Languages must error, not silently default to eng");
        assert!(
            err.to_string().contains("`@Languages:`"),
            "error must point at the missing header: {err}"
        );
    }

    #[test]
    fn resolve_per_file_lang_uses_primary_only_when_bilingual() {
        // Bilingual file: primary lang wins. Secondary is consumed by the
        // multilingual policy / per-utterance routing, not by the pipeline's
        // top-level lang choice for inference + provenance.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\tspa, eng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\tspa|test|CHI||female|||Target_Child|||\n\
                    *CHI:\thola .\n\
                    @End\n";
        let chat_file = parse(chat);
        let resolved = super::resolve_per_file_lang(&chat_file)
            .expect("primary lang must resolve cleanly for bilingual file");
        assert_eq!(
            resolved.as_ref(),
            "spa",
            "primary lang wins; secondary is for per-utterance routing only",
        );
    }

    #[test]
    fn supported_language_with_unsupported_secondary_passes() {
        // A bilingual file where primary is supported (eng) and secondary is
        // not Stanza-supported (e.g., a non-Stanza tongue). The gate looks at
        // primary only; multilingual policy handles per-utterance routing.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\teng, srp\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI||female|||Target_Child|||\n\
                    *CHI:\thello .\n\
                    @End\n";
        let chat_file = parse(chat);
        assert!(
            unsupported_primary_language_error(&chat_file).is_none(),
            "primary=eng with secondary=srp must pass (gate is on primary only)",
        );
    }

    #[tokio::test]
    async fn unsupported_primary_language_returns_typed_validation_error() {
        // 2026-05-10 inversion: unsupported primary language is no longer
        // a silent pass-through. The pipeline returns a typed
        // `ServerError::Validation` so the per-file dispatch surfaces the
        // failure to the operator via the dashboard. The OLD behavior
        // (round-trip unchanged with no provenance) was dishonest UX —
        // operators got their input back with no signal that nothing
        // happened. See `unsupported_primary_language_error` doc comment
        // for the full rationale.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\tsrp\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\tsrp|test|CHI||female|||Target_Child|||\n\
                    *CHI:\tnešto .\n\
                    @End\n";
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cache = UtteranceCache::sqlite(Some(tempdir.path().join("cache")))
            .await
            .expect("cache");
        let pool = WorkerPool::new(PoolConfig::default());
        let engine_version = EngineVersion::from("test-morphotag");
        let services = PipelineServices::new(&pool, &cache, &engine_version);

        let err = run_morphosyntax_pipeline(
            chat,
            &crate::api::LanguageCode3::eng(),
            services,
            TokenizationMode::StanzaRetokenize,
            MultilingualPolicy::from_skip_flag(false),
            &MwtDict::default(),
            true,
        )
        .await
        .expect_err("unsupported primary language must surface as Err");

        let msg = err.to_string();
        assert!(
            msg.contains("srp"),
            "error must name the unsupported lang: {msg}"
        );
        assert!(
            msg.contains("not supported by Stanza"),
            "error must call out unsupported-by-Stanza: {msg}"
        );
    }

    #[tokio::test]
    async fn noalign_files_get_morphotagged_with_provenance() {
        // Pin the post-2026-05-07 inversion: NoAlign no longer skips
        // morphotag. Background in `is_no_align` field doc and the
        // postmortem at `docs/postmortems/2026-05-07-noalign-morphotag-skip.md`.
        let chat = "@UTF8\n\
                    @PID:\t11312/c-test\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI||female|||Target_Child|||\n\
                    @Options:\tNoAlign\n\
                    *CHI:\thello .\n\
                    @End\n";
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cache = UtteranceCache::sqlite(Some(tempdir.path().join("cache")))
            .await
            .expect("cache");
        let pool = WorkerPool::new(PoolConfig::default());
        let engine_version = EngineVersion::from("test-morphotag");
        let services = PipelineServices::new(&pool, &cache, &engine_version);

        let output = run_morphosyntax_pipeline(
            chat,
            &crate::api::LanguageCode3::eng(),
            services,
            TokenizationMode::StanzaRetokenize,
            MultilingualPolicy::from_skip_flag(false),
            &MwtDict::default(),
            true,
        )
        .await
        .expect("NoAlign file should be processed (no longer skipped)");

        assert!(
            output.contains("[ba3 morphotag |"),
            "NoAlign file must receive morphotag provenance — the \
             pipeline is no longer pass-through for NoAlign. Output: {output}"
        );
        // The @Options: NoAlign line itself is preserved (we don't
        // strip it; the directive remains for the `align` command
        // which is what it was always for).
        assert!(
            output.contains("@Options:\tNoAlign"),
            "NoAlign directive must be preserved verbatim",
        );
    }
}
