//! REST API request models — `POST /jobs` submission types.
//!
//! These are re-exported from [`super::api`] for backward compatibility.

use serde::{Deserialize, Serialize};

use crate::options::{AsrEngineName, CommandOptions, UtrEngine};
use crate::revai::{revai_known_broken, try_revai_language_hint};

use super::domain::{DisplayPath, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand};

// ---------------------------------------------------------------------------
// Request models
// ---------------------------------------------------------------------------

/// A single CHAT file submitted by the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct FilePayload {
    /// Original filename (e.g. "01DM_18.cha").
    pub filename: DisplayPath,
    /// Full CHAT file text.
    pub content: String,
}

/// `POST /jobs` request body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct JobSubmission {
    /// Batchalign command (align, morphotag, etc.).
    pub command: ReleasedCommand,
    /// Language specification: a 3-letter ISO code or `"auto"` for
    /// ASR-driven detection.
    #[serde(default = "default_lang")]
    pub lang: LanguageSpec,
    /// Number of speakers.
    #[serde(default = "default_num_speakers")]
    pub num_speakers: NumSpeakers,
    /// CHAT files to process.
    #[serde(default)]
    pub files: Vec<FilePayload>,
    /// Media filenames for the server to resolve from media_roots (transcribe only).
    #[serde(default)]
    pub media_files: Vec<String>,
    /// Key into server's media_mappings config (e.g. "childes-data").
    #[serde(default)]
    pub media_mapping: batchalign_types::paths::MediaMappingKey,
    /// Subdirectory under the mapped root (e.g. "Eng-NA/MacWhinney/0young-ASR").
    #[serde(default)]
    pub media_subdir: batchalign_types::paths::RepoRelativePath,
    /// Client's input directory path (for dashboard display).
    #[serde(default)]
    pub source_dir: batchalign_types::paths::ClientPath,
    /// Typed command options (engine selections, processing flags, etc.).
    #[cfg_attr(feature = "server", schema(value_type = serde_json::Value))]
    pub options: CommandOptions,

    // Paths mode — local daemon sends filesystem paths instead of content.
    /// When true, server reads/writes files directly via source_paths/output_paths.
    #[serde(default)]
    pub paths_mode: bool,
    /// Absolute paths to read input files from (paths_mode only).
    #[serde(default)]
    pub source_paths: Vec<batchalign_types::paths::ClientPath>,
    /// Absolute paths to write output files to (paths_mode only).
    #[serde(default)]
    pub output_paths: Vec<batchalign_types::paths::ClientPath>,
    /// Human-readable filenames for display (paths_mode only, optional).
    #[serde(default)]
    pub display_names: Vec<String>,

    /// When true, the server collects detailed algorithm traces for
    /// visualization (DP alignment matrices, ASR pipeline stages, FA
    /// timelines, retokenization mappings). Defaults to false — zero
    /// overhead when off.
    #[serde(default)]
    pub debug_traces: bool,

    /// Absolute paths to "before" files for incremental processing
    /// (paths_mode only). When non-empty, the diff engine compares each
    /// before file against its corresponding source_path and only
    /// reprocesses changed utterances.
    ///
    /// Must be the same length as `source_paths` when non-empty.
    #[serde(default)]
    pub before_paths: Vec<batchalign_types::paths::ClientPath>,
}

pub(crate) fn default_lang() -> LanguageSpec {
    LanguageSpec::Resolved(LanguageCode3::eng())
}

pub(crate) fn default_num_speakers() -> NumSpeakers {
    NumSpeakers(1)
}

impl JobSubmission {
    /// Validate submission constraints (paths_mode, command consistency).
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Validate options command tag matches the command field.
        if self.command != self.options.command_name() {
            return Err(ValidationError(format!(
                "options command tag '{}' does not match submission command '{}'",
                self.options.command_name(),
                self.command
            )));
        }

        // Validate the (command, lang) pairing is a legal one. Morphotag,
        // translate, and coref MUST submit `LanguageSpec::PerFile`; every
        // other processing command MUST NOT. This boundary check keeps
        // pipeline code from ever observing an invalid combination, and
        // makes the dashboard / job record show honest values.
        self.validate_lang_command_pairing()?;

        // Validate language support for engines the command will use.
        self.validate_language_support()?;

        if self.paths_mode {
            if self.source_paths.is_empty() || self.output_paths.is_empty() {
                return Err(ValidationError(
                    "paths_mode requires non-empty source_paths and output_paths".into(),
                ));
            }
            if self.source_paths.len() != self.output_paths.len() {
                return Err(ValidationError(
                    "source_paths and output_paths must have equal length".into(),
                ));
            }
            if !self.before_paths.is_empty() && self.before_paths.len() != self.source_paths.len() {
                return Err(ValidationError(
                    "before_paths must have the same length as source_paths when non-empty".into(),
                ));
            }
            if !self.files.is_empty() || !self.media_files.is_empty() {
                return Err(ValidationError(
                    "paths_mode is mutually exclusive with files/media_files".into(),
                ));
            }
        }
        Ok(())
    }

    /// Check that the job's language is supported by all engines the command
    /// will use.
    ///
    /// Called at job submission time to fail fast with a clear diagnostic
    /// rather than letting errors surface deep in the pipeline (Rev.AI HTTP
    /// 400, Whisper wrong-language transcription, Stanza model-not-found).
    /// Reject (command, lang) combinations that would be lies.
    ///
    /// `LanguageSpec::PerFile` is the unique correct shape for morphotag,
    /// translate, and coref — they have no `--lang` CLI flag and read
    /// language per-file from each CHAT file's `@Languages:` header. Any
    /// other shape on those commands means a placeholder is sneaking
    /// through (the 2026-05-03 morphotag incident).
    ///
    /// Conversely, every other processing command (transcribe,
    /// transcribe_s, benchmark, align, compare, utseg, opensmile, avqi)
    /// takes a concrete `--lang` (or `--lang auto` for ASR-detect). Those
    /// must never carry `PerFile`; if they do, something downstream of
    /// the CLI built a malformed `JobSubmission`.
    fn validate_lang_command_pairing(&self) -> Result<(), ValidationError> {
        use crate::api::ReleasedCommand;
        use crate::types::domain::LanguageSpec;

        let is_per_file_command = matches!(
            self.command,
            ReleasedCommand::Morphotag | ReleasedCommand::Translate | ReleasedCommand::Coref,
        );

        match (&self.lang, is_per_file_command) {
            (LanguageSpec::PerFile, true) => Ok(()),
            (LanguageSpec::PerFile, false) => Err(ValidationError(format!(
                "command '{}' does not accept LanguageSpec::PerFile; pass --lang or --lang auto",
                self.command
            ))),
            (LanguageSpec::Auto | LanguageSpec::Resolved(_), true) => {
                Err(ValidationError(format!(
                    "command '{}' has no --lang; submission must use LanguageSpec::PerFile (per-file \
                 resolution from @Languages: header). Job-level lang sentinels are banned for \
                 this command — see the 2026-05-03 morphotag incident.",
                    self.command
                )))
            }
            (LanguageSpec::Auto | LanguageSpec::Resolved(_), false) => Ok(()),
        }
    }

    fn validate_language_support(&self) -> Result<(), ValidationError> {
        // Auto-detect and per-file resolution both defer language to a later
        // stage, so submission-time engine-support checks can't run here. For
        // PerFile commands (morphotag/translate/coref) the per-file
        // `@Languages:` header is the authority; they don't use Rev.AI or
        // engine-tied processing this validator covers.
        let lang = match &self.lang {
            LanguageSpec::Auto | LanguageSpec::PerFile => return Ok(()),
            LanguageSpec::Resolved(code) => code,
        };

        // Commands that use eager request-level ASR validation: transcribe,
        // transcribe_s, benchmark.
        //
        // Align is intentionally excluded here. Whether align needs a UTR/ASR
        // stage depends on the parsed file's timing state, so align-specific
        // backend validation is deferred to the runtime after parsing.
        let asr_engine = match &self.options {
            CommandOptions::Transcribe(opts) | CommandOptions::TranscribeS(opts) => {
                Some(opts.effective_asr_engine())
            }
            CommandOptions::Benchmark(opts) => Some(opts.effective_asr_engine()),
            _ => None,
        };

        // Check Rev.AI language support
        if let Some(AsrEngineName::RevAi) = &asr_engine
            && try_revai_language_hint(lang).is_none()
        {
            return Err(ValidationError(format!(
                "Language '{}' is not supported by Rev.AI ASR. Alternatives:\n\
                 - Use --asr-engine whisper for local Whisper ASR (supports most languages)\n\
                 - Use --asr-engine-custom tencent for Chinese/Hakka via Tencent\n\
                 - Check supported languages: book/src/reference/language-code-resolution.md",
                lang
            )));
        }

        // Rev.AI known-broken (engine, language) deny-list.
        //
        // Rev.AI advertises support for a language but has been observed to
        // return output unusable for CHAT construction — see
        // `REVAI_KNOWN_BROKEN` in `revai/preflight.rs` for current entries
        // with dated provenance. Rejecting at preflight turns a late-stage
        // per-token validation failure into a clear up-front message that
        // names a working alternative.
        //
        // Rationale and escalation path (when the deny-list stops being
        // enough): book/src/reference/revai-language-quality-strategy.md.
        if let Some(AsrEngineName::RevAi) = &asr_engine
            && let Some(entry) = revai_known_broken(lang)
        {
            return Err(ValidationError(format!(
                "Language '{}' is known to produce unusable quality on Rev.AI ASR: {}. \
                 Use --asr-engine {} instead. \
                 See book/src/reference/revai-language-quality-strategy.md \
                 for the rationale and the list of known-broken pairs.",
                lang, entry.reason, entry.recommended_engine
            )));
        }

        // Commands that use Stanza: morphotag, utseg, coref, compare
        let uses_stanza = matches!(
            &self.options,
            CommandOptions::Morphotag(_)
                | CommandOptions::Utseg(_)
                | CommandOptions::Coref(_)
                | CommandOptions::Compare(_)
        );
        if uses_stanza && !is_stanza_supported_language(lang) {
            return Err(ValidationError(format!(
                "Language '{}' is not supported by Stanza. Supported languages:\n\
                 {}",
                lang,
                stanza_supported_languages_help()
            )));
        }

        // Check HK ASR engine language constraints
        if let Some(engine) = &asr_engine {
            let chinese_codes = ["zho", "yue", "wuu", "nan", "hak", "cmn"];
            match engine {
                AsrEngineName::HkTencent if !chinese_codes.contains(&lang.as_ref()) => {
                    return Err(ValidationError(format!(
                        "Language '{}' is not supported by Tencent ASR (Chinese variants only: {}). \
                         Use --asr-engine whisper or --asr-engine rev instead.",
                        lang,
                        chinese_codes.join(", ")
                    )));
                }
                AsrEngineName::HkAliyun if lang.as_ref() != "yue" => {
                    return Err(ValidationError(format!(
                        "Language '{}' is not supported by Aliyun ASR (Cantonese 'yue' only). \
                         Use --asr-engine whisper or --asr-engine rev instead.",
                        lang
                    )));
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Validate that one selected UTR backend can support a resolved language.
///
/// This is intended for stage-aware runtime checks inside `align`, after the
/// file has been parsed and the runtime knows whether UTR is actually needed.
pub(crate) fn validate_utr_language_support(
    lang: &LanguageCode3,
    engine: &UtrEngine,
) -> Result<(), ValidationError> {
    match engine {
        UtrEngine::RevAi => {
            if try_revai_language_hint(lang).is_none() {
                return Err(ValidationError(format!(
                    "This file requires utterance timing recovery, but the selected UTR backend 'rev' does not support language '{}'. \
                     Use --utr-engine whisper for local Whisper UTR or --utr-engine-custom tencent_utr for Cantonese/Hakka UTR.",
                    lang
                )));
            }
        }
        UtrEngine::HkTencent => {
            let chinese_codes = ["zho", "yue", "wuu", "nan", "hak", "cmn"];
            if !chinese_codes.contains(&lang.as_ref()) {
                return Err(ValidationError(format!(
                    "This file requires utterance timing recovery, but the selected UTR backend 'tencent_utr' only supports Chinese variants ({}).",
                    chinese_codes.join(", ")
                )));
            }
        }
        UtrEngine::Whisper => {}
    }

    Ok(())
}

/// Validation error for request models.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct ValidationError(pub String);

// ---------------------------------------------------------------------------
// Stanza language support — hardcoded fallback table
// ---------------------------------------------------------------------------

/// Hardcoded fallback table of ISO 639-3 codes supported by Stanza.
///
/// **DEPRECATED as the primary check.** The authoritative source is now
/// the `StanzaRegistry` built from Stanza's `resources.json` at worker
/// startup. This table is ONLY used as a pre-validation safety net when
/// the registry hasn't been populated yet (before first worker spawn).
///
/// Check whether an ISO 639-3 language code is supported by Stanza.
///
/// Single Rust source of truth: delegates to
/// `crate::chat_ops::morphosyntax_ops::is_stanza_supported`.
/// A previous hardcoded `STANZA_SUPPORTED_ISO3` list duplicated that data
/// and silently drifted from it (see the 2026-04-24 Malayalam crash
/// audit at `stanza_languages.rs` module docs). The authoritative
/// truth ultimately lives in the Python capability table built from
/// Stanza's installed `resources.json`; this Rust function is a fast
/// preflight that uses the most-recently-audited approximation.
fn is_stanza_supported_language(lang: &LanguageCode3) -> bool {
    let code = crate::chat_ops::LanguageCode::new(lang.as_ref());
    crate::chat_ops::morphosyntax_ops::is_stanza_supported(&code)
}

/// Format a help string listing supported Stanza languages for error messages.
fn stanza_supported_languages_help() -> String {
    crate::chat_ops::morphosyntax_ops::supported_iso3_codes()
        .chunks(10)
        .map(|chunk| chunk.join(", "))
        .collect::<Vec<_>>()
        .join(",\n  ")
}

/// Validate a job's language support using the runtime Stanza registry.
///
/// This is the **authoritative** language validation, called from
/// `materialize_submission_job()` where the registry is available.
/// It supersedes the hardcoded `is_stanza_supported_language()` check
/// in `validate_language_support()`, which acts as a conservative
/// pre-filter only.
///
/// Returns `Ok(())` when:
/// - The command doesn't use Stanza
/// - The language is auto-detect
/// - The registry confirms the language has required processors
/// - The registry is not populated (fallback to hardcoded table)
pub fn validate_language_with_registry(
    submission: &JobSubmission,
    registry: Option<&crate::stanza_registry::StanzaRegistry>,
) -> Result<(), ValidationError> {
    // Auto: can't validate until ASR resolves the language.
    // PerFile: morphotag/translate/coref resolve per-file from @Languages:;
    // the registry validation happens per-file in stage_parse.
    let lang = match &submission.lang {
        LanguageSpec::Auto | LanguageSpec::PerFile => return Ok(()),
        LanguageSpec::Resolved(code) => code,
    };

    let uses_stanza = matches!(
        &submission.options,
        CommandOptions::Morphotag(_)
            | CommandOptions::Utseg(_)
            | CommandOptions::Coref(_)
            | CommandOptions::Compare(_)
    );

    if !uses_stanza {
        return Ok(());
    }

    let Some(reg) = registry else {
        // Registry not populated — the hardcoded table in validate() already
        // caught obviously unsupported languages.
        return Ok(());
    };

    if !reg.supports_morphosyntax(lang.as_ref()) {
        let supported = reg.supported_languages().join(", ");
        return Err(ValidationError(format!(
            "Language '{}' is not supported by Stanza on this server. \
             Supported languages: {}",
            lang, supported
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{
        AlignOptions, CommonOptions, MorphotagOptions, TranscribeOptions, UtsegOptions,
    };

    /// Build a minimal morphotag `JobSubmission` for testing validation.
    ///
    /// Morphotag has no `--lang` flag — every legal submission carries
    /// `LanguageSpec::PerFile`. The `lang_spec` parameter exists only so
    /// regression tests can construct *invalid* submissions (e.g. with
    /// `Resolved(eng)`) and assert that `validate()` rejects them.
    fn morphotag_submission_with_lang(lang_spec: LanguageSpec) -> JobSubmission {
        JobSubmission {
            command: ReleasedCommand::Morphotag,
            lang: lang_spec,
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            paths_mode: false,
            source_paths: vec![],
            output_paths: vec![],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        }
    }

    /// Convenience for the common case: a legal morphotag submission.
    fn morphotag_submission() -> JobSubmission {
        morphotag_submission_with_lang(LanguageSpec::PerFile)
    }

    fn utseg_submission(lang: &str) -> JobSubmission {
        JobSubmission {
            command: ReleasedCommand::Utseg,
            lang: LanguageSpec::Resolved(LanguageCode3::try_new(lang).expect("test lang")),
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: CommandOptions::Utseg(UtsegOptions {
                common: CommonOptions::default(),
                merge_abbrev: Default::default(),
                utseg_fallback: Default::default(),
            }),
            paths_mode: false,
            source_paths: vec![],
            output_paths: vec![],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        }
    }

    fn align_submission(lang: &str, utr_engine: Option<UtrEngine>) -> JobSubmission {
        JobSubmission {
            command: ReleasedCommand::Align,
            lang: LanguageSpec::Resolved(LanguageCode3::try_new(lang).expect("test lang")),
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: CommandOptions::Align(AlignOptions {
                common: CommonOptions::default(),
                utr_engine,
                ..AlignOptions::default()
            }),
            paths_mode: true,
            source_paths: vec!["/tmp/test.cha".into()],
            output_paths: vec!["/tmp/out.cha".into()],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        }
    }

    /// Build a minimal transcribe submission parameterized by language and
    /// ASR engine. Used by the deny-list tests below to exercise the
    /// validation path without spinning up a real server.
    fn transcribe_submission(lang: &str, asr_engine: AsrEngineName) -> JobSubmission {
        JobSubmission {
            command: ReleasedCommand::Transcribe,
            lang: LanguageSpec::Resolved(LanguageCode3::try_new(lang).expect("test lang")),
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: CommandOptions::Transcribe(TranscribeOptions {
                common: CommonOptions::default(),
                asr_engine,
                diarize: false,
                wor: false.into(),
                merge_abbrev: false.into(),
                utseg_fallback: false.into(),
                batch_size: 8,
            }),
            paths_mode: true,
            source_paths: vec!["/tmp/test.mp3".into()],
            output_paths: vec!["/tmp/out.cha".into()],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        }
    }

    // --- RED: known-broken (engine, language) pair deny-list -----------------
    //
    // As of 2026-04-22, Rev.AI's Malayalam (lang=ml / iso3=mal) ASR is
    // unusable in practice. A ~1-minute Malayalam sample re-submitted
    // directly to Rev.AI with language=ml returned 55 text elements
    // comprising Korean Hangul mixed with Malayalam vowel signs
    // ('모두െ'), stray Latin words ('occurrence', 'Moo', 'Take', 'Me',
    // 'ganhar', 'segueiasm'), bare Gurmukhi/Punjabi tokens in the final
    // third of the transcript, U+FFFD replacement characters inside
    // tokens (');�', 'philan�ുടഖ഻ിറ്'), and semicolon+paren punctuation
    // as "words" (');�'). Evidence is kept in an operational workspace
    // outside this repo; see the strategy doc for the procedure.
    //
    // `try_revai_language_hint("mal")` maps to "ml" which Rev.AI accepts —
    // but the result is cross-script garbage that no CHAT validator can
    // accept. Propagating that output produces confusing late-stage E220 /
    // E330 validation errors on arbitrary tokens; users have no way to tell
    // it was the ASR that failed, not their transcript.
    //
    // The fix is to extend `validate_language_support()` with a known-broken
    // deny-list that rejects unusable (engine, language) pairs at submission
    // time and points the user at a working alternative.
    #[test]
    fn transcribe_rev_on_malayalam_is_rejected_as_known_broken() {
        let submission = transcribe_submission("mal", AsrEngineName::RevAi);
        let err = submission
            .validate()
            .expect_err("Rev.AI + Malayalam must be rejected at preflight");
        let msg = err.to_string();
        assert!(
            msg.contains("mal") || msg.contains("Malayalam"),
            "error must name the offending language; got: {msg}"
        );
        assert!(
            msg.contains("whisper_hub"),
            "error must recommend --asr-engine whisper_hub specifically \
             (stock whisper is also empirically broken for mal; see the \
             strategy doc); got: {msg}"
        );
        // The message must explain *why* (quality / known-broken), so the
        // user understands this isn't their file.
        let low = msg.to_lowercase();
        assert!(
            low.contains("known") || low.contains("quality") || low.contains("unusable"),
            "error must explain quality / known-broken reason; got: {msg}"
        );
    }

    /// Guard rail: the deny-list must not over-reject. `eng` + Rev.AI is the
    /// default path used by every English-language job on the fleet and
    /// must keep passing validation.
    #[test]
    fn transcribe_rev_on_english_remains_valid() {
        let submission = transcribe_submission("eng", AsrEngineName::RevAi);
        submission
            .validate()
            .expect("eng + Rev.AI must remain valid; deny-list must not over-reject");
    }

    /// Whisper is a fallback alternative for languages where the stock
    /// model still produces usable output. It must itself pass validation
    /// for Malayalam submissions so any downstream escalation from Rev.AI
    /// (or any other engine) doesn't hit a second validation failure.
    ///
    /// Caveat: empirical evaluation (2026-04-22) showed stock
    /// Whisper's Malayalam output is *also* broken — it collapses into
    /// Khmer/Gurmukhi loops and hallucinates "Thank you for watching."
    /// That is why the Rev.AI deny-list for ``mal`` now recommends
    /// ``whisper_hub``, not ``whisper``. This test remains as a *validation*
    /// guard rail (API-supported), not a quality claim.
    #[test]
    fn transcribe_whisper_on_malayalam_passes_validation() {
        let submission = transcribe_submission("mal", AsrEngineName::Whisper);
        submission
            .validate()
            .expect("whisper + mal must pass validation; quality caveats live in the book");
    }

    /// ``whisper_hub`` is the recommended alternative in the updated
    /// Rev.AI deny-list error message (see
    /// ``book/src/reference/revai-language-quality-strategy.md``). For
    /// the recommendation to be viable, ``whisper_hub`` + ``mal`` must
    /// pass validation itself.
    #[test]
    fn transcribe_whisper_hub_on_malayalam_passes_validation() {
        let submission = transcribe_submission("mal", AsrEngineName::WhisperHub);
        submission.validate().expect(
            "whisper_hub + mal must pass validation so the deny-list \
             recommendation is viable",
        );
    }

    #[test]
    fn morphotag_per_file_passes_validation() {
        // The only legal morphotag submission shape: PerFile lang.
        let submission = morphotag_submission();
        submission
            .validate()
            .expect("morphotag with LanguageSpec::PerFile must pass validation");
    }

    /// 2026-05-03 incident regression test. A morphotag job-level
    /// `Resolved(eng)` was the historical placeholder; submission-time
    /// validation must reject it so it never appears in job records or
    /// leaks into worker pre-warming.
    #[test]
    fn morphotag_resolved_lang_is_rejected() {
        let submission = morphotag_submission_with_lang(LanguageSpec::Resolved(
            LanguageCode3::try_new("eng").unwrap(),
        ));
        let err = submission
            .validate()
            .expect_err("morphotag with Resolved(eng) must be rejected");
        assert!(
            err.to_string().contains("LanguageSpec::PerFile"),
            "rejection message must point at PerFile remedy: {err}"
        );
    }

    #[test]
    fn morphotag_auto_lang_is_rejected() {
        let submission = morphotag_submission_with_lang(LanguageSpec::Auto);
        let err = submission
            .validate()
            .expect_err("morphotag with Auto must be rejected (Auto is an ASR-engine signal)");
        assert!(err.to_string().contains("LanguageSpec::PerFile"));
    }

    /// Mirror coverage: utseg DOES take an explicit `--lang`, so it must
    /// continue to reject `PerFile` (the per-file variant is reserved for
    /// morphotag, translate, coref).
    #[test]
    fn utseg_per_file_lang_is_rejected() {
        let mut submission = utseg_submission("eng");
        submission.lang = LanguageSpec::PerFile;
        let err = submission
            .validate()
            .expect_err("utseg with PerFile must be rejected");
        assert!(
            err.to_string()
                .contains("does not accept LanguageSpec::PerFile")
        );
    }

    #[test]
    fn utseg_with_unsupported_language_fails() {
        let submission = utseg_submission("xyz");
        let err = submission.validate().unwrap_err();
        assert!(
            err.to_string().contains("not supported by Stanza"),
            "expected Stanza error, got: {err}"
        );
    }

    #[test]
    fn align_with_unsupported_rev_language_does_not_fail_request_validation() {
        let submission = align_submission("yue", Some(UtrEngine::RevAi));
        assert!(
            submission.validate().is_ok(),
            "align should defer UTR language checks until file timing state is known"
        );
    }

    #[test]
    fn utr_runtime_validation_rejects_rev_for_unsupported_language() {
        let err =
            validate_utr_language_support(&LanguageCode3::yue(), &UtrEngine::RevAi).unwrap_err();
        assert!(
            err.to_string()
                .contains("requires utterance timing recovery")
                && err.to_string().contains("Use --utr-engine whisper"),
            "expected stage-aware UTR error, got: {err}"
        );
    }

    #[test]
    fn utr_runtime_validation_allows_whisper_for_yue() {
        assert!(validate_utr_language_support(&LanguageCode3::yue(), &UtrEngine::Whisper).is_ok());
    }
}
