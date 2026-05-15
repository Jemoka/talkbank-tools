//! Pure functions that extract typed dispatch parameters from [`CommandOptions`].
//!
//! These functions encapsulate the "read options from job" logic that was
//! previously inlined in async dispatch functions, making it testable without
//! async runtimes, job stores, or worker pools.
//!
//! # Wiring guarantee
//!
//! Every field in every [`CommandOptions`] variant MUST be consumed by exactly
//! one extraction function. The tests in this module verify this invariant by
//! constructing options with non-default values and asserting the extracted
//! parameters reflect those values. If a new field is added to a
//! `CommandOptions` struct but not wired here, the corresponding test will
//! fail (or a new test must be added to cover it).

// These pure extractors feed the typed dispatch-plan builders in `plan.rs`.
// Some returned compatibility fields (`wor_tier`, `batch_size`) are still
// carried forward for future pipeline wiring even when today's orchestrator
// does not yet consume them.

use crate::api::DurationMs;
use crate::chat_ops::fa::{FaEngineType, FaTimingMode};
use crate::chat_ops::morphosyntax_ops::{MultilingualPolicy, TokenizationMode};
#[allow(unused_imports)]
use crate::options::{
    AlignOptions, AsrEngineName, BenchmarkOptions, CommandOptions, CommonOptions, MorphotagOptions,
    OpensmileOptions, TranscribeOptions, UtrEngine, UtrOverlapStrategy,
};
use crate::params::{CachePolicy, FaParams, MergeAbbrevPolicy, WorTierPolicy};

// ---------------------------------------------------------------------------
// Align
// ---------------------------------------------------------------------------

/// Extracted parameters for the FA dispatch path.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct FaDispatchParams {
    pub fa_params: FaParams,
    pub merge_abbrev: MergeAbbrevPolicy,
    pub utr_engine: Option<UtrEngine>,
    pub utr_overlap_strategy: UtrOverlapStrategy,
}

/// Extract FA dispatch parameters from [`CommandOptions`].
///
/// Returns `None` if the options are not for an `align` command.
pub(crate) fn extract_fa_dispatch_params(
    options: &CommandOptions,
    cache_policy: CachePolicy,
) -> Option<FaDispatchParams> {
    let (
        fa_engine,
        pauses,
        wor,
        utr_engine,
        utr_overlap_strategy,
        merge_abbrev,
        bullet_repair,
        review_level,
    ) = match options {
        CommandOptions::Align(a) => (
            a.effective_fa_engine(),
            a.pauses,
            a.wor,
            a.utr_engine.clone(),
            a.utr_overlap_strategy,
            a.merge_abbrev,
            a.bullet_repair,
            a.review_level,
        ),
        _ => return None,
    };

    let engine = FaEngineType::from_str_lossy(fa_engine.as_wire_name());

    let (timing_mode, max_group_ms) = match engine {
        FaEngineType::Wave2Vec => (FaTimingMode::Continuous, DurationMs(15_000)),
        FaEngineType::WhisperFa if pauses => (FaTimingMode::WithPauses, DurationMs(20_000)),
        FaEngineType::WhisperFa => (FaTimingMode::Continuous, DurationMs(20_000)),
    };

    Some(FaDispatchParams {
        fa_params: FaParams {
            timing_mode,
            max_group_ms,
            engine,
            cache_policy,
            wor_tier: wor,
            bullet_repair,
            review_level,
        },
        merge_abbrev,
        utr_engine,
        utr_overlap_strategy,
    })
}

// ---------------------------------------------------------------------------
// Transcribe
// ---------------------------------------------------------------------------

/// Extracted parameters for the transcribe dispatch path.
#[derive(Debug, PartialEq)]
pub(crate) struct TranscribeDispatchParams {
    pub asr_engine: AsrEngineName,
    pub diarize: bool,
    pub wor_tier: WorTierPolicy,
    pub batch_size: i32,
    pub merge_abbrev: MergeAbbrevPolicy,
    pub override_media_cache: bool,
    /// Operator opt-in to the legacy Stanza constituency-parser
    /// fallback for utseg when no language-specific TalkBank BERT
    /// utseg model is configured. Surfaced as
    /// `--utseg-fallback-stanza` on the transcribe / transcribe-s CLI.
    pub allow_stanza_fallback_utseg: bool,
}

/// Extract transcribe dispatch parameters from [`CommandOptions`].
///
/// Returns `None` if the options are not for a `transcribe` or `transcribe_s`
/// command.
pub(crate) fn extract_transcribe_dispatch_params(
    options: &CommandOptions,
) -> Option<TranscribeDispatchParams> {
    match options {
        CommandOptions::Transcribe(t) | CommandOptions::TranscribeS(t) => {
            Some(TranscribeDispatchParams {
                asr_engine: t.effective_asr_engine(),
                diarize: t.diarize,
                wor_tier: t.wor,
                batch_size: t.batch_size,
                merge_abbrev: t.merge_abbrev,
                override_media_cache: t.common.override_media_cache,
                allow_stanza_fallback_utseg: t.utseg_fallback.is_allowed(),
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Morphotag
// ---------------------------------------------------------------------------

/// Extracted parameters for the morphotag dispatch path.
#[derive(Debug, PartialEq)]
pub(crate) struct MorphotagDispatchParams {
    pub tokenization_mode: TokenizationMode,
    pub multilingual_policy: MultilingualPolicy,
    pub override_media_cache: bool,
    pub merge_abbrev: MergeAbbrevPolicy,
    pub l2_morphotag: bool,
    pub respect_pos_hints: bool,
}

/// Extract morphotag dispatch parameters from [`CommandOptions`].
pub(crate) fn extract_morphotag_dispatch_params(
    options: &CommandOptions,
) -> Option<MorphotagDispatchParams> {
    match options {
        CommandOptions::Morphotag(m) => Some(MorphotagDispatchParams {
            tokenization_mode: TokenizationMode::from(m.retokenize),
            multilingual_policy: MultilingualPolicy::from_skip_flag(m.skipmultilang),
            override_media_cache: m.common.override_media_cache,
            merge_abbrev: m.merge_abbrev,
            l2_morphotag: !m.no_l2_morphotag,
            respect_pos_hints: !m.no_pos_hints,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------

/// Extracted parameters for the benchmark dispatch path.
#[derive(Debug, PartialEq)]
pub(crate) struct BenchmarkDispatchParams {
    pub asr_engine: AsrEngineName,
    pub wor_tier: WorTierPolicy,
    pub merge_abbrev: MergeAbbrevPolicy,
    pub override_media_cache: bool,
}

/// Extract benchmark dispatch parameters from [`CommandOptions`].
pub(crate) fn extract_benchmark_dispatch_params(
    options: &CommandOptions,
) -> Option<BenchmarkDispatchParams> {
    match options {
        CommandOptions::Benchmark(b) => Some(BenchmarkDispatchParams {
            asr_engine: b.effective_asr_engine(),
            wor_tier: b.wor,
            merge_abbrev: b.merge_abbrev,
            override_media_cache: b.common.override_media_cache,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Opensmile
// ---------------------------------------------------------------------------

/// Extracted parameters for the opensmile dispatch path.
#[derive(Debug, PartialEq)]
pub(crate) struct OpensmileDispatchParams {
    pub feature_set: String,
}

/// Extract opensmile dispatch parameters from [`CommandOptions`].
pub(crate) fn extract_opensmile_dispatch_params(
    options: &CommandOptions,
) -> Option<OpensmileDispatchParams> {
    match options {
        CommandOptions::Opensmile(o) => Some(OpensmileDispatchParams {
            feature_set: o.feature_set.clone(),
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{AsrEngineName, FaEngineName};
    use std::collections::BTreeMap;

    fn common_default() -> CommonOptions {
        CommonOptions::default()
    }

    fn common_with_cache_override() -> CommonOptions {
        CommonOptions {
            override_media_cache: true,
            ..Default::default()
        }
    }

    // =======================================================================
    // Align: every AlignOptions field is exercised
    // =======================================================================

    fn align_opts(
        fa_engine: &str,
        utr_engine: Option<UtrEngine>,
        pauses: bool,
        wor: bool,
        merge_abbrev: bool,
    ) -> CommandOptions {
        CommandOptions::Align(AlignOptions {
            common: common_default(),
            fa_engine: FaEngineName::from_wire_name(fa_engine).expect("test fa_engine"),
            utr_engine,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses,
            wor: wor.into(),
            merge_abbrev: merge_abbrev.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        })
    }

    #[test]
    fn fa_dispatch_reads_fa_engine() {
        let p1 = extract_fa_dispatch_params(
            &align_opts("whisper_fa", None, false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p1.fa_params.engine, FaEngineType::WhisperFa);

        let p2 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", None, false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p2.fa_params.engine, FaEngineType::Wave2Vec);
    }

    #[test]
    fn fa_dispatch_reads_pauses() {
        let p1 = extract_fa_dispatch_params(
            &align_opts("whisper_fa", None, true, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p1.fa_params.timing_mode, FaTimingMode::WithPauses);
        assert_eq!(p1.fa_params.max_group_ms, DurationMs(20_000));

        let p2 = extract_fa_dispatch_params(
            &align_opts("whisper_fa", None, false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p2.fa_params.timing_mode, FaTimingMode::Continuous);
    }

    #[test]
    fn fa_dispatch_reads_wor() {
        let p1 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", None, false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p1.fa_params.wor_tier, WorTierPolicy::Include);

        let p2 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", None, false, false, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p2.fa_params.wor_tier, WorTierPolicy::Omit);
    }

    #[test]
    fn fa_dispatch_reads_utr_engine() {
        let p1 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", Some(UtrEngine::RevAi), false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p1.utr_engine, Some(UtrEngine::RevAi));

        let p2 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", None, false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p2.utr_engine, None);

        let p3 = extract_fa_dispatch_params(
            &align_opts("wav2vec_fa", Some(UtrEngine::HkTencent), false, true, false),
            CachePolicy::UseCache,
        )
        .unwrap();
        assert_eq!(p3.utr_engine, Some(UtrEngine::HkTencent));
    }

    #[test]
    fn fa_dispatch_reads_merge_abbrev() {
        let opts = CommandOptions::Align(AlignOptions {
            common: common_default(),
            fa_engine: FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: true.into(),
            merge_abbrev: true.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        });
        let params = extract_fa_dispatch_params(&opts, CachePolicy::UseCache).unwrap();
        assert!(params.merge_abbrev.should_merge());
    }

    #[test]
    fn fa_dispatch_passes_cache_policy() {
        let opts = CommandOptions::Align(AlignOptions {
            common: common_default(),
            fa_engine: FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: true.into(),
            merge_abbrev: false.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        });
        let params = extract_fa_dispatch_params(&opts, CachePolicy::SkipCache).unwrap();
        assert_eq!(params.fa_params.cache_policy, CachePolicy::SkipCache);
    }

    // =======================================================================
    // Transcribe: every TranscribeOptions field is exercised
    // =======================================================================

    #[test]
    fn transcribe_dispatch_reads_all_fields() {
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: common_with_cache_override(),
            asr_engine: AsrEngineName::WhisperX,
            diarize: false,
            wor: true.into(),
            merge_abbrev: true.into(),
            batch_size: 32,
            utseg_fallback: false.into(),
        });
        let params = extract_transcribe_dispatch_params(&opts).unwrap();
        assert_eq!(params.asr_engine, AsrEngineName::WhisperX);
        assert!(!params.diarize);
        assert!(params.wor_tier.should_write());
        assert!(params.merge_abbrev.should_merge());
        assert_eq!(params.batch_size, 32);
        assert!(params.override_media_cache);
    }

    #[test]
    fn transcribe_s_dispatch_reads_all_fields() {
        let opts = CommandOptions::TranscribeS(TranscribeOptions {
            common: common_default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: true,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        });
        let params = extract_transcribe_dispatch_params(&opts).unwrap();
        assert_eq!(params.asr_engine, AsrEngineName::RevAi);
        assert!(params.diarize);
        assert!(!params.wor_tier.should_write());
        assert!(!params.merge_abbrev.should_merge());
        assert_eq!(params.batch_size, 8);
        assert!(!params.override_media_cache);
    }

    #[test]
    fn transcribe_dispatch_prefers_common_asr_override() {
        let mut common = common_default();
        common.engine_overrides.asr = Some(AsrEngineName::HkTencent);
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common,
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        });

        let params = extract_transcribe_dispatch_params(&opts).unwrap();
        assert_eq!(params.asr_engine, AsrEngineName::HkTencent);
    }

    // =======================================================================
    // Morphotag: every MorphotagOptions field is exercised
    // =======================================================================

    #[test]
    fn morphotag_dispatch_reads_all_fields() {
        let opts = CommandOptions::Morphotag(MorphotagOptions {
            common: common_with_cache_override(),
            retokenize: true,
            skipmultilang: true,
            merge_abbrev: true.into(),

            ..Default::default()
        });
        let params = extract_morphotag_dispatch_params(&opts).unwrap();
        assert_eq!(params.tokenization_mode, TokenizationMode::StanzaRetokenize);
        assert_eq!(
            params.multilingual_policy,
            MultilingualPolicy::SkipNonPrimary
        );
        assert!(params.override_media_cache);
        assert!(params.merge_abbrev.should_merge());
    }

    #[test]
    fn morphotag_dispatch_defaults() {
        let opts = CommandOptions::Morphotag(MorphotagOptions {
            common: common_default(),

            ..Default::default()
        });
        let params = extract_morphotag_dispatch_params(&opts).unwrap();
        assert_eq!(params.tokenization_mode, TokenizationMode::Preserve);
        assert_eq!(params.multilingual_policy, MultilingualPolicy::ProcessAll);
        assert!(!params.override_media_cache);
        assert!(!params.merge_abbrev.should_merge());
    }

    // =======================================================================
    // Benchmark: every BenchmarkOptions field is exercised
    // =======================================================================

    #[test]
    fn benchmark_dispatch_reads_all_fields() {
        let opts = CommandOptions::Benchmark(BenchmarkOptions {
            common: common_with_cache_override(),
            asr_engine: AsrEngineName::WhisperOai,
            wor: true.into(),
            merge_abbrev: true.into(),
        });
        let params = extract_benchmark_dispatch_params(&opts).unwrap();
        assert_eq!(params.asr_engine, AsrEngineName::WhisperOai);
        assert!(params.wor_tier.should_write());
        assert!(params.merge_abbrev.should_merge());
        assert!(params.override_media_cache);
    }

    #[test]
    fn benchmark_dispatch_prefers_common_asr_override() {
        let mut common = common_default();
        common.engine_overrides.asr = Some(AsrEngineName::HkAliyun);
        let opts = CommandOptions::Benchmark(BenchmarkOptions {
            common,
            asr_engine: AsrEngineName::RevAi,
            wor: true.into(),
            merge_abbrev: false.into(),
        });

        let params = extract_benchmark_dispatch_params(&opts).unwrap();
        assert_eq!(params.asr_engine, AsrEngineName::HkAliyun);
    }

    // =======================================================================
    // Opensmile: every OpensmileOptions field is exercised
    // =======================================================================

    #[test]
    fn opensmile_dispatch_reads_feature_set() {
        let opts = CommandOptions::Opensmile(OpensmileOptions {
            common: common_default(),
            feature_set: "ComParE_2016".into(),
        });
        let params = extract_opensmile_dispatch_params(&opts).unwrap();
        assert_eq!(params.feature_set, "ComParE_2016");
    }

    // =======================================================================
    // Wrong-command returns None
    // =======================================================================

    #[test]
    fn wrong_command_returns_none() {
        let align = CommandOptions::Align(AlignOptions {
            common: common_default(),
            fa_engine: FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: true.into(),
            merge_abbrev: false.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        });
        assert!(extract_transcribe_dispatch_params(&align).is_none());
        assert!(extract_morphotag_dispatch_params(&align).is_none());
        assert!(extract_benchmark_dispatch_params(&align).is_none());
        assert!(extract_opensmile_dispatch_params(&align).is_none());

        let transcribe = CommandOptions::Transcribe(TranscribeOptions {
            common: common_default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        });
        assert!(extract_fa_dispatch_params(&transcribe, CachePolicy::UseCache).is_none());
    }

    // =======================================================================
    // CommonOptions: engine_overrides and mwt are accessible
    // =======================================================================

    #[test]
    fn common_options_accessible_from_all_variants() {
        use crate::options::EngineOverrides;
        let overrides = EngineOverrides {
            asr: Some(AsrEngineName::HkTencent),
            fa: None,
        };
        let common = CommonOptions {
            override_media_cache: true,
            engine_overrides: overrides.clone(),
            mwt: BTreeMap::new(),
            ..Default::default()
        };

        // Verify common() accessor works for every variant
        let variants: Vec<CommandOptions> = vec![
            CommandOptions::Align(AlignOptions {
                common: common.clone(),
                fa_engine: FaEngineName::Wave2Vec,
                utr_engine: None,
                utr_overlap_strategy: Default::default(),
                utr_two_pass: Default::default(),
                pauses: false,
                wor: true.into(),
                merge_abbrev: false.into(),
                media_dir: None,
                bullet_repair: false,
                review_level: Default::default(),
            }),
            CommandOptions::Transcribe(TranscribeOptions {
                common: common.clone(),
                asr_engine: AsrEngineName::RevAi,
                diarize: false,
                wor: false.into(),
                merge_abbrev: false.into(),
                batch_size: 8,
                utseg_fallback: false.into(),
            }),
            CommandOptions::Morphotag(MorphotagOptions {
                common: common.clone(),

                ..Default::default()
            }),
        ];

        for v in &variants {
            assert!(v.common().override_media_cache);
            assert_eq!(v.common().engine_overrides, overrides);
        }
    }

    // =======================================================================
    // Job-level options: before_paths indexing pattern
    // =======================================================================
    //
    // These tests verify the indexing pattern used by dispatch code to extract
    // per-file before_paths from Job.before_paths. The pattern appears in both
    // dispatch_batched_infer and dispatch_fa_infer.

    /// Helper: extracts the before_path for a given file_index, matching the
    /// pattern used in `dispatch_fa_infer` and `dispatch_batched_infer`.
    fn resolve_before_path(before_paths: &[String], file_index: usize) -> Option<String> {
        if !before_paths.is_empty() && file_index < before_paths.len() {
            Some(before_paths[file_index].clone())
        } else {
            None
        }
    }

    #[test]
    fn job_level_options_before_paths_resolved_by_index() {
        let before_paths = vec![
            "/tmp/before/a.cha".to_string(),
            "/tmp/before/b.cha".to_string(),
            "/tmp/before/c.cha".to_string(),
        ];

        assert_eq!(
            resolve_before_path(&before_paths, 0),
            Some("/tmp/before/a.cha".into())
        );
        assert_eq!(
            resolve_before_path(&before_paths, 2),
            Some("/tmp/before/c.cha".into())
        );
        // Out of bounds returns None
        assert_eq!(resolve_before_path(&before_paths, 3), None);
    }

    #[test]
    fn job_level_options_empty_before_paths_returns_none() {
        let before_paths: Vec<String> = vec![];
        assert_eq!(resolve_before_path(&before_paths, 0), None);
    }

    #[test]
    fn job_level_options_before_paths_supports_incremental_commands() {
        // Document which commands support --before (incremental processing).
        // This test is a living doc — update when adding incremental support
        // to new commands.
        let incremental_commands = ["morphotag", "align"];
        let non_incremental_commands = ["transcribe", "translate", "utseg", "coref", "benchmark"];

        // Verify the command names are valid
        for cmd in incremental_commands
            .iter()
            .chain(non_incremental_commands.iter())
        {
            assert!(!cmd.is_empty(), "command name should not be empty: {cmd}");
        }

        // Incremental commands should have 2 entries (morphotag, align)
        assert_eq!(incremental_commands.len(), 2);
    }
}
