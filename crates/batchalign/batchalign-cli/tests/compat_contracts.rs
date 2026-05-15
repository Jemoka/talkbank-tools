//! Contract tests for the hidden batchalign2-compatible CLI surface.
//!
//! These tests intentionally exercise the public clap parser and typed-option
//! builder so we preserve representative BA2 aliases, precedence rules, and
//! downstream dispatch invariants without requiring real ML runs.

mod cli_common;

use batchalign::cli::args::{Cli, CommonOpts, build_typed_options};
use batchalign::options::{
    AsrEngineName, CommandOptions, FaEngineName, UtrEngine as AppUtrEngine,
    UtrOverlapStrategy as AppUtrOverlapStrategy,
};
use clap::Parser;
use predicates::prelude::*;

use cli_common::cli_cmd as cmd;

/// Parse one synthetic CLI invocation through the published clap surface.
fn parse_cli(args: &[&str]) -> Cli {
    let argv: Vec<&str> = std::iter::once("batchalign3")
        .chain(args.iter().copied())
        .collect();
    Cli::parse_from(argv)
}

/// Resolve typed job options exactly as the production CLI does before dispatch.
fn typed_options(args: &[&str]) -> CommandOptions {
    let cli = parse_cli(args);
    build_typed_options(&cli.command, &cli.global).expect("processing command should build options")
}

/// Extract the dispatch command name chosen for a processing invocation.
fn dispatch_command(args: &[&str]) -> &'static str {
    let cli = parse_cli(args);
    CommonOpts::command_profile(&cli.command).command.as_str()
}

/// Assert that a compat path reaches the normal missing-input validation path.
fn assert_missing_input_usage_error(args: &[&str]) {
    cmd()
        .args(args)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("no input paths"));
}

/// Representative canonical/compat pairs that should stay semantically equal.
struct EquivalenceCase {
    canonical: &'static [&'static str],
    compat: &'static [&'static str],
    note: &'static str,
}

#[test]
fn representative_batchalign2_aliases_match_canonical_typed_options() {
    let cases = [
        EquivalenceCase {
            canonical: &["align", "--fa-engine", "whisper", "corpus/"],
            compat: &["align", "--whisper-fa", "corpus/"],
            note: "align whisper-fa alias",
        },
        EquivalenceCase {
            canonical: &["align", "--utr-engine", "whisper", "corpus/"],
            compat: &["align", "--whisper", "corpus/"],
            note: "align whisper UTR alias",
        },
        EquivalenceCase {
            canonical: &["align", "--utr-engine", "rev", "corpus/"],
            compat: &["align", "--rev", "corpus/"],
            note: "align rev UTR alias",
        },
        EquivalenceCase {
            canonical: &["transcribe", "--asr-engine", "whisperx", "audio/"],
            compat: &["transcribe", "--whisperx", "audio/"],
            note: "transcribe whisperx alias",
        },
        EquivalenceCase {
            canonical: &["transcribe", "--asr-engine", "rev", "audio/"],
            compat: &["transcribe", "--rev", "audio/"],
            note: "transcribe rev alias",
        },
        EquivalenceCase {
            canonical: &["transcribe", "--diarization", "enabled", "audio/"],
            compat: &["transcribe", "--diarize", "audio/"],
            note: "transcribe diarize alias",
        },
        EquivalenceCase {
            canonical: &["transcribe", "--diarization", "disabled", "audio/"],
            compat: &["transcribe", "--nodiarize", "audio/"],
            note: "transcribe nodiarize alias",
        },
        EquivalenceCase {
            canonical: &["benchmark", "--asr-engine", "whisper-oai", "audio/"],
            compat: &["benchmark", "--whisper-oai", "audio/"],
            note: "benchmark whisper-oai alias",
        },
        EquivalenceCase {
            canonical: &["benchmark", "--asr-engine", "rev", "audio/"],
            compat: &["benchmark", "--rev", "audio/"],
            note: "benchmark rev alias",
        },
    ];

    for case in cases {
        assert_eq!(
            typed_options(case.canonical),
            typed_options(case.compat),
            "compat form should resolve like canonical form for {}",
            case.note
        );
    }
}

#[test]
fn compat_engine_aliases_override_canonical_enums_before_dispatch() {
    match typed_options(&["align", "--fa-engine", "wav2vec", "--whisper-fa", "corpus/"]) {
        CommandOptions::Align(options) => assert_eq!(options.fa_engine, FaEngineName::Whisper),
        other => panic!("expected Align options, got {other:?}"),
    }

    match typed_options(&["align", "--utr-engine", "rev", "--whisper", "corpus/"]) {
        CommandOptions::Align(options) => {
            assert_eq!(options.utr_engine, Some(AppUtrEngine::Whisper));
        }
        other => panic!("expected Align options, got {other:?}"),
    }

    match typed_options(&["transcribe", "--asr-engine", "rev", "--whisperx", "audio/"]) {
        CommandOptions::Transcribe(options) => {
            assert_eq!(options.asr_engine, AsrEngineName::WhisperX)
        }
        other => panic!("expected Transcribe options, got {other:?}"),
    }

    match typed_options(&[
        "benchmark",
        "--asr-engine",
        "rev",
        "--whisper-oai",
        "audio/",
    ]) {
        CommandOptions::Benchmark(options) => {
            assert_eq!(options.asr_engine, AsrEngineName::WhisperOai)
        }
        other => panic!("expected Benchmark options, got {other:?}"),
    }
}

#[test]
fn align_utr_strategy_defaults_to_auto_and_maps_explicit_variants() {
    match typed_options(&["align", "corpus/"]) {
        CommandOptions::Align(options) => {
            assert_eq!(options.utr_overlap_strategy, AppUtrOverlapStrategy::Auto)
        }
        other => panic!("expected Align options, got {other:?}"),
    }

    for (flag, expected) in [
        ("auto", AppUtrOverlapStrategy::Auto),
        ("global", AppUtrOverlapStrategy::Global),
        ("two-pass", AppUtrOverlapStrategy::TwoPass),
    ] {
        match typed_options(&["align", "--utr-strategy", flag, "corpus/"]) {
            CommandOptions::Align(options) => {
                assert_eq!(
                    options.utr_overlap_strategy, expected,
                    "--utr-strategy {flag} should map to the typed overlap strategy"
                );
            }
            other => panic!("expected Align options, got {other:?}"),
        }
    }
}

#[test]
fn custom_engine_names_override_hidden_aliases_and_canonical_enums() {
    match typed_options(&[
        "align",
        "--fa-engine",
        "whisper",
        "--whisper-fa",
        "--fa-engine-custom",
        "cantonese_fa",
        "corpus/",
    ]) {
        CommandOptions::Align(options) => assert_eq!(options.fa_engine, FaEngineName::Wav2vecCanto),
        other => panic!("expected Align options, got {other:?}"),
    }

    match typed_options(&[
        "align",
        "--utr-engine",
        "whisper",
        "--whisper",
        "--utr-engine-custom",
        "tencent_utr",
        "corpus/",
    ]) {
        CommandOptions::Align(options) => {
            assert_eq!(options.utr_engine, Some(AppUtrEngine::HkTencent))
        }
        other => panic!("expected Align options, got {other:?}"),
    }

    match typed_options(&[
        "transcribe",
        "--asr-engine",
        "whisper",
        "--whisperx",
        "--asr-engine-custom",
        "tencent",
        "audio/",
    ]) {
        CommandOptions::Transcribe(options) => {
            assert_eq!(options.asr_engine, AsrEngineName::HkTencent)
        }
        other => panic!("expected Transcribe options, got {other:?}"),
    }

    match typed_options(&[
        "benchmark",
        "--asr-engine",
        "whisper",
        "--whisper-oai",
        "--asr-engine-custom",
        "funaudio",
        "audio/",
    ]) {
        CommandOptions::Benchmark(options) => {
            assert_eq!(options.asr_engine, AsrEngineName::HkFunaudio)
        }
        other => panic!("expected Benchmark options, got {other:?}"),
    }
}

#[test]
fn compat_diarization_flags_override_enum_and_preserve_dispatch_contract() {
    match typed_options(&[
        "transcribe",
        "--diarization",
        "disabled",
        "--diarize",
        "audio/",
    ]) {
        CommandOptions::TranscribeS(options) => assert!(options.diarize),
        other => panic!("expected TranscribeS options, got {other:?}"),
    }
    assert_eq!(
        dispatch_command(&[
            "transcribe",
            "--diarization",
            "disabled",
            "--diarize",
            "audio/"
        ]),
        "transcribe_s"
    );

    match typed_options(&[
        "transcribe",
        "--diarization",
        "enabled",
        "--nodiarize",
        "audio/",
    ]) {
        CommandOptions::Transcribe(options) => assert!(!options.diarize),
        other => panic!("expected Transcribe options, got {other:?}"),
    }
    assert_eq!(
        dispatch_command(&[
            "transcribe",
            "--diarization",
            "enabled",
            "--nodiarize",
            "audio/"
        ]),
        "transcribe"
    );

    match typed_options(&["transcribe", "--diarize", "--nodiarize", "audio/"]) {
        CommandOptions::TranscribeS(options) => assert!(options.diarize),
        other => panic!("expected TranscribeS options, got {other:?}"),
    }
    assert_eq!(
        dispatch_command(&["transcribe", "--diarize", "--nodiarize", "audio/"]),
        "transcribe_s",
        "--diarize must remain the deciding compat flag when both BA2 bools are present"
    );
}

#[test]
fn compat_paths_reach_the_same_usage_error_path_as_canonical_forms() {
    let cases = [
        &["align", "--whisper-fa"][..],
        &["align", "--fa-engine", "whisper"][..],
        &["transcribe", "--whisper"][..],
        &["transcribe", "--asr-engine", "whisper"][..],
        &["transcribe", "--diarize"][..],
        &["transcribe", "--diarization", "enabled"][..],
    ];

    for args in cases {
        assert_missing_input_usage_error(args);
    }
}
