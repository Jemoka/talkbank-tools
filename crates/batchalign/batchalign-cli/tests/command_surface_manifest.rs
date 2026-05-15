//! Release-facing manifest for the published `batchalign3` CLI surface.

use std::collections::BTreeSet;

mod cli_common;

use cli_common::cli_cmd as cmd;

/// Functional family for visible top-level commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceFamily {
    /// Transcript/media processing commands.
    Processing,
    /// Local or remote server management commands.
    Server,
    /// Utility, setup, schema, and benchmark helper commands.
    Utility,
}

/// Coverage expectations for one surface family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoverageExpectation {
    /// Command must remain visible in `--help`.
    HelpContract,
    /// Command family needs systematic option-combination coverage.
    OptionMatrix,
    /// Command family includes compatibility aliases that must stay accepted.
    LegacyCompatibility,
}

/// Reviewed visible command family entry.
#[derive(Clone, Copy, Debug)]
struct SurfaceGroup {
    family: SurfaceFamily,
    commands: &'static [&'static str],
    coverage: &'static [CoverageExpectation],
    note: &'static str,
}

/// One hidden compatibility flag that should parse successfully while staying
/// absent from normal help output.
#[derive(Clone, Copy, Debug)]
struct HiddenCompatCase {
    args: &'static [&'static str],
    hidden_flag: &'static str,
    help_scope: &'static [&'static str],
    note: &'static str,
}

const PROCESSING_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::LegacyCompatibility,
];

const SERVER_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
];

const UTILITY_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
];

const SURFACE_GROUPS: &[SurfaceGroup] = &[
    SurfaceGroup {
        family: SurfaceFamily::Processing,
        commands: &[
            "align",
            "transcribe",
            "translate",
            "morphotag",
            "coref",
            "utseg",
            "benchmark",
            "opensmile",
            "compare",
            "avqi",
        ],
        coverage: PROCESSING_COVERAGE,
        note: "primary transcript and audio processing commands",
    },
    SurfaceGroup {
        family: SurfaceFamily::Server,
        commands: &["serve", "jobs", "logs", "cache"],
        coverage: SERVER_COVERAGE,
        note: "local daemon and remote-server management commands",
    },
    SurfaceGroup {
        family: SurfaceFamily::Utility,
        commands: &["setup", "openapi", "version", "models", "bench"],
        coverage: UTILITY_COVERAGE,
        note: "configuration, schema, version, training, and benchmarking utilities",
    },
];

const HIDDEN_COMPAT_CASES: &[HiddenCompatCase] = &[
    HiddenCompatCase {
        args: &["align", "--whisper", "--help"],
        hidden_flag: "--whisper",
        help_scope: &["align", "--help"],
        note: "BA2 align alias for whisper UTR",
    },
    HiddenCompatCase {
        args: &["align", "--rev", "--help"],
        hidden_flag: "--rev",
        help_scope: &["align", "--help"],
        note: "BA2 align alias for Rev UTR",
    },
    HiddenCompatCase {
        args: &["align", "--whisper-fa", "--help"],
        hidden_flag: "--whisper-fa",
        help_scope: &["align", "--help"],
        note: "BA2 align alias for whisper FA",
    },
    HiddenCompatCase {
        args: &["align", "--wav2vec", "--help"],
        hidden_flag: "--wav2vec",
        help_scope: &["align", "--help"],
        note: "BA2 align alias for wav2vec FA",
    },
    HiddenCompatCase {
        args: &["transcribe", "--whisper", "--help"],
        hidden_flag: "--whisper",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for whisper ASR",
    },
    HiddenCompatCase {
        args: &["transcribe", "--whisperx", "--help"],
        hidden_flag: "--whisperx",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for whisperx ASR",
    },
    HiddenCompatCase {
        args: &["transcribe", "--whisper-oai", "--help"],
        hidden_flag: "--whisper-oai",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for whisper-oai ASR",
    },
    HiddenCompatCase {
        args: &["transcribe", "--rev", "--help"],
        hidden_flag: "--rev",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for Rev ASR",
    },
    HiddenCompatCase {
        args: &["transcribe", "--diarize", "--help"],
        hidden_flag: "--diarize",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for enabled diarization",
    },
    HiddenCompatCase {
        args: &["transcribe", "--nodiarize", "--help"],
        hidden_flag: "--nodiarize",
        help_scope: &["transcribe", "--help"],
        note: "BA2 transcribe alias for disabled diarization",
    },
    HiddenCompatCase {
        args: &["benchmark", "--whisper", "--help"],
        hidden_flag: "--whisper",
        help_scope: &["benchmark", "--help"],
        note: "BA2 benchmark alias for whisper ASR",
    },
    HiddenCompatCase {
        args: &["benchmark", "--whisper-oai", "--help"],
        hidden_flag: "--whisper-oai",
        help_scope: &["benchmark", "--help"],
        note: "BA2 benchmark alias for whisper-oai ASR",
    },
    HiddenCompatCase {
        args: &["benchmark", "--rev", "--help"],
        hidden_flag: "--rev",
        help_scope: &["benchmark", "--help"],
        note: "BA2 benchmark alias for Rev ASR",
    },
];

fn help_output(args: &[&str]) -> String {
    let output = cmd()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8_lossy(&output).into_owned()
}

fn listed_commands(help: &str) -> BTreeSet<String> {
    let mut commands = BTreeSet::new();
    let mut in_commands = false;

    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed == "Commands:" {
            in_commands = true;
            continue;
        }

        if !in_commands {
            continue;
        }

        if trimmed == "Options:" {
            break;
        }

        if line.starts_with("  ")
            && !trimmed.is_empty()
            && let Some(command) = trimmed.split_whitespace().next()
        {
            commands.insert(command.to_string());
        }
    }

    commands
}

fn manifest_commands() -> BTreeSet<&'static str> {
    SURFACE_GROUPS
        .iter()
        .flat_map(|group| group.commands.iter().copied())
        .collect()
}

#[test]
fn command_surface_manifest_has_unique_visible_commands() {
    let mut seen = BTreeSet::new();
    for command in manifest_commands() {
        assert!(
            seen.insert(command),
            "duplicate visible command `{command}` in batchalign surface manifest"
        );
    }
}

#[test]
fn top_level_help_lists_all_manifested_commands() {
    let commands = listed_commands(&help_output(&["--help"]));
    for command in manifest_commands() {
        assert!(
            commands.contains(command),
            "top-level help is missing manifested command `{command}`"
        );
    }
}

#[test]
fn every_surface_group_declares_coverage_and_rationale() {
    for group in SURFACE_GROUPS {
        assert!(
            !group.commands.is_empty(),
            "{:?} surface group has no commands",
            group.family
        );
        assert!(
            !group.coverage.is_empty(),
            "{:?} surface group has no coverage expectations",
            group.family
        );
        assert!(
            !group.note.is_empty(),
            "{:?} surface group has no rationale",
            group.family
        );
    }
}

/// Check for `flag` in `help` using word-boundary matching so that a short
/// flag (e.g. `--rev`) does not falsely match a longer flag that starts with
/// the same prefix (e.g. `--review-level`).
fn flag_in_help(help: &str, flag: &str) -> bool {
    let mut s = help;
    while let Some(pos) = s.find(flag) {
        let rest = &s[pos + flag.len()..];
        // A flag match is only real if the next character is a word boundary:
        // space, tab, newline, `=` (for `--flag=value`), or end of string.
        let boundary = rest.is_empty()
            || rest.starts_with(' ')
            || rest.starts_with('\t')
            || rest.starts_with('\n')
            || rest.starts_with('=');
        if boundary {
            return true;
        }
        s = &s[pos + 1..];
    }
    false
}

#[test]
fn hidden_batchalign2_compat_flags_are_accepted_but_not_listed_in_help() {
    for case in HIDDEN_COMPAT_CASES {
        cmd().args(case.args).assert().success();

        let help = help_output(case.help_scope);
        assert!(
            !flag_in_help(&help, case.hidden_flag),
            "hidden compatibility flag `{}` leaked into help for {}",
            case.hidden_flag,
            case.note
        );
    }
}
