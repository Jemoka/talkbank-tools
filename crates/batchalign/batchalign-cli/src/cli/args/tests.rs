use super::*;
use crate::api::ReleasedCommand;
use crate::options::{AsrEngineName, CommandOptions, FaEngineName, UtrEngine as AppUtrEngine};
use clap::{CommandFactory, Parser};
use rstest::rstest;
use std::path::{Path, PathBuf};

fn typed_options_for(args: &[&str]) -> CommandOptions {
    let cli = Cli::parse_from(args);
    build_typed_options(&cli.command, &cli.global).unwrap()
}

fn assert_typed_options_equivalent(lhs: &[&str], rhs: &[&str], note: &str) {
    let lhs_opts = typed_options_for(lhs);
    let rhs_opts = typed_options_for(rhs);
    assert_eq!(
        lhs_opts, rhs_opts,
        "{note}\nleft args: {:?}\nright args: {:?}",
        lhs, rhs
    );
}

fn assert_parse_error_contains(args: &[&str], fragments: &[&str]) {
    let error = Cli::try_parse_from(args)
        .expect_err("CLI parse should fail for conflicting arguments")
        .to_string();
    for fragment in fragments {
        assert!(
            error.contains(fragment),
            "expected parse error to contain {:?}, got `{error}`",
            fragment
        );
    }
}

fn render_subcommand_help(name: &str) -> String {
    let mut cmd = Cli::command();
    let sub = cmd
        .find_subcommand_mut(name)
        .unwrap_or_else(|| panic!("missing subcommand {name}"));
    let mut buf = Vec::new();
    sub.write_long_help(&mut buf).expect("write help");
    String::from_utf8(buf).expect("utf8 help")
}

fn merge_abbrev_value(options: &CommandOptions) -> Option<bool> {
    match options {
        CommandOptions::Align(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Transcribe(opts) | CommandOptions::TranscribeS(opts) => {
            Some(opts.merge_abbrev.should_merge())
        }
        CommandOptions::Translate(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Morphotag(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Coref(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Compare(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Utseg(opts) => Some(opts.merge_abbrev.should_merge()),
        CommandOptions::Benchmark(opts) => Some(opts.merge_abbrev.should_merge()),
        _ => None,
    }
}

fn wor_value(options: &CommandOptions) -> Option<bool> {
    match options {
        CommandOptions::Align(opts) => Some(opts.wor.should_write()),
        CommandOptions::Transcribe(opts) | CommandOptions::TranscribeS(opts) => {
            Some(opts.wor.should_write())
        }
        CommandOptions::Benchmark(opts) => Some(opts.wor.should_write()),
        _ => None,
    }
}

#[test]
fn parse_morphotag() {
    let cli = Cli::parse_from(["batchalign3", "morphotag", "corpus/"]);
    assert!(matches!(cli.command, Commands::Morphotag(_)));
}

#[test]
fn morphotag_defaults_l2_on() {
    let options = typed_options_for(&["batchalign3", "morphotag", "corpus/"]);
    match options {
        CommandOptions::Morphotag(opts) => {
            assert!(
                !opts.no_l2_morphotag,
                "default morphotag invocation should keep L2 dispatch on"
            );
        }
        other => panic!("expected Morphotag options, got {other:?}"),
    }
}

#[test]
fn morphotag_no_l2_flag_opts_out() {
    let options = typed_options_for(&["batchalign3", "morphotag", "corpus/", "--no-l2-morphotag"]);
    match options {
        CommandOptions::Morphotag(opts) => {
            assert!(
                opts.no_l2_morphotag,
                "--no-l2-morphotag should disable L2 dispatch"
            );
        }
        other => panic!("expected Morphotag options, got {other:?}"),
    }
}

#[test]
fn morphotag_rejects_removed_positive_l2_flag() {
    assert_parse_error_contains(
        &["batchalign3", "morphotag", "corpus/", "--l2-morphotag"],
        &["unexpected argument '--l2-morphotag'"],
    );
}

#[test]
fn parse_align_with_options() {
    let cli = Cli::parse_from([
        "batchalign3",
        "--verbose",
        "align",
        "input/",
        "-o",
        "output/",
        "--whisper-fa",
        "--pauses",
    ]);
    assert_eq!(cli.global.verbose, 1);
    if let Commands::Align(a) = &cli.command {
        assert!(a.whisper_fa);
        assert!(a.pauses);
        assert_eq!(a.common.output.as_deref(), Some(Path::new("output/")));
    } else {
        panic!("expected Align");
    }
}

#[test]
fn parse_align_with_engine_overrides() {
    let cli = Cli::parse_from([
        "batchalign3",
        "align",
        "input/",
        "--fa-engine-custom",
        "wav2vec_fa_canto",
        "--utr-engine-custom",
        "tencent_utr",
    ]);
    if let Commands::Align(a) = &cli.command {
        assert_eq!(a.fa_engine_custom.as_deref(), Some("wav2vec_fa_canto"));
        assert_eq!(a.utr_engine_custom.as_deref(), Some("tencent_utr"));
    } else {
        panic!("expected Align");
    }
}

#[test]
fn parse_transcribe_with_lang() {
    let cli = Cli::parse_from([
        "batchalign3",
        "transcribe",
        "audio/",
        "--lang",
        "spa",
        "-n",
        "3",
        "--whisperx",
    ]);
    if let Commands::Transcribe(a) = &cli.command {
        assert_eq!(a.lang, "spa");
        assert_eq!(a.num_speakers, 3);
        assert!(a.whisperx);
    } else {
        panic!("expected Transcribe");
    }
}

#[test]
fn parse_transcribe_lang_auto() {
    let cli = Cli::parse_from(["batchalign3", "transcribe", "audio/", "--lang", "auto"]);
    if let Commands::Transcribe(a) = &cli.command {
        assert_eq!(a.lang, "auto", "--lang auto must pass through as-is");
    } else {
        panic!("expected Transcribe");
    }
}

#[test]
fn parse_transcribe_asr_engine_override() {
    let cli = Cli::parse_from([
        "batchalign3",
        "transcribe",
        "audio/",
        "--asr-engine-custom",
        "tencent",
    ]);
    if let Commands::Transcribe(a) = &cli.command {
        assert_eq!(a.asr_engine_custom.as_deref(), Some("tencent"));
    } else {
        panic!("expected Transcribe");
    }
}

// -----------------------------------------------------------------------------
// --utseg-fallback-stanza — operator opt-in to the legacy Stanza
// constituency-parser fallback for utseg when no language-specific
// TalkBank BERT model is configured. Replaces the
// `BA3_UTSEG_FALLBACK_STANZA` env var with a typed CLI surface
// discoverable from `--help`. Exposed on every utseg-invoking
// subcommand: transcribe, transcribe-s, and utseg.
// -----------------------------------------------------------------------------

#[test]
fn parse_transcribe_with_utseg_fallback_stanza() {
    let cli = Cli::parse_from([
        "batchalign3",
        "transcribe",
        "audio/",
        "--lang",
        "spa",
        "--utseg-fallback-stanza",
    ]);
    if let Commands::Transcribe(a) = &cli.command {
        assert!(
            a.utseg_fallback_stanza,
            "--utseg-fallback-stanza must set the flag on TranscribeArgs"
        );
    } else {
        panic!("expected Transcribe");
    }
}

#[test]
fn transcribe_s_typed_options_carry_utseg_fallback_stanza() {
    // The `transcribe_s` shape is produced when `transcribe --diarize`
    // is invoked. The flag must propagate through to the typed
    // `CommandOptions::TranscribeS` variant.
    let opts = typed_options_for(&[
        "batchalign3",
        "transcribe",
        "audio/",
        "--diarize",
        "--utseg-fallback-stanza",
    ]);
    match opts {
        CommandOptions::TranscribeS(o) => assert!(
            o.utseg_fallback.is_allowed(),
            "--utseg-fallback-stanza must reach TranscribeS typed options under --diarize"
        ),
        other => panic!("expected TranscribeS options, got {other:?}"),
    }
}

#[test]
fn parse_utseg_with_utseg_fallback_stanza() {
    let cli = Cli::parse_from([
        "batchalign3",
        "utseg",
        "corpus/",
        "--lang",
        "spa",
        "--utseg-fallback-stanza",
    ]);
    if let Commands::Utseg(a) = &cli.command {
        assert!(
            a.utseg_fallback_stanza,
            "--utseg-fallback-stanza must set the flag on UtsegArgs"
        );
    } else {
        panic!("expected Utseg");
    }
}

#[test]
fn utseg_fallback_default_is_refuse() {
    // Without the flag, every utseg-invoking subcommand must default
    // to refusing the Stanza substitution (BUG-032 invariant).
    let opts = typed_options_for(&["batchalign3", "transcribe", "audio/"]);
    match opts {
        CommandOptions::Transcribe(o) => assert!(
            !o.utseg_fallback.is_allowed(),
            "default Transcribe must refuse Stanza utseg fallback"
        ),
        other => panic!("expected Transcribe options, got {other:?}"),
    }

    let opts = typed_options_for(&["batchalign3", "utseg", "corpus/"]);
    match opts {
        CommandOptions::Utseg(o) => assert!(
            !o.utseg_fallback.is_allowed(),
            "default Utseg must refuse Stanza utseg fallback"
        ),
        other => panic!("expected Utseg options, got {other:?}"),
    }
}

#[test]
fn utseg_fallback_allowed_when_flag_passed() {
    let opts = typed_options_for(&[
        "batchalign3",
        "transcribe",
        "audio/",
        "--utseg-fallback-stanza",
    ]);
    match opts {
        CommandOptions::Transcribe(o) => assert!(
            o.utseg_fallback.is_allowed(),
            "--utseg-fallback-stanza must lift TranscribeOptions.utseg_fallback to AllowStanza"
        ),
        other => panic!("expected Transcribe options, got {other:?}"),
    }
}

#[test]
fn parse_translate_with_file_list_and_output() {
    let cli = Cli::parse_from([
        "batchalign3",
        "translate",
        "--file-list",
        "inputs.txt",
        "-o",
        "output/",
    ]);
    if let Commands::Translate(a) = &cli.command {
        assert!(a.common.paths.is_empty());
        assert_eq!(a.common.file_list.as_deref(), Some(Path::new("inputs.txt")));
        assert_eq!(a.common.output.as_deref(), Some(Path::new("output/")));
    } else {
        panic!("expected Translate");
    }
}

#[test]
fn parse_coref_in_place() {
    let cli = Cli::parse_from(["batchalign3", "coref", "--in-place", "corpus/"]);
    if let Commands::Coref(a) = &cli.command {
        assert!(a.common.in_place);
        assert_eq!(a.common.paths, vec![PathBuf::from("corpus/")]);
    } else {
        panic!("expected Coref");
    }
}

#[test]
fn parse_align_with_before_and_output() {
    let cli = Cli::parse_from([
        "batchalign3",
        "align",
        "corpus/",
        "--before",
        "baseline/",
        "-o",
        "out/",
    ]);
    if let Commands::Align(a) = &cli.command {
        assert_eq!(a.common.paths, vec![PathBuf::from("corpus/")]);
        assert_eq!(
            a.incremental.before.as_deref(),
            Some(Path::new("baseline/"))
        );
        assert_eq!(a.common.output.as_deref(), Some(Path::new("out/")));
    } else {
        panic!("expected Align");
    }
}

#[test]
fn parse_morphotag_with_before() {
    let cli = Cli::parse_from([
        "batchalign3",
        "morphotag",
        "corpus/",
        "--before",
        "baseline/",
    ]);
    if let Commands::Morphotag(a) = &cli.command {
        assert_eq!(
            a.incremental.before.as_deref(),
            Some(Path::new("baseline/"))
        );
    } else {
        panic!("expected Morphotag");
    }
}

#[rstest]
#[case(&["batchalign3", "transcribe", "audio/", "--before", "old/"])]
#[case(&["batchalign3", "translate", "corpus/", "--before", "old/"])]
#[case(&["batchalign3", "coref", "corpus/", "--before", "old/"])]
#[case(&["batchalign3", "compare", "corpus/", "--before", "old/"])]
#[case(&["batchalign3", "utseg", "corpus/", "--before", "old/"])]
#[case(&["batchalign3", "benchmark", "audio/", "--before", "old/"])]
fn unsupported_commands_reject_before_flag(#[case] args: &[&str]) {
    assert_parse_error_contains(args, &["unexpected argument '--before'"]);
}

#[test]
fn help_shows_before_only_on_supported_commands() {
    let align_help = render_subcommand_help("align");
    let morphotag_help = render_subcommand_help("morphotag");
    let transcribe_help = render_subcommand_help("transcribe");
    let translate_help = render_subcommand_help("translate");

    assert!(align_help.contains("--before <PATH>"));
    assert!(morphotag_help.contains("--before <PATH>"));
    assert!(!transcribe_help.contains("--before <PATH>"));
    assert!(!translate_help.contains("--before <PATH>"));
}

#[test]
fn parse_utseg_with_file_list_lang_and_speakers() {
    let cli = Cli::parse_from([
        "batchalign3",
        "utseg",
        "--lang",
        "spa",
        "-n",
        "3",
        "--file-list",
        "inputs.txt",
    ]);
    if let Commands::Utseg(a) = &cli.command {
        assert_eq!(a.lang, "spa");
        assert_eq!(a.num_speakers, 3);
        assert_eq!(a.common.file_list.as_deref(), Some(Path::new("inputs.txt")));
        assert!(a.common.paths.is_empty());
    } else {
        panic!("expected Utseg");
    }
}

#[test]
fn parse_opensmile_with_bank_and_subdir() {
    let cli = Cli::parse_from([
        "batchalign3",
        "opensmile",
        "input/",
        "output/",
        "--feature-set",
        "ComParE_2016",
        "--lang",
        "spa",
        "--bank",
        "phon-data",
        "--subdir",
        "Eng-NA",
    ]);
    if let Commands::Opensmile(args) = &cli.command {
        assert_eq!(args.input_dir, PathBuf::from("input/"));
        assert_eq!(args.output_dir, PathBuf::from("output/"));
        assert_eq!(args.feature_set, "ComParE_2016");
        assert_eq!(args.lang, "spa");
        assert_eq!(args.bank.as_deref(), Some("phon-data"));
        assert_eq!(args.subdir.as_deref(), Some("Eng-NA"));
    } else {
        panic!("expected Opensmile");
    }
}

#[test]
fn parse_ba2_compat_flags() {
    let cli = Cli::parse_from([
        "batchalign3",
        "align",
        "--rev",
        "--wav2vec",
        "--no-merge-abbrev",
        "input/",
    ]);
    if let Commands::Align(a) = &cli.command {
        assert!(a.rev);
        assert!(a.wav2vec);
        assert!(a.no_merge_abbrev);
    } else {
        panic!("expected Align");
    }
}

#[test]
fn parse_serve_start_foreground() {
    let cli = Cli::parse_from([
        "batchalign3",
        "serve",
        "start",
        "--foreground",
        "--test-echo",
        "--port",
        "9000",
    ]);
    if let Commands::Serve(s) = &cli.command {
        if let ServeAction::Start(args) = &s.action {
            assert!(args.foreground);
            assert!(args.test_echo);
            assert_eq!(args.port, Some(9000));
        } else {
            panic!("expected Start");
        }
    } else {
        panic!("expected Serve");
    }
}

#[test]
fn parse_serve_start_python() {
    let cli = Cli::parse_from([
        "batchalign3",
        "serve",
        "start",
        "--python",
        "/custom/python3",
    ]);
    if let Commands::Serve(s) = &cli.command {
        if let ServeAction::Start(args) = &s.action {
            assert_eq!(args.python.as_deref(), Some("/custom/python3"));
            assert!(args.port.is_none());
            assert!(args.host.is_none());
            assert!(!args.test_echo);
            assert!(args.warmup.is_none());
        } else {
            panic!("expected Start");
        }
    } else {
        panic!("expected Serve");
    }
}

#[test]
fn parse_serve_start_warmup_preset() {
    let cli = Cli::parse_from(["batchalign3", "serve", "start", "--warmup", "minimal"]);
    if let Commands::Serve(s) = &cli.command {
        if let ServeAction::Start(args) = &s.action {
            assert_eq!(args.warmup.as_deref(), Some("minimal"));
        } else {
            panic!("expected Start");
        }
    } else {
        panic!("expected Serve");
    }
}

#[test]
fn parse_serve_start_warmup_explicit_commands() {
    let cli = Cli::parse_from([
        "batchalign3",
        "serve",
        "start",
        "--warmup",
        "align,morphotag",
    ]);
    if let Commands::Serve(s) = &cli.command {
        if let ServeAction::Start(args) = &s.action {
            assert_eq!(args.warmup.as_deref(), Some("align,morphotag"));
        } else {
            panic!("expected Start");
        }
    } else {
        panic!("expected Serve");
    }
}

#[test]
fn parse_jobs_with_id() {
    let cli = Cli::parse_from([
        "batchalign3",
        "jobs",
        "abc-123",
        "--server",
        "http://localhost:8000",
    ]);
    if let Commands::Jobs(j) = &cli.command {
        assert_eq!(j.job_id.as_deref(), Some("abc-123"));
        assert_eq!(j.server.as_deref(), Some("http://localhost:8000"));
    } else {
        panic!("expected Jobs");
    }
}

#[test]
fn parse_server_env() {
    // Simulate BATCHALIGN_SERVER env var via --server
    let cli = Cli::parse_from([
        "batchalign3",
        "--server",
        "http://myhost:8000",
        "morphotag",
        "corpus/",
    ]);
    assert_eq!(cli.global.server.as_deref(), Some("http://myhost:8000"));
}

#[test]
fn parse_open_dashboard() {
    let cli = Cli::parse_from(["batchalign3", "--open-dashboard", "align", "corpus/"]);
    assert!(cli.global.open_dashboard);
    assert!(!cli.global.no_open_dashboard);
}

#[test]
fn parse_no_open_dashboard() {
    let cli = Cli::parse_from(["batchalign3", "--no-open-dashboard", "align", "corpus/"]);
    assert!(cli.global.open_dashboard);
    assert!(cli.global.no_open_dashboard);
}

#[rstest]
#[case(&["batchalign3", "align", "--wor", "--nowor", "corpus/"], &["--wor", "--nowor"])]
#[case(&["batchalign3", "align", "--merge-abbrev", "--no-merge-abbrev", "corpus/"], &["--merge-abbrev", "--no-merge-abbrev"])]
#[case(&["batchalign3", "transcribe", "--wor", "--nowor", "audio/"], &["--wor", "--nowor"])]
#[case(&["batchalign3", "morphotag", "--retokenize", "--keeptokens", "corpus/"], &["--retokenize", "--keeptokens"])]
#[case(&["batchalign3", "morphotag", "--skipmultilang", "--multilang", "corpus/"], &["--skipmultilang", "--multilang"])]
fn conflicting_flags_are_rejected(#[case] args: &[&str], #[case] expected_fragments: &[&str]) {
    assert_parse_error_contains(args, expected_fragments);
}

#[test]
fn parse_hidden_commands() {
    let cli = Cli::parse_from(["batchalign3", "bench", "align", "in/", "out/"]);
    assert!(matches!(cli.command, Commands::Bench(_)));
}

#[test]
fn parse_models_prep() {
    let cli = Cli::parse_from([
        "batchalign3",
        "models",
        "prep",
        "my_run",
        "input/",
        "output/",
        "--min-length",
        "5",
    ]);
    if let Commands::Models(args) = &cli.command {
        if let ModelsAction::Prep(prep) = &args.action {
            assert_eq!(prep.run_name, "my_run");
            assert_eq!(prep.input_dir, PathBuf::from("input/"));
            assert_eq!(prep.output_dir, PathBuf::from("output/"));
            assert_eq!(prep.min_length, 5);
        } else {
            panic!("expected Prep");
        }
    } else {
        panic!("expected Models");
    }
}

#[test]
fn parse_models_train_passthrough() {
    let cli = Cli::parse_from([
        "batchalign3",
        "models",
        "train",
        "utterance",
        "train",
        "--epochs",
        "5",
    ]);
    if let Commands::Models(args) = &cli.command {
        if let ModelsAction::Train(train) = &args.action {
            assert_eq!(
                train.args,
                vec!["utterance", "train", "--epochs", "5"]
                    .into_iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        } else {
            panic!("expected Train");
        }
    } else {
        panic!("expected Models");
    }
}

#[test]
fn parse_avqi() {
    let cli = Cli::parse_from(["batchalign3", "avqi", "input/", "output/", "--lang", "eng"]);
    if let Commands::Avqi(args) = &cli.command {
        assert_eq!(args.input_dir, PathBuf::from("input/"));
        assert_eq!(args.output_dir, PathBuf::from("output/"));
        assert_eq!(args.lang, "eng");
    } else {
        panic!("expected Avqi");
    }
}

#[test]
fn parse_bench_options() {
    let cli = Cli::parse_from([
        "batchalign3",
        "bench",
        "transcribe_s",
        "input/",
        "output/",
        "--runs",
        "3",
        "--workers",
        "4",
        "--use-cache",
    ]);
    if let Commands::Bench(args) = &cli.command {
        assert_eq!(args.command, BenchTarget::TranscribeS);
        assert_eq!(args.in_dir, PathBuf::from("input/"));
        assert_eq!(args.out_dir, PathBuf::from("output/"));
        assert_eq!(args.runs, 3);
        assert_eq!(args.workers, Some(4));
        assert!(args.use_cache);
    } else {
        panic!("expected Bench");
    }
}

#[test]
fn parse_setup_non_interactive_rev() {
    let cli = Cli::parse_from([
        "batchalign3",
        "setup",
        "--engine",
        "rev",
        "--rev-key",
        "secret",
        "--non-interactive",
    ]);
    if let Commands::Setup(args) = &cli.command {
        assert_eq!(args.engine, Some(SetupEngine::Rev));
        assert_eq!(args.rev_key.as_deref(), Some("secret"));
        assert!(args.non_interactive);
    } else {
        panic!("expected Setup");
    }
}

#[test]
fn parse_logs_options() {
    let cli = Cli::parse_from(["batchalign3", "logs", "--last", "--raw", "--count", "5"]);
    if let Commands::Logs(args) = &cli.command {
        assert!(args.last);
        assert!(args.raw);
        assert_eq!(args.count, 5);
    } else {
        panic!("expected Logs");
    }
}

#[test]
fn parse_cache_stats() {
    let cli = Cli::parse_from(["batchalign3", "cache", "stats"]);
    if let Commands::Cache(args) = &cli.command {
        assert!(matches!(args.action, Some(CacheAction::Stats)));
    } else {
        panic!("expected Cache");
    }
}

#[test]
fn parse_cache_clear_all_yes() {
    let cli = Cli::parse_from(["batchalign3", "cache", "clear", "--all", "-y"]);
    if let Commands::Cache(args) = &cli.command {
        if let Some(CacheAction::Clear(clear)) = &args.action {
            assert!(clear.all);
            assert!(clear.yes);
        } else {
            panic!("expected Clear");
        }
    } else {
        panic!("expected Cache");
    }
}

#[test]
fn parse_cache_legacy_stats() {
    let cli = Cli::parse_from(["batchalign3", "cache", "--stats"]);
    if let Commands::Cache(args) = &cli.command {
        assert!(args.action.is_none());
        assert!(args.stats);
        assert!(!args.clear);
    } else {
        panic!("expected Cache");
    }
}

#[test]
fn parse_cache_legacy_clear_all_yes() {
    let cli = Cli::parse_from(["batchalign3", "cache", "--clear", "--all", "-y"]);
    if let Commands::Cache(args) = &cli.command {
        assert!(args.action.is_none());
        assert!(args.clear);
        assert!(args.all);
        assert!(args.yes);
    } else {
        panic!("expected Cache");
    }
}

#[test]
fn build_options_morphotag() {
    let cli = Cli::parse_from([
        "batchalign3",
        "--override-media-cache",
        "morphotag",
        "--retokenize",
        "corpus/",
    ]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    assert!(opts.common().override_media_cache);
    match opts {
        CommandOptions::Morphotag(m) => assert!(m.retokenize),
        _ => panic!("expected Morphotag"),
    }
}

// -----------------------------------------------------------------------
// build_typed_options — comprehensive coverage
// -----------------------------------------------------------------------

#[test]
fn build_options_align_defaults() {
    let cli = Cli::parse_from(["batchalign3", "align", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Align(a) => {
            assert_eq!(a.fa_engine, FaEngineName::Wave2Vec);
            assert_eq!(a.utr_engine, Some(AppUtrEngine::RevAi));
            assert!(!a.pauses);
            assert!(a.wor.should_write());
            assert!(!a.merge_abbrev.should_merge());
            assert!(!a.common.override_media_cache);
        }
        _ => panic!("expected Align"),
    }
}

#[test]
fn build_options_align_single_flags() {
    // Each flag's effect on the Align options struct.
    type AlignCase = (
        &'static [&'static str],
        Box<dyn Fn(&crate::options::AlignOptions)>,
    );
    let cases: Vec<AlignCase> = vec![
        (
            &["batchalign3", "align", "--whisper-fa", "corpus/"] as &[&str],
            Box::new(|a| assert_eq!(a.fa_engine, FaEngineName::Whisper)),
        ),
        (
            &[
                "batchalign3",
                "align",
                "--fa-engine-custom",
                "wav2vec_fa_canto",
                "corpus/",
            ],
            Box::new(|a| assert_eq!(a.fa_engine, FaEngineName::Wav2vecCanto)),
        ),
        (
            &[
                "batchalign3",
                "align",
                "--utr-engine-custom",
                "tencent_utr",
                "corpus/",
            ],
            Box::new(|a| assert_eq!(a.utr_engine, Some(AppUtrEngine::HkTencent))),
        ),
        (
            &["batchalign3", "align", "--no-utr", "corpus/"],
            Box::new(|a| assert!(a.utr_engine.is_none())),
        ),
        (
            &["batchalign3", "align", "--nowor", "corpus/"],
            Box::new(|a| assert!(!a.wor.should_write())),
        ),
        (
            &["batchalign3", "align", "--pauses", "corpus/"],
            Box::new(|a| assert!(a.pauses)),
        ),
        (
            &["batchalign3", "align", "--merge-abbrev", "corpus/"],
            Box::new(|a| assert!(a.merge_abbrev.should_merge())),
        ),
    ];
    for (args, check) in cases {
        let opts = typed_options_for(args);
        match opts {
            CommandOptions::Align(a) => check(&a),
            _ => panic!("expected Align for {args:?}"),
        }
    }
}

#[test]
fn legacy_align_aliases_match_canonical_options() {
    assert_typed_options_equivalent(
        &["batchalign3", "align", "--whisper", "corpus/"],
        &["batchalign3", "align", "--utr-engine", "whisper", "corpus/"],
        "hidden --whisper should match canonical --utr-engine whisper",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "align", "--rev", "corpus/"],
        &["batchalign3", "align", "--utr-engine", "rev", "corpus/"],
        "hidden --rev should match canonical --utr-engine rev",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "align", "--whisper-fa", "corpus/"],
        &["batchalign3", "align", "--fa-engine", "whisper", "corpus/"],
        "hidden --whisper-fa should match canonical --fa-engine whisper",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "align", "--wav2vec", "corpus/"],
        &["batchalign3", "align", "--fa-engine", "wav2vec", "corpus/"],
        "hidden --wav2vec should match canonical --fa-engine wav2vec",
    );
}

#[test]
fn legacy_align_alias_precedence_matches_documented_order() {
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "align",
            "--utr-engine-custom",
            "tencent_utr",
            "--whisper",
            "corpus/",
        ],
        &[
            "batchalign3",
            "align",
            "--utr-engine-custom",
            "tencent_utr",
            "corpus/",
        ],
        "custom UTR engine should override hidden BA2 align aliases",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "align",
            "--fa-engine-custom",
            "wav2vec_fa_canto",
            "--whisper-fa",
            "corpus/",
        ],
        &[
            "batchalign3",
            "align",
            "--fa-engine-custom",
            "wav2vec_fa_canto",
            "corpus/",
        ],
        "custom FA engine should override hidden BA2 align aliases",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "align",
            "--utr-engine",
            "rev",
            "--whisper",
            "corpus/",
        ],
        &["batchalign3", "align", "--utr-engine", "whisper", "corpus/"],
        "hidden --whisper should override canonical --utr-engine rev when --rev is absent",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "align", "--whisper", "--rev", "corpus/"],
        &["batchalign3", "align", "--utr-engine", "rev", "corpus/"],
        "hidden --rev should cancel the hidden --whisper override path",
    );
}

#[test]
fn build_options_transcribe_defaults() {
    let cli = Cli::parse_from(["batchalign3", "transcribe", "audio/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Transcribe(t) => {
            assert_eq!(t.asr_engine, AsrEngineName::RevAi);
            assert!(!t.diarize);
            assert!(!t.wor.should_write());
            assert!(!t.merge_abbrev.should_merge());
            assert_eq!(t.batch_size, 8); // default, no longer CLI-configurable
        }
        _ => panic!("expected Transcribe"),
    }
}

#[rstest]
#[case(&["batchalign3", "transcribe", "--whisperx", "audio/"], AsrEngineName::WhisperX)]
#[case(&["batchalign3", "transcribe", "--whisper-oai", "audio/"], AsrEngineName::WhisperOai)]
#[case(&["batchalign3", "transcribe", "--whisper", "audio/"], AsrEngineName::Whisper)]
#[case(&["batchalign3", "transcribe", "--asr-engine-custom", "tencent", "audio/"], AsrEngineName::HkTencent)]
fn build_options_transcribe_asr_engine(#[case] args: &[&str], #[case] expected: AsrEngineName) {
    let opts = typed_options_for(args);
    match opts {
        CommandOptions::Transcribe(t) => assert_eq!(t.asr_engine, expected, "{args:?}"),
        _ => panic!("expected Transcribe for {args:?}"),
    }
}

#[test]
fn build_options_transcribe_diarize() {
    let cli = Cli::parse_from(["batchalign3", "transcribe", "--diarize", "audio/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::TranscribeS(t) => assert!(t.diarize),
        _ => panic!("expected TranscribeS"),
    }
}

#[test]
fn build_options_transcribe_diarize_matches_batchalign2_baseline_defaults() {
    let cli = Cli::parse_from(["batchalign3", "transcribe", "--diarize", "audio/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    let profile = CommonOpts::command_profile(&cli.command);

    match opts {
        CommandOptions::TranscribeS(t) => {
            assert_eq!(t.asr_engine, AsrEngineName::RevAi);
            assert!(t.diarize);
            assert!(!t.wor.should_write());
            assert!(!t.merge_abbrev.should_merge());
            assert_eq!(t.batch_size, 8); // default, no longer CLI-configurable
        }
        other => panic!("expected TranscribeS, got {other:?}"),
    }

    assert_eq!(profile.command, ReleasedCommand::TranscribeS);
    assert_eq!(profile.lang, "eng");
    assert_eq!(profile.num_speakers, 2);
    assert_eq!(profile.extensions, &["mp3", "mp4", "wav"]);
}

#[test]
fn legacy_transcribe_aliases_match_canonical_options() {
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--whisper", "audio/"],
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine",
            "whisper",
            "audio/",
        ],
        "hidden --whisper should match canonical --asr-engine whisper",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--whisperx", "audio/"],
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine",
            "whisperx",
            "audio/",
        ],
        "hidden --whisperx should match canonical --asr-engine whisperx",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--whisper-oai", "audio/"],
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine",
            "whisper-oai",
            "audio/",
        ],
        "hidden --whisper-oai should match canonical --asr-engine whisper-oai",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--rev", "audio/"],
        &["batchalign3", "transcribe", "--asr-engine", "rev", "audio/"],
        "hidden --rev should match canonical --asr-engine rev",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--diarize", "audio/"],
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "enabled",
            "audio/",
        ],
        "hidden --diarize should match canonical --diarization enabled",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "transcribe", "--nodiarize", "audio/"],
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "disabled",
            "audio/",
        ],
        "hidden --nodiarize should match canonical --diarization disabled",
    );
}

#[test]
fn legacy_transcribe_alias_precedence_matches_documented_order() {
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine-custom",
            "tencent",
            "--whisper",
            "audio/",
        ],
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine-custom",
            "tencent",
            "audio/",
        ],
        "custom ASR engine should override hidden BA2 transcribe aliases",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine",
            "rev",
            "--whisper",
            "audio/",
        ],
        &[
            "batchalign3",
            "transcribe",
            "--asr-engine",
            "whisper",
            "audio/",
        ],
        "hidden --whisper should override canonical --asr-engine rev",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "disabled",
            "--diarize",
            "audio/",
        ],
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "enabled",
            "audio/",
        ],
        "hidden --diarize should override canonical disabled diarization",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "enabled",
            "--nodiarize",
            "audio/",
        ],
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "disabled",
            "audio/",
        ],
        "hidden --nodiarize should override canonical enabled diarization",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "transcribe",
            "--diarize",
            "--nodiarize",
            "audio/",
        ],
        &[
            "batchalign3",
            "transcribe",
            "--diarization",
            "enabled",
            "audio/",
        ],
        "hidden --diarize should win when both hidden diarization flags are present",
    );
}

#[test]
fn build_options_translate() {
    let cli = Cli::parse_from(["batchalign3", "translate", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Translate(t) => {
            assert!(!t.merge_abbrev.should_merge());
        }
        _ => panic!("expected Translate"),
    }
}

#[test]
fn build_options_merge_abbrev_matrix_for_processing_commands() {
    let cases = [
        (
            vec!["batchalign3", "align", "corpus/"],
            false,
            "align defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "align", "--merge-abbrev", "corpus/"],
            true,
            "align enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "align", "--no-merge-abbrev", "corpus/"],
            false,
            "align honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "transcribe", "audio/"],
            false,
            "transcribe defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "transcribe", "--merge-abbrev", "audio/"],
            true,
            "transcribe enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "transcribe", "--no-merge-abbrev", "audio/"],
            false,
            "transcribe honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "translate", "corpus/"],
            false,
            "translate defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "translate", "--merge-abbrev", "corpus/"],
            true,
            "translate enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "translate", "--no-merge-abbrev", "corpus/"],
            false,
            "translate honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "morphotag", "corpus/"],
            false,
            "morphotag defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "morphotag", "--merge-abbrev", "corpus/"],
            true,
            "morphotag enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "morphotag", "--no-merge-abbrev", "corpus/"],
            false,
            "morphotag honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "coref", "corpus/"],
            false,
            "coref defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "coref", "--merge-abbrev", "corpus/"],
            true,
            "coref enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "coref", "--no-merge-abbrev", "corpus/"],
            false,
            "coref honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "compare", "corpus/"],
            false,
            "compare defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "compare", "--merge-abbrev", "corpus/"],
            true,
            "compare enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "compare", "--no-merge-abbrev", "corpus/"],
            false,
            "compare honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "utseg", "corpus/"],
            false,
            "utseg defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "utseg", "--merge-abbrev", "corpus/"],
            true,
            "utseg enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "utseg", "--no-merge-abbrev", "corpus/"],
            false,
            "utseg honors no-merge-abbrev",
        ),
        (
            vec!["batchalign3", "benchmark", "audio/"],
            false,
            "benchmark defaults to no merge-abbrev",
        ),
        (
            vec!["batchalign3", "benchmark", "--merge-abbrev", "audio/"],
            true,
            "benchmark enables merge-abbrev",
        ),
        (
            vec!["batchalign3", "benchmark", "--no-merge-abbrev", "audio/"],
            false,
            "benchmark honors no-merge-abbrev",
        ),
    ];

    for (args, expected, note) in cases {
        let opts = typed_options_for(&args);
        assert_eq!(merge_abbrev_value(&opts), Some(expected), "{note}");
    }
}

#[test]
fn build_options_wor_matrix_for_processing_commands() {
    let cases = [
        (
            vec!["batchalign3", "align", "corpus/"],
            true,
            "align defaults to writing %wor",
        ),
        (
            vec!["batchalign3", "align", "--nowor", "corpus/"],
            false,
            "align honors --nowor",
        ),
        (
            vec!["batchalign3", "transcribe", "audio/"],
            false,
            "transcribe defaults to omitting %wor",
        ),
        (
            vec!["batchalign3", "transcribe", "--wor", "audio/"],
            true,
            "transcribe honors --wor",
        ),
        (
            vec!["batchalign3", "transcribe", "--nowor", "audio/"],
            false,
            "transcribe honors --nowor",
        ),
        (
            vec!["batchalign3", "benchmark", "audio/"],
            false,
            "benchmark defaults to omitting %wor",
        ),
        (
            vec!["batchalign3", "benchmark", "--wor", "audio/"],
            true,
            "benchmark honors --wor",
        ),
        (
            vec!["batchalign3", "benchmark", "--nowor", "audio/"],
            false,
            "benchmark honors --nowor",
        ),
    ];

    for (args, expected, note) in cases {
        let opts = typed_options_for(&args);
        assert_eq!(wor_value(&opts), Some(expected), "{note}");
    }
}

#[test]
fn build_options_coref_merge_abbrev() {
    let cli = Cli::parse_from(["batchalign3", "coref", "--merge-abbrev", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Coref(c) => assert!(c.merge_abbrev.should_merge()),
        _ => panic!("expected Coref"),
    }
}

#[test]
fn build_options_utseg() {
    let cli = Cli::parse_from(["batchalign3", "utseg", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Utseg(u) => {
            assert!(!u.merge_abbrev.should_merge());
        }
        _ => panic!("expected Utseg"),
    }
}

#[test]
fn build_options_benchmark_defaults() {
    let cli = Cli::parse_from(["batchalign3", "benchmark", "audio/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Benchmark(b) => {
            assert_eq!(b.asr_engine, AsrEngineName::RevAi);
            assert!(!b.wor.should_write());
            assert!(!b.merge_abbrev.should_merge());
        }
        _ => panic!("expected Benchmark"),
    }
}

#[rstest]
#[case(&["batchalign3", "benchmark", "--whisper", "audio/"], AsrEngineName::Whisper)]
#[case(&["batchalign3", "benchmark", "--whisper-oai", "audio/"], AsrEngineName::WhisperOai)]
#[case(&["batchalign3", "benchmark", "--asr-engine-custom", "funaudio", "audio/"], AsrEngineName::HkFunaudio)]
fn build_options_benchmark_asr_engine(#[case] args: &[&str], #[case] expected: AsrEngineName) {
    let opts = typed_options_for(args);
    match opts {
        CommandOptions::Benchmark(b) => assert_eq!(b.asr_engine, expected, "{args:?}"),
        _ => panic!("expected Benchmark for {args:?}"),
    }
}

#[test]
fn legacy_benchmark_aliases_match_canonical_options() {
    assert_typed_options_equivalent(
        &["batchalign3", "benchmark", "--whisper", "audio/"],
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine",
            "whisper",
            "audio/",
        ],
        "hidden benchmark --whisper should match canonical --asr-engine whisper",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "benchmark", "--whisper-oai", "audio/"],
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine",
            "whisper-oai",
            "audio/",
        ],
        "hidden benchmark --whisper-oai should match canonical --asr-engine whisper-oai",
    );
    assert_typed_options_equivalent(
        &["batchalign3", "benchmark", "--rev", "audio/"],
        &["batchalign3", "benchmark", "--asr-engine", "rev", "audio/"],
        "hidden benchmark --rev should match canonical --asr-engine rev",
    );
}

#[test]
fn legacy_benchmark_alias_precedence_matches_documented_order() {
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine-custom",
            "funaudio",
            "--whisper",
            "audio/",
        ],
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine-custom",
            "funaudio",
            "audio/",
        ],
        "custom benchmark ASR engine should override hidden BA2 aliases",
    );
    assert_typed_options_equivalent(
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine",
            "rev",
            "--whisper",
            "audio/",
        ],
        &[
            "batchalign3",
            "benchmark",
            "--asr-engine",
            "whisper",
            "audio/",
        ],
        "hidden benchmark --whisper should override canonical --asr-engine rev",
    );
}

#[test]
fn build_options_opensmile_defaults() {
    let cli = Cli::parse_from(["batchalign3", "opensmile", "in/", "out/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Opensmile(o) => assert_eq!(o.feature_set, "eGeMAPSv02"),
        _ => panic!("expected Opensmile"),
    }
}

#[test]
fn build_options_opensmile_compare() {
    let cli = Cli::parse_from([
        "batchalign3",
        "opensmile",
        "--feature-set",
        "ComParE_2016",
        "in/",
        "out/",
    ]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    match opts {
        CommandOptions::Opensmile(o) => assert_eq!(o.feature_set, "ComParE_2016"),
        _ => panic!("expected Opensmile"),
    }
}

#[test]
fn build_options_avqi_defaults() {
    let opts = typed_options_for(&["batchalign3", "avqi", "input/", "output/"]);
    match opts {
        CommandOptions::Avqi(a) => {
            assert!(!a.common.override_media_cache);
        }
        _ => panic!("expected Avqi"),
    }
}

#[test]
fn build_options_override_media_cache_global() {
    let cli = Cli::parse_from(["batchalign3", "--override-media-cache", "align", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    assert!(opts.common().override_media_cache);
}

// -----------------------------------------------------------------------
// command_profile (parametrized)
// -----------------------------------------------------------------------

#[rstest]
#[case(&["batchalign3", "align", "corpus/"], ReleasedCommand::Align, "eng", 1, &["cha"])]
#[case(&["batchalign3", "transcribe", "audio/"], ReleasedCommand::Transcribe, "eng", 2, &["mp3", "mp4", "wav"])]
#[case(&["batchalign3", "transcribe", "--diarize", "audio/"], ReleasedCommand::TranscribeS, "eng", 2, &["mp3", "mp4", "wav"])]
// per-file commands: translate/morphotag/coref. The `lang` field on
// CommandProfile carries the wire string `"per-file"`, which parses to
// `LanguageSpec::PerFile` at submission. No English placeholder ever
// appears for these commands in job records, dashboards, or worker
// pre-warming.
#[case(&["batchalign3", "translate", "corpus/"], ReleasedCommand::Translate, "per-file", 1, &["cha"])]
#[case(&["batchalign3", "morphotag", "corpus/"], ReleasedCommand::Morphotag, "per-file", 1, &["cha"])]
#[case(&["batchalign3", "coref", "corpus/"], ReleasedCommand::Coref, "per-file", 1, &["cha"])]
#[case(&["batchalign3", "compare", "corpus/"], ReleasedCommand::Compare, "eng", 2, &["cha"])]
#[case(&["batchalign3", "compare", "--lang", "spa", "-n", "3", "corpus/"], ReleasedCommand::Compare, "spa", 3, &["cha"])]
#[case(&["batchalign3", "utseg", "--lang", "spa", "-n", "3", "corpus/"], ReleasedCommand::Utseg, "spa", 3, &["cha"])]
#[case(&["batchalign3", "benchmark", "audio/"], ReleasedCommand::Benchmark, "eng", 2, &["mp3", "mp4", "wav"])]
#[case(&["batchalign3", "opensmile", "in/", "out/"], ReleasedCommand::Opensmile, "eng", 1, &["mp3", "mp4", "wav"])]
#[case(&["batchalign3", "avqi", "in/", "out/", "--lang", "yue"], ReleasedCommand::Avqi, "yue", 1, &["mp3", "mp4", "wav"])]
fn command_profile_matches_expected(
    #[case] args: &[&str],
    #[case] expected_cmd: ReleasedCommand,
    #[case] expected_lang: &str,
    #[case] expected_speakers: u32,
    #[case] expected_exts: &[&str],
) {
    let cli = Cli::parse_from(args);
    let profile = CommonOpts::command_profile(&cli.command);
    assert_eq!(
        profile.command, expected_cmd,
        "command mismatch for {args:?}"
    );
    assert_eq!(profile.lang, expected_lang, "lang mismatch for {args:?}");
    assert_eq!(
        profile.num_speakers, expected_speakers,
        "num_speakers mismatch for {args:?}"
    );
    assert_eq!(
        profile.extensions, expected_exts,
        "extensions mismatch for {args:?}"
    );
}

// -----------------------------------------------------------------------
// common_opts (parametrized)
// -----------------------------------------------------------------------

/// BA2 parity: `coref` does NOT accept `--lang`. The command is English-only
/// and per-file routing comes from the file's `@Languages:` header. This
/// regression test catches any reintroduction of `--lang` on coref. See the
/// 2026-05-03 incident (morphotag had the same shape; sentinel `--lang`
/// silently rewrote non-English files).
#[test]
fn coref_rejects_lang_flag_for_ba2_parity() {
    let result = Cli::try_parse_from(["batchalign3", "coref", "corpus/", "--lang", "eng"]);
    assert!(
        result.is_err(),
        "coref must NOT accept --lang (BA2 parity); CLI parse should fail"
    );
}

/// BA2 parity: `translate` does NOT accept `--lang`. Source language for each
/// file comes from that file's `@Languages:` header (BA2's
/// `pipelines/translate/seamless.py:40` reads `doc.langs[0]` per file); the
/// translation TARGET is hardcoded to English (BA2 `seamless.py:41`,
/// `tgt_lang="eng"`). Re-introducing `--lang` here would recreate the
/// 2026-05-03 morphotag failure mode where a job-level sentinel silently
/// overrode per-file routing.
#[test]
fn translate_rejects_lang_flag_for_ba2_parity() {
    let result = Cli::try_parse_from(["batchalign3", "translate", "corpus/", "--lang", "spa"]);
    assert!(
        result.is_err(),
        "translate must NOT accept --lang (BA2 parity); CLI parse should fail"
    );
}

#[rstest]
#[case(&["batchalign3", "align", "x/"])]
#[case(&["batchalign3", "transcribe", "x/"])]
#[case(&["batchalign3", "translate", "x/"])]
#[case(&["batchalign3", "morphotag", "x/"])]
#[case(&["batchalign3", "coref", "x/"])]
#[case(&["batchalign3", "compare", "x/"])]
#[case(&["batchalign3", "utseg", "x/"])]
#[case(&["batchalign3", "benchmark", "x/"])]
fn common_opts_processing_commands_return_some(#[case] args: &[&str]) {
    let cli = Cli::parse_from(args);
    assert!(
        common_opts(&cli.command).is_some(),
        "common_opts should be Some for {args:?}"
    );
}

#[rstest]
#[case(&["batchalign3", "version"])]
#[case(&["batchalign3", "opensmile", "in/", "out/"])]
#[case(&["batchalign3", "avqi", "in/", "out/"])]
fn common_opts_non_processing_commands_return_none(#[case] args: &[&str]) {
    let cli = Cli::parse_from(args);
    assert!(
        common_opts(&cli.command).is_none(),
        "common_opts should be None for {args:?}"
    );
}

// -----------------------------------------------------------------------
// extract_bank / extract_subdir / extract_lexicon (parametrized)
// -----------------------------------------------------------------------

#[rstest]
#[case(&["batchalign3", "benchmark", "--bank", "childes-data", "audio/"], Some("childes-data"))]
#[case(&["batchalign3", "opensmile", "--bank", "mybank", "in/", "out/"], Some("mybank"))]
#[case(&["batchalign3", "morphotag", "corpus/"], None)]
fn extract_bank_matches(#[case] args: &[&str], #[case] expected: Option<&str>) {
    let cli = Cli::parse_from(args);
    assert_eq!(extract_bank(&cli.command), expected, "{args:?}");
}

#[rstest]
#[case(&["batchalign3", "benchmark", "--subdir", "Eng-NA", "audio/"], Some("Eng-NA"))]
#[case(&["batchalign3", "opensmile", "in/", "out/", "--subdir", "Eng-NA"], Some("Eng-NA"))]
#[case(&["batchalign3", "align", "corpus/"], None)]
fn extract_subdir_matches(#[case] args: &[&str], #[case] expected: Option<&str>) {
    let cli = Cli::parse_from(args);
    assert_eq!(extract_subdir(&cli.command), expected, "{args:?}");
}

#[rstest]
#[case(&["batchalign3", "morphotag", "--lexicon", "lex.csv", "corpus/"], Some("lex.csv"))]
#[case(&["batchalign3", "align", "corpus/"], None)]
fn extract_lexicon_matches(#[case] args: &[&str], #[case] expected: Option<&str>) {
    let cli = Cli::parse_from(args);
    assert_eq!(extract_lexicon(&cli.command), expected, "{args:?}");
}

// -----------------------------------------------------------------------
// --engine-overrides global flag
// -----------------------------------------------------------------------

#[test]
fn parse_engine_overrides_global_flag() {
    let cli = Cli::parse_from([
        "batchalign3",
        "--engine-overrides",
        r#"{"asr": "tencent", "fa": "cantonese_fa"}"#,
        "morphotag",
        "corpus/",
    ]);
    assert_eq!(
        cli.global.engine_overrides.as_deref(),
        Some(r#"{"asr": "tencent", "fa": "cantonese_fa"}"#)
    );
}

#[test]
fn parse_engine_overrides_absent() {
    let cli = Cli::parse_from(["batchalign3", "morphotag", "corpus/"]);
    assert!(cli.global.engine_overrides.is_none());
}

#[test]
fn build_options_engine_overrides_populates_common() {
    let cli = Cli::parse_from([
        "batchalign3",
        "--engine-overrides",
        r#"{"asr": "tencent"}"#,
        "morphotag",
        "corpus/",
    ]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    assert_eq!(
        opts.common().engine_overrides.asr,
        Some(AsrEngineName::HkTencent)
    );
}

#[test]
fn build_options_engine_overrides_empty_by_default() {
    let cli = Cli::parse_from(["batchalign3", "align", "corpus/"]);
    let opts = build_typed_options(&cli.command, &cli.global).unwrap();
    assert!(opts.common().engine_overrides.is_empty());
}

#[rstest]
#[case("not valid json")]
#[case(r#"{"asr":{"name":"whisper"}}"#)]
fn build_options_engine_overrides_invalid_values_are_rejected(#[case] overrides: &str) {
    assert_parse_error_contains(
        &[
            "batchalign3",
            "--engine-overrides",
            overrides,
            "morphotag",
            "corpus/",
        ],
        &["invalid --engine-overrides JSON"],
    );
}

#[test]
fn build_options_engine_overrides_multiple_commands() {
    // Verify engine overrides flow through to different command variants
    for (args, variant_name) in [
        (
            vec![
                "batchalign3",
                "--engine-overrides",
                r#"{"fa": "whisper_fa"}"#,
                "align",
                "corpus/",
            ],
            "align",
        ),
        (
            vec![
                "batchalign3",
                "--engine-overrides",
                r#"{"asr": "tencent"}"#,
                "transcribe",
                "audio/",
            ],
            "transcribe",
        ),
        (
            vec![
                "batchalign3",
                "--engine-overrides",
                r#"{"asr": "whisper"}"#,
                "translate",
                "corpus/",
            ],
            "translate",
        ),
    ] {
        let cli = Cli::parse_from(args);
        let opts = build_typed_options(&cli.command, &cli.global).unwrap();
        assert!(
            !opts.common().engine_overrides.is_empty(),
            "engine_overrides should be non-empty for {variant_name}"
        );
    }
}
