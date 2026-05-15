//! Shared harness helpers for CLAN golden integration tests.

use std::path::Path;

use clan_core::framework::{FilterConfig, SpeakerFilter, UtteranceRange, WordFilter};
use talkbank_model::SpeakerCode;

pub use crate::common::{clan_bin_dir, corpus_dir, corpus_file};
use crate::common::{require_clan_command, run_clan_stdout_from_stdin};
pub use clan_core::framework::OutputFormat;

#[derive(Clone, Copy)]
enum GoldenRunner {
    Command(&'static str),
    Check,
}

#[derive(Clone, Copy)]
pub(crate) enum FilterSpec {
    None,
    SpeakerInclude(&'static [&'static str]),
    WordInclude(&'static [&'static str]),
    UtteranceRange { start: usize, end: usize },
}

impl FilterSpec {
    pub(crate) fn speakers(speakers: &'static [&'static str]) -> Self {
        Self::SpeakerInclude(speakers)
    }

    pub(crate) fn words(words: &'static [&'static str]) -> Self {
        Self::WordInclude(words)
    }

    pub(crate) fn range(start: usize, end: usize) -> Self {
        Self::UtteranceRange { start, end }
    }
}

pub(crate) struct ParityCase {
    runner: GoldenRunner,
    file: &'static str,
    clan_args: &'static [&'static str],
    rust_args: &'static [&'static str],
    rust_filter: FilterSpec,
    format: OutputFormat,
    clan_snapshot: &'static str,
    rust_snapshot: &'static str,
    clan_compat_message: Option<&'static str>,
}

impl ParityCase {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn command(
        command: &'static str,
        file: &'static str,
        clan_args: &'static [&'static str],
        rust_args: &'static [&'static str],
        rust_filter: FilterSpec,
        format: OutputFormat,
        clan_snapshot: &'static str,
        rust_snapshot: &'static str,
    ) -> Self {
        Self {
            runner: GoldenRunner::Command(command),
            file,
            clan_args,
            rust_args,
            rust_filter,
            format,
            clan_snapshot,
            rust_snapshot,
            clan_compat_message: None,
        }
    }

    pub(crate) fn check(
        file: &'static str,
        clan_snapshot: &'static str,
        rust_snapshot: &'static str,
    ) -> Self {
        Self {
            runner: GoldenRunner::Check,
            file,
            clan_args: &[],
            rust_args: &[],
            rust_filter: FilterSpec::None,
            format: OutputFormat::Text,
            clan_snapshot,
            rust_snapshot,
            clan_compat_message: None,
        }
    }

    pub(crate) fn with_clan_compat(mut self, message: &'static str) -> Self {
        self.clan_compat_message = Some(message);
        self
    }
}

pub(crate) struct RustSnapshotCase {
    command: &'static str,
    file: &'static str,
    rust_args: &'static [&'static str],
    format: OutputFormat,
    rust_snapshot: &'static str,
}

impl RustSnapshotCase {
    pub(crate) fn new(
        command: &'static str,
        file: &'static str,
        rust_args: &'static [&'static str],
        format: OutputFormat,
        rust_snapshot: &'static str,
    ) -> Self {
        Self {
            command,
            file,
            rust_args,
            format,
            rust_snapshot,
        }
    }
}

pub(crate) struct ParityOutputs {
    pub(crate) clan_snapshot: &'static str,
    pub(crate) clan_output: String,
    pub(crate) rust_snapshot: &'static str,
    pub(crate) rust_output: String,
}

pub(crate) struct RustSnapshotOutput {
    pub(crate) rust_snapshot: &'static str,
    pub(crate) rust_output: String,
}

pub(crate) fn run_parity_case(case: &ParityCase) -> Option<ParityOutputs> {
    let required_command = match case.runner {
        GoldenRunner::Command(command) => command,
        GoldenRunner::Check => "check",
    };
    if !require_clan_command(required_command, "skipping golden test") {
        return None;
    }

    let file = corpus_file(case.file);
    let clan_output = match case.runner {
        GoldenRunner::Command(command) => run_clan(command, &file, case.clan_args)
            .unwrap_or_else(|| panic!("CLAN {command} failed")),
        GoldenRunner::Check => run_clan_check(&file).expect("CLAN check failed"),
    };
    let rust_output = match case.runner {
        GoldenRunner::Command(command) => run_rust_filtered(
            command,
            &file,
            case.rust_args,
            case.format,
            build_filter(case.rust_filter),
        ),
        GoldenRunner::Check => run_rust_check(&file),
    };

    if let Some(message) = case.clan_compat_message {
        let GoldenRunner::Command(command) = case.runner else {
            panic!("CHECK cases do not support CLAN compatibility assertions");
        };
        let clan_compat = run_rust_filtered(
            command,
            &file,
            case.rust_args,
            OutputFormat::Clan,
            build_filter(case.rust_filter),
        );
        assert_eq!(clan_compat.trim(), clan_output.trim(), "{message}");
    }

    Some(ParityOutputs {
        clan_snapshot: case.clan_snapshot,
        clan_output,
        rust_snapshot: case.rust_snapshot,
        rust_output,
    })
}

pub(crate) fn run_rust_snapshot_case(case: &RustSnapshotCase) -> RustSnapshotOutput {
    let file = corpus_file(case.file);
    let rust_output = run_rust(case.command, &file, case.rust_args, case.format);
    RustSnapshotOutput {
        rust_snapshot: case.rust_snapshot,
        rust_output,
    }
}

fn build_filter(spec: FilterSpec) -> Option<FilterConfig> {
    match spec {
        FilterSpec::None => None,
        FilterSpec::SpeakerInclude(speakers) => Some(FilterConfig {
            speakers: SpeakerFilter {
                include: speakers
                    .iter()
                    .map(|speaker| SpeakerCode::from(*speaker))
                    .collect(),
                ..SpeakerFilter::default()
            },
            ..FilterConfig::default()
        }),
        FilterSpec::WordInclude(words) => Some(FilterConfig {
            words: WordFilter {
                include: words
                    .iter()
                    .map(|word| clan_core::framework::WordPattern::from(*word))
                    .collect(),
                ..WordFilter::default()
            },
            ..FilterConfig::default()
        }),
        FilterSpec::UtteranceRange { start, end } => Some(FilterConfig {
            utterance_range: Some(UtteranceRange::new(start, end).expect("valid range")),
            ..FilterConfig::default()
        }),
    }
}

macro_rules! parity_case_tests {
    ($($name:ident => $case:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let case = $case;
                let Some(outputs) = crate::harness::run_parity_case(&case) else {
                    return;
                };
                insta::assert_snapshot!(outputs.clan_snapshot, outputs.clan_output);
                insta::assert_snapshot!(outputs.rust_snapshot, outputs.rust_output);
            }
        )+
    };
}

pub(crate) use parity_case_tests;

macro_rules! rust_snapshot_tests {
    ($($name:ident => $case:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let case = $case;
                let output = crate::harness::run_rust_snapshot_case(&case);
                insta::assert_snapshot!(output.rust_snapshot, output.rust_output);
            }
        )+
    };
}

pub(crate) use rust_snapshot_tests;

/// Run a legacy CLAN command by piping file content to standard input.
pub fn run_clan(command: &str, file: &Path, args: &[&str]) -> Option<String> {
    run_clan_stdout_from_stdin(command, file, args).map(|raw| strip_clan_header(&raw))
}

/// Strip the legacy CLAN boilerplate header from command output.
pub fn strip_clan_header(output: &str) -> String {
    let mut lines: Vec<&str> = output.lines().collect();

    if let Some(pos) = lines.iter().rposition(|l| l.trim() == "From pipe input") {
        lines = lines[pos + 1..].to_vec();
    }

    if let Some(first) = lines.first() {
        let trimmed = first.trim();
        if trimmed.parse::<u64>().is_ok()
            || trimmed
                .split_whitespace()
                .next()
                .is_some_and(|w| w.parse::<u64>().is_ok())
        {
            lines = lines[1..].to_vec();
        }
    }

    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    lines.join(
        "
",
    )
}

/// Run the Rust implementation of a CLAN command and render the result.
pub fn run_rust(
    command_name: &str,
    file: &Path,
    extra_args: &[&str],
    format: OutputFormat,
) -> String {
    run_rust_filtered(command_name, file, extra_args, format, None)
}

/// Run the Rust implementation of a CLAN command with an optional filter.
pub fn run_rust_filtered(
    command_name: &str,
    file: &Path,
    extra_args: &[&str],
    format: OutputFormat,
    filter: Option<FilterConfig>,
) -> String {
    use clan_core::framework::{AnalysisRunner, CommandOutput};

    let files = vec![file.to_path_buf()];
    let runner = AnalysisRunner::with_filter(filter.unwrap_or_default());

    macro_rules! run_and_render {
        ($command:expr) => {
            match runner.run(&$command, &files) {
                Ok(r) => r.render(format),
                Err(e) => format!("Error: {e}"),
            }
        };
    }

    match command_name {
        "freq" => {
            use clan_core::commands::freq::{FreqCommand, FreqConfig};
            let use_mor = extra_args.contains(&"--mor");
            run_and_render!(FreqCommand::new(FreqConfig { use_mor }))
        }
        "mlu" => {
            use clan_core::commands::mlu::{MluCommand, MluConfig};
            let words_only = extra_args.contains(&"--words");
            run_and_render!(MluCommand::new(MluConfig { words_only }))
        }
        "mlt" => {
            use clan_core::commands::mlt::MltCommand;
            run_and_render!(MltCommand)
        }
        "wdlen" => {
            use clan_core::commands::wdlen::WdlenCommand;
            run_and_render!(WdlenCommand)
        }
        "freqpos" => {
            use clan_core::commands::freqpos::FreqposCommand;
            run_and_render!(FreqposCommand)
        }
        "cooccur" => {
            use clan_core::commands::cooccur::CooccurCommand;
            run_and_render!(CooccurCommand)
        }
        "dist" => {
            use clan_core::commands::dist::DistCommand;
            run_and_render!(DistCommand)
        }
        "maxwd" => {
            use clan_core::commands::maxwd::{MaxwdCommand, MaxwdConfig};
            let limit = extra_args
                .windows(2)
                .find(|w| w[0] == "--limit")
                .and_then(|w| w[1].parse().ok())
                .unwrap_or(20);
            run_and_render!(MaxwdCommand::new(MaxwdConfig {
                limit: clan_core::framework::WordLimit::new(limit)
            }))
        }
        "kwal" => {
            use clan_core::commands::kwal::{KwalCommand, KwalConfig};
            let keywords: Vec<String> = extra_args
                .windows(2)
                .filter(|w| w[0] == "--keyword")
                .map(|w| w[1].to_owned())
                .collect();
            run_and_render!(KwalCommand::new(KwalConfig {
                keywords: keywords
                    .into_iter()
                    .map(clan_core::framework::KeywordPattern::from)
                    .collect(),
            }))
        }
        "chip" => {
            use clan_core::commands::chip::ChipCommand;
            run_and_render!(ChipCommand)
        }
        "gemlist" => {
            use clan_core::commands::gemlist::GemlistCommand;
            run_and_render!(GemlistCommand)
        }
        "modrep" => {
            use clan_core::commands::modrep::ModrepCommand;
            run_and_render!(ModrepCommand)
        }
        "phonfreq" => {
            use clan_core::commands::phonfreq::PhonfreqCommand;
            run_and_render!(PhonfreqCommand)
        }
        "vocd" => {
            use clan_core::commands::vocd::VocdCommand;
            run_and_render!(VocdCommand::default())
        }
        "combo" => {
            use clan_core::commands::combo::{ComboCommand, ComboConfig, SearchExpr};
            let search: Vec<SearchExpr> = extra_args
                .windows(2)
                .filter(|w| w[0] == "--search")
                .map(|w| SearchExpr::parse(w[1]))
                .collect();
            run_and_render!(ComboCommand::new(ComboConfig { search }))
        }
        "codes" => {
            use clan_core::commands::codes::{CodesCommand, CodesConfig};
            run_and_render!(CodesCommand::new(CodesConfig {
                max_depth: clan_core::framework::CodeDepth::new(0)
            }))
        }
        "chains" => {
            use clan_core::commands::chains::{ChainsCommand, ChainsConfig};
            run_and_render!(ChainsCommand::new(ChainsConfig::default()))
        }
        "sugar" => {
            use clan_core::commands::sugar::{SugarCommand, SugarConfig};
            run_and_render!(SugarCommand::new(SugarConfig {
                min_utterances: clan_core::framework::UtteranceLimit::new(0)
            }))
        }
        "timedur" => {
            use clan_core::commands::timedur::TimedurCommand;
            run_and_render!(TimedurCommand)
        }
        "trnfix" => {
            use clan_core::commands::trnfix::{TrnfixCommand, TrnfixConfig};
            let tier1 = extra_args
                .windows(2)
                .find(|w| w[0] == "--tier1")
                .map(|w| w[1].to_string())
                .unwrap_or_else(|| "pho".to_string());
            let tier2 = extra_args
                .windows(2)
                .find(|w| w[0] == "--tier2")
                .map(|w| w[1].to_string())
                .unwrap_or_else(|| "mod".to_string());
            run_and_render!(TrnfixCommand::new(TrnfixConfig {
                tier1: clan_core::framework::TierKind::from(tier1.as_str()),
                tier2: clan_core::framework::TierKind::from(tier2.as_str()),
            }))
        }
        "uniq" => {
            use clan_core::commands::uniq::{UniqCommand, UniqConfig};
            run_and_render!(UniqCommand::new(UniqConfig {
                sort_by_frequency: false,
            }))
        }
        "dss" => {
            use clan_core::commands::dss::{DssCommand, DssConfig};
            let cmd = DssCommand::new(DssConfig::default()).expect("DSS init failed");
            run_and_render!(cmd)
        }
        "eval" => {
            use clan_core::commands::eval::{EvalCommand, EvalConfig};
            run_and_render!(EvalCommand::new(EvalConfig::default()))
        }
        "flucalc" => {
            use clan_core::commands::flucalc::{FlucalcCommand, FlucalcConfig};
            run_and_render!(FlucalcCommand::new(FlucalcConfig {
                syllable_mode: false,
            }))
        }
        "ipsyn" => {
            use clan_core::commands::ipsyn::{IpsynCommand, IpsynConfig};
            let cmd = IpsynCommand::new(IpsynConfig::default()).expect("IPSYN init failed");
            run_and_render!(cmd)
        }
        "kideval" => {
            use clan_core::commands::kideval::{KidevalCommand, KidevalConfig};
            let cmd = KidevalCommand::new(KidevalConfig::default()).expect("KIDEVAL init failed");
            run_and_render!(cmd)
        }
        "keymap" => {
            use clan_core::commands::keymap::{KeymapCommand, KeymapConfig};
            let keywords: Vec<String> = extra_args
                .windows(2)
                .filter(|w| w[0] == "--keyword")
                .map(|w| w[1].to_owned())
                .collect();
            let tier = extra_args
                .windows(2)
                .find(|w| w[0] == "--tier")
                .map(|w| w[1].to_string())
                .unwrap_or_else(|| "cod".to_string());
            run_and_render!(KeymapCommand::new(KeymapConfig {
                keywords: keywords
                    .into_iter()
                    .map(clan_core::framework::KeywordPattern::from)
                    .collect(),
                tier: clan_core::framework::TierKind::from(tier.as_str()),
            }))
        }
        "complexity" => {
            use clan_core::commands::complexity::ComplexityCommand;
            run_and_render!(ComplexityCommand)
        }
        "corelex" => {
            use clan_core::commands::corelex::{CorelexCommand, CorelexConfig};
            let threshold = extra_args
                .windows(2)
                .find(|w| w[0] == "--threshold")
                .and_then(|w| w[1].parse().ok())
                .unwrap_or(2);
            run_and_render!(CorelexCommand::new(CorelexConfig {
                min_frequency: clan_core::framework::FrequencyThreshold::new(threshold),
            }))
        }
        "wdsize" => {
            use clan_core::commands::wdsize::{WdsizeCommand, WdsizeConfig};
            let use_main_tier = extra_args.contains(&"--main-tier");
            run_and_render!(WdsizeCommand::new(WdsizeConfig { use_main_tier }))
        }
        other => panic!("Unknown command: {other}"),
    }
}

/// Run the Rust CHECK implementation on a file.
pub fn run_rust_check(file: &Path) -> String {
    use clan_core::commands::check::{CheckConfig, run_check};
    use clan_core::framework::CommandOutput;

    let content = std::fs::read_to_string(file).expect("Failed to read file");
    let config = CheckConfig::default();
    let result = run_check(file, &content, &config);

    if result.errors.is_empty() && !result.has_errors {
        "ALL FILES CHECKED OUT OK!".to_string()
    } else {
        result.render_text()
    }
}

/// Run the legacy CLAN CHECK implementation on a file.
pub fn run_clan_check(file: &Path) -> Option<String> {
    let bin = clan_bin_dir()?.join("check");
    if !bin.exists() {
        return None;
    }

    let file_content = std::fs::read(file).ok()?;
    let output = std::process::Command::new(&bin)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(&file_content).ok();
            }
            child.wait_with_output()
        })
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = if stderr.is_empty() {
        stdout
    } else {
        format!("{stderr}{stdout}")
    };

    Some(strip_check_output(&raw))
}

/// Strip the legacy CLAN CHECK header and footer boilerplate.
pub fn strip_check_output(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let mut result_lines = Vec::new();
    let mut in_body = false;

    for line in &lines {
        if line.contains("********") {
            in_body = true;
            continue;
        }
        if !in_body {
            continue;
        }
        if line.starts_with("check")
            || line.starts_with("  UD features")
            || line.starts_with("Language codes")
            || line.starts_with("Dep-file")
            || line.contains("conducting analyses on:")
            || line.contains("ALL speaker tiers")
            || line.contains("dependent tiers")
            || line.contains("header tiers")
        {
            break;
        }
        result_lines.push(*line);
    }

    while result_lines.first().is_some_and(|l| l.trim().is_empty()) {
        result_lines.remove(0);
    }
    while result_lines.last().is_some_and(|l| l.trim().is_empty()) {
        result_lines.pop();
    }

    result_lines.join(
        "
",
    )
}
