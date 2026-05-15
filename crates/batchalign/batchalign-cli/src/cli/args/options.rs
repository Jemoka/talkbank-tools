//! Shared option-building logic for job submission.
//!
//! [`build_typed_options()`] converts parsed CLI args into a [`CommandOptions`]
//! enum variant for type-safe job submission.

use crate::chat_ops::CacheTaskName;
use crate::chat_ops::fa::CaMarkerPolicy as AppCaMarkerPolicy;
use crate::options::{
    AlignOptions, AsrEngineName, AvqiOptions, BenchmarkOptions, CommandOptions, CommonOptions,
    CompareOptions, CorefOptions, EngineOverrides, FaEngineName, MorphotagOptions,
    OpensmileOptions, TranscribeOptions, TranslateOptions, UtrEngine as AppUtrEngine,
    UtrOverlapStrategy as AppUtrOverlapStrategy, UtsegOptions,
};
use crate::params::{CacheOverrides, MergeAbbrevPolicy, WorTierPolicy};

use super::{
    AsrEngine, BenchAsrEngine, CaMarkerPolicy as CliCaMarkerPolicy, Commands, CommonOpts,
    DiarizationMode, FaEngine, GlobalOpts, UtrEngine as CliUtrEngine,
    UtrOverlapStrategy as CliUtrOverlapStrategy,
};

/// Parse one `--engine-overrides` JSON payload into typed `EngineOverrides`.
///
/// Rejects unknown keys and invalid engine names at parse time.
pub(crate) fn parse_engine_overrides_json(input: &str) -> Result<EngineOverrides, String> {
    serde_json::from_str::<EngineOverrides>(input)
        .map_err(|error| format!("invalid --engine-overrides JSON: {error}"))
}

/// Parse an optional JSON string into typed `EngineOverrides`.
///
/// Returns default (empty) overrides if `input` is `None` or empty.
pub fn parse_engine_overrides(input: &Option<String>) -> EngineOverrides {
    // clap-validation invariant: `--engine-overrides` is parsed by
    // `clap::ValueParser` which calls `parse_engine_overrides_json`
    // and rejects invalid input at the CLI parse boundary. Reaching
    // this expect would mean the validator drifted out of sync with
    // the canonical parser — caught by the args integration tests.
    #[allow(clippy::expect_used)]
    match input.as_deref() {
        None | Some("") => EngineOverrides::default(),
        Some(json) => parse_engine_overrides_json(json)
            .expect("clap should reject invalid --engine-overrides JSON before option building"),
    }
}

/// Resolve a simple `--foo` / `--no-foo` pair after clap has applied defaults.
fn resolve_flag_pair(enabled: bool, disabled: bool) -> bool {
    enabled && !disabled
}

/// Resolve the merge-abbreviation option family into a typed policy.
fn resolve_merge_abbrev_policy(enabled: bool, disabled: bool) -> MergeAbbrevPolicy {
    resolve_flag_pair(enabled, disabled).into()
}

/// Resolve the `%wor` option family into a typed policy.
fn resolve_wor_tier_policy(enabled: bool, disabled: bool) -> WorTierPolicy {
    resolve_flag_pair(enabled, disabled).into()
}

/// Parse a wire name into a [`CacheTaskName`].
///
/// Only audio tasks are cached. Text-task names are accepted for
/// backward-compatible CLI scripting but resolve to `None` with a
/// warning.
fn parse_cache_task(name: &str) -> Option<CacheTaskName> {
    match name.trim() {
        "utr_asr" => Some(CacheTaskName::UtrAsr),
        "forced_alignment" => Some(CacheTaskName::ForcedAlignment),
        "morphosyntax" | "utterance_segmentation" | "translation" => {
            eprintln!(
                "warning: --override-media-cache-tasks {name} ignored \
                 (batchalign3 does not cache text NLP)"
            );
            None
        }
        _ => {
            eprintln!("warning: unknown cache task name '{name}', ignoring");
            None
        }
    }
}

/// Resolve the cache override policy from CLI flags.
pub fn resolve_cache_overrides(global: &GlobalOpts) -> CacheOverrides {
    if !global.override_media_cache_tasks.is_empty() {
        let tasks = global
            .override_media_cache_tasks
            .iter()
            .filter_map(|s| parse_cache_task(s))
            .collect();
        CacheOverrides::Tasks(tasks)
    } else if global.override_media_cache {
        CacheOverrides::All
    } else {
        CacheOverrides::None
    }
}

/// Resolve a `--debug-dir` path to absolute form for transmission to the
/// server. The server interprets `CommonOptions::debug_dir` against its own
/// working directory, which on a remote daemon is opaque to the user and on
/// a local daemon is rarely the user's `cwd`. By canonicalizing on the
/// client we eliminate the cross-process path-frame ambiguity.
///
/// Uses `std::path::absolute` (Rust 1.79+) which works on non-existent paths
/// (unlike `canonicalize`); the directory is created lazily when the server
/// first writes to it. The result stays in the typed `PathBuf` domain;
/// stringification for the wire `CommonOptions::debug_dir: Option<String>`
/// happens at the boundary in `build_typed_options`.
fn canonicalize_debug_dir(p: &std::path::Path) -> std::path::PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Build typed command options from parsed CLI args.
///
/// Returns `None` for non-processing commands (serve, jobs, version, etc.).
pub fn build_typed_options(cmd: &Commands, global: &GlobalOpts) -> Option<CommandOptions> {
    let common = CommonOptions {
        override_media_cache: global.override_media_cache,
        engine_overrides: parse_engine_overrides(&global.engine_overrides),
        debug_dir: global.debug_dir.as_deref().map(canonicalize_debug_dir),
        override_media_cache_tasks: global.override_media_cache_tasks.clone(),
        batch_window: global.batch_window,
        ..Default::default()
    };

    match cmd {
        Commands::Align(a) => {
            let fa_engine = if let Some(engine) = a.fa_engine_custom.as_deref() {
                FaEngineName::from_wire_name(engine).ok()?
            } else if a.whisper_fa {
                // BA2 compat alias
                FaEngineName::Whisper
            } else {
                match a.fa_engine {
                    FaEngine::Wav2vec => FaEngineName::Wave2Vec,
                    FaEngine::Whisper => FaEngineName::Whisper,
                }
            };
            let utr_engine = if a.utr && !a.no_utr {
                let utr = if let Some(engine) = a.utr_engine_custom.as_deref() {
                    AppUtrEngine::from_wire_name(engine).ok()?
                } else if a.whisper && !a.rev {
                    // BA2 compat alias
                    AppUtrEngine::Whisper
                } else {
                    match a.utr_engine {
                        CliUtrEngine::Rev => AppUtrEngine::RevAi,
                        CliUtrEngine::Whisper => AppUtrEngine::Whisper,
                    }
                };
                Some(utr)
            } else {
                None
            };
            let utr_overlap_strategy = match a.utr_strategy {
                CliUtrOverlapStrategy::Auto => AppUtrOverlapStrategy::Auto,
                CliUtrOverlapStrategy::Global => AppUtrOverlapStrategy::Global,
                CliUtrOverlapStrategy::TwoPass => AppUtrOverlapStrategy::TwoPass,
            };
            let utr_ca_markers = match a.utr_ca_markers {
                CliCaMarkerPolicy::Enabled => AppCaMarkerPolicy::Enabled,
                CliCaMarkerPolicy::Disabled => AppCaMarkerPolicy::Disabled,
            };
            Some(CommandOptions::Align(AlignOptions {
                common,
                fa_engine,
                utr_engine,
                utr_overlap_strategy,
                utr_two_pass: crate::chat_ops::fa::TwoPassConfig {
                    ca_markers: utr_ca_markers,
                    max_exclusion_density: a.utr_density_threshold,
                    tight_buffer_ms: a.utr_tight_buffer,
                    match_mode: match a.utr_fuzzy {
                        Some(threshold) => crate::chat_ops::fa::UtrMatchMode::Fuzzy { threshold },
                        None => crate::chat_ops::fa::TwoPassConfig::default().match_mode,
                    },
                },
                pauses: a.pauses,
                wor: resolve_wor_tier_policy(a.wor, a.nowor),
                merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
                bullet_repair: a.bullet_repair,
                review_level: match a.review_level {
                    super::commands::CliReviewLevel::None => crate::chat_ops::fa::ReviewLevel::None,
                    super::commands::CliReviewLevel::LowConfidence => {
                        crate::chat_ops::fa::ReviewLevel::LowConfidence
                    }
                    super::commands::CliReviewLevel::All => crate::chat_ops::fa::ReviewLevel::All,
                },
                media_dir: a.media_dir.clone(),
            }))
        }
        Commands::Transcribe(a) => {
            let asr_engine = if let Some(engine) = a.asr_engine_custom.as_deref() {
                AsrEngineName::from_wire_name(engine).ok()?
            } else if a.whisperx {
                // BA2 compat alias
                AsrEngineName::WhisperX
            } else if a.whisper_oai {
                // BA2 compat alias
                AsrEngineName::WhisperOai
            } else if a.whisper {
                // BA2 compat alias
                AsrEngineName::Whisper
            } else if a.rev {
                // BA2 compat alias
                AsrEngineName::RevAi
            } else {
                match a.asr_engine {
                    AsrEngine::Rev => AsrEngineName::RevAi,
                    AsrEngine::Whisper => AsrEngineName::Whisper,
                    AsrEngine::WhisperHub => AsrEngineName::WhisperHub,
                    AsrEngine::WhisperX => AsrEngineName::WhisperX,
                    AsrEngine::WhisperOai => AsrEngineName::WhisperOai,
                }
            };
            // Resolve diarization: BA2 compat bools override the enum
            let diarize = if a.diarize {
                true
            } else if a.nodiarize {
                false
            } else {
                match a.diarization {
                    DiarizationMode::Auto | DiarizationMode::Disabled => false,
                    DiarizationMode::Enabled => true,
                }
            };
            let variant = TranscribeOptions {
                common,
                asr_engine,
                diarize,
                wor: resolve_wor_tier_policy(a.wor, a.nowor),
                merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
                utseg_fallback: a.utseg_fallback_stanza.into(),
                batch_size: 8,
            };
            if diarize {
                Some(CommandOptions::TranscribeS(variant))
            } else {
                Some(CommandOptions::Transcribe(variant))
            }
        }
        Commands::Translate(a) => Some(CommandOptions::Translate(TranslateOptions {
            common,
            merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
        })),
        Commands::Morphotag(a) => Some(CommandOptions::Morphotag(MorphotagOptions {
            common,
            retokenize: a.retokenize && !a.keeptokens,
            skipmultilang: a.skipmultilang && !a.multilang,
            merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
            // Keep the domain / JSON field name so the wire format remains
            // stable while the public CLI stays default-on with an explicit
            // opt-out flag.
            no_l2_morphotag: a.no_l2_morphotag,
            no_pos_hints: a.no_pos_hints,
        })),
        Commands::Coref(a) => Some(CommandOptions::Coref(CorefOptions {
            common,
            merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
        })),
        Commands::Utseg(a) => Some(CommandOptions::Utseg(UtsegOptions {
            common,
            merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
            utseg_fallback: a.utseg_fallback_stanza.into(),
        })),
        Commands::Benchmark(a) => {
            let asr_engine = if let Some(engine) = a.asr_engine_custom.as_deref() {
                AsrEngineName::from_wire_name(engine).ok()?
            } else if a.whisper_oai {
                // BA2 compat alias
                AsrEngineName::WhisperOai
            } else if a.whisper {
                // BA2 compat alias
                AsrEngineName::Whisper
            } else if a.rev {
                // BA2 compat alias
                AsrEngineName::RevAi
            } else {
                match a.asr_engine {
                    BenchAsrEngine::Rev => AsrEngineName::RevAi,
                    BenchAsrEngine::Whisper => AsrEngineName::Whisper,
                    BenchAsrEngine::WhisperOai => AsrEngineName::WhisperOai,
                }
            };
            Some(CommandOptions::Benchmark(BenchmarkOptions {
                common,
                asr_engine,
                wor: resolve_wor_tier_policy(a.wor, a.nowor),
                merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
            }))
        }
        Commands::Opensmile(a) => Some(CommandOptions::Opensmile(OpensmileOptions {
            common,
            feature_set: a.feature_set.clone(),
        })),
        Commands::Compare(a) => Some(CommandOptions::Compare(CompareOptions {
            common,
            merge_abbrev: resolve_merge_abbrev_policy(a.merge_abbrev, a.no_merge_abbrev),
        })),
        Commands::Avqi(_) => Some(CommandOptions::Avqi(AvqiOptions { common })),
        _ => None,
    }
}

/// Extract `CommonOpts` from a processing command, if present.
pub fn common_opts(cmd: &Commands) -> Option<&CommonOpts> {
    match cmd {
        Commands::Align(a) => Some(&a.common),
        Commands::Transcribe(a) => Some(&a.common),
        Commands::Translate(a) => Some(&a.common),
        Commands::Morphotag(a) => Some(&a.common),
        Commands::Coref(a) => Some(&a.common),
        Commands::Utseg(a) => Some(&a.common),
        Commands::Benchmark(a) => Some(&a.common),
        Commands::Compare(a) => Some(&a.common),
        _ => None,
    }
}

/// Extract `--before` from commands that support incremental processing.
pub fn extract_before(cmd: &Commands) -> Option<&std::path::Path> {
    match cmd {
        Commands::Align(a) => a.incremental.before.as_deref(),
        Commands::Morphotag(a) => a.incremental.before.as_deref(),
        _ => None,
    }
}

/// Extract --bank from a processing command, if applicable.
pub fn extract_bank(cmd: &Commands) -> Option<&str> {
    match cmd {
        Commands::Benchmark(a) => a.bank.as_deref(),
        Commands::Opensmile(a) => a.bank.as_deref(),
        _ => None,
    }
}

/// Extract --subdir from a processing command, if applicable.
pub fn extract_subdir(cmd: &Commands) -> Option<&str> {
    match cmd {
        Commands::Benchmark(a) => a.subdir.as_deref(),
        Commands::Opensmile(a) => a.subdir.as_deref(),
        _ => None,
    }
}

/// Extract --lexicon path from morphotag, if present.
pub fn extract_lexicon(cmd: &Commands) -> Option<&str> {
    match cmd {
        Commands::Morphotag(a) => a.lexicon.as_deref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_engine_overrides_none() {
        assert!(parse_engine_overrides(&None).is_empty());
    }

    #[test]
    fn parse_engine_overrides_empty_string() {
        assert!(parse_engine_overrides(&Some(String::new())).is_empty());
    }

    #[test]
    fn parse_engine_overrides_valid_json() {
        let input = Some(r#"{"asr": "tencent", "fa": "cantonese_fa"}"#.to_string());
        let overrides = parse_engine_overrides(&input);
        assert_eq!(overrides.asr, Some(AsrEngineName::HkTencent));
        assert_eq!(overrides.fa, Some(FaEngineName::Wav2vecCanto));
    }

    #[test]
    fn parse_engine_overrides_json_rejects_invalid_shape() {
        let error = parse_engine_overrides_json(r#"{"asr":{"name":"whisper"}}"#)
            .expect_err("nested objects should be rejected");
        assert!(error.contains("invalid"));
    }

    #[test]
    fn parse_engine_overrides_empty_object() {
        let input = Some("{}".to_string());
        assert!(parse_engine_overrides(&input).is_empty());
    }

    #[test]
    fn parse_engine_overrides_rejects_unknown_key() {
        let error = parse_engine_overrides_json(r#"{"mor": "custom_mor"}"#)
            .expect_err("unknown engine category should be rejected");
        assert!(error.contains("unknown engine override key"));
    }

    /// `--debug-dir` is interpreted by the server (not the client). When the
    /// CLI submits a job, a relative `--debug-dir` value gets resolved against
    /// the *server's* working directory, which on a remote daemon is opaque
    /// to the user and on a local daemon is rarely the user's `cwd`. The
    /// client must canonicalize the path to absolute form before sending.
    ///
    /// Bug report (2026-04-19): running
    /// `batchalign3 transcribe in -o out2 --debug-dir debug2` against a local
    /// daemon resulted in artifacts landing at the *daemon's* working
    /// directory rather than the client's. Canonicalizing on the client side
    /// before submission fixes the asymmetry.
    #[test]
    fn canonicalize_debug_dir_resolves_relative_to_absolute() {
        let absolute = canonicalize_debug_dir(std::path::Path::new("debug2"));
        assert!(
            absolute.is_absolute(),
            "expected canonicalize_debug_dir to return absolute path, got: {}",
            absolute.display()
        );
    }

    #[test]
    fn canonicalize_debug_dir_preserves_already_absolute() {
        let input = std::path::Path::new("/tmp/already_absolute");
        let out = canonicalize_debug_dir(input);
        assert_eq!(out, std::path::PathBuf::from("/tmp/already_absolute"));
    }
}
