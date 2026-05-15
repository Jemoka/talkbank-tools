//! Golden tests comparing clan-core transform output against CLAN CLI output.
//!
//! These tests run the legacy CLAN binaries (C/C++) on reference corpus files
//! and compare their output against our Rust reimplementation. Transform commands
//! write to output files (e.g., `pipeout.flo.cex`) rather than stdout.
//!
//! # Requirements
//!
//! - CLAN binaries must be available at the path specified by `CLAN_BIN_DIR`
//! - Tests are skipped if the binaries are not found (CI-safe)
//!
//! # Snapshot Review
//!
//! Each test produces two insta snapshots:
//! - `<test>@clan` — the legacy CLAN output (reference)
//! - `<test>@rust` — our Rust implementation output
//!
//! Review with `cargo insta review -p clan-core`.

mod common;

use std::path::Path;

use common::{ClanTempDirRun, clan_command_available, corpus_file, require_clan_command};
use clan_core::framework::TransformCommand;
use clan_core::transforms::compound::{CompoundCommand, CompoundConfig};
use clan_core::transforms::dates::DatesCommand;
use clan_core::transforms::delim::DelimCommand;
use clan_core::transforms::fixbullets::FixbulletsCommand;
use clan_core::transforms::flo::FloCommand;
use clan_core::transforms::lowcase::LowcaseCommand;
use clan_core::transforms::repeat::RepeatCommand;
use clan_core::transforms::retrace::RetraceCommand;
use clan_core::transforms::tierorder::TierorderCommand;
use talkbank_model::{SpeakerCode, WriteChat};

/// Run a CLAN transform command on a file by piping its content to stdin.
///
/// CLAN transform commands write output to a file (e.g., `pipeout.flo.cex`),
/// not stdout. We run in a temp directory and capture the output file.
fn run_clan_transform(command: &str, file: &Path, args: &[&str]) -> Option<String> {
    let run = ClanTempDirRun::from_stdin(command, file, args)?;
    run.read_named_file(&format!("pipeout.{}.cex", transform_output_stem(command)))
        .or_else(|| run.read_first_with_extension("cex"))
}

/// Run our Rust transform command on a file and return the output string.
fn run_rust_transform(command: &str, file: &Path) -> String {
    let content = std::fs::read_to_string(file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    match command {
        "flo" => FloCommand
            .transform(&mut chat_file)
            .expect("FLO transform failed"),
        "lowcase" => LowcaseCommand
            .transform(&mut chat_file)
            .expect("LOWCASE transform failed"),
        "retrace" => RetraceCommand
            .transform(&mut chat_file)
            .expect("RETRACE transform failed"),
        "delim" => DelimCommand
            .transform(&mut chat_file)
            .expect("DELIM transform failed"),
        "fixbullets" => FixbulletsCommand::default()
            .transform(&mut chat_file)
            .expect("FIXBULLETS transform failed"),
        "repeat" => RepeatCommand::new(SpeakerCode::from("CHI"))
            .transform(&mut chat_file)
            .expect("REPEAT transform failed"),
        "tierorder" => TierorderCommand
            .transform(&mut chat_file)
            .expect("TIERORDER transform failed"),
        "dates" => DatesCommand
            .transform(&mut chat_file)
            .expect("DATES transform failed"),
        "compound" => CompoundCommand::new(CompoundConfig { dash_to_plus: true })
            .transform(&mut chat_file)
            .expect("COMPOUND transform failed"),
        other => panic!("Unknown transform command: {other}"),
    }

    chat_file.to_chat_string()
}

/// Run a CLAN transform command on a file by passing the file path as an argument.
///
/// Some CLAN commands (compound, dates) don't support pipe input and require
/// file arguments. We copy the file to a temp directory and pass the path.
fn run_clan_transform_with_file(command: &str, file: &Path, args: &[&str]) -> Option<String> {
    let run = ClanTempDirRun::with_file_argument(command, file, args, "input.cha")?;
    run.read_first_with_extension("cex")
}

fn transform_output_stem(command: &str) -> &str {
    match command {
        "flo" => "flo",
        "lowcase" => "lowcas",
        "fixbullets" => "fxblts",
        "repeat" => "rpeat",
        "chstring" => "chstr",
        "compound" => "cmpund",
        "tierorder" => "trordr",
        "dataclean" => "datcln",
        other => other,
    }
}

#[derive(Clone, Copy)]
enum ClanTransformRunner {
    Stdin,
    FileArgument,
}

fn run_transform_parity_case(
    file_name: &str,
    command: &str,
    clan_args: &[&str],
    clan_snapshot: &str,
    rust_snapshot: &str,
    runner: ClanTransformRunner,
    exact_match_message: Option<&str>,
) {
    if !require_clan_command(command, "skipping golden test") {
        return;
    }

    let file = corpus_file(file_name);
    let clan_output = match runner {
        ClanTransformRunner::Stdin => run_clan_transform(command, &file, clan_args),
        ClanTransformRunner::FileArgument => {
            run_clan_transform_with_file(command, &file, clan_args)
        }
    }
    .unwrap_or_else(|| panic!("CLAN {command} failed"));
    let rust_output = run_rust_transform(command, &file);

    insta::assert_snapshot!(clan_snapshot, &clan_output);
    insta::assert_snapshot!(rust_snapshot, &rust_output);

    if let Some(exact_match_message) = exact_match_message {
        assert_eq!(
            rust_output.trim(),
            clan_output.trim(),
            "{exact_match_message}"
        );
    }
}

macro_rules! transform_parity_tests {
    ($($name:ident => {
        file: $file:expr,
        command: $command:expr,
        clan_args: $clan_args:expr,
        clan_snapshot: $clan_snapshot:expr,
        rust_snapshot: $rust_snapshot:expr,
        runner: $runner:expr,
        exact_match_message: $exact_match_message:expr
    };)+) => {
        $(
            #[test]
            fn $name() {
                run_transform_parity_case(
                    $file,
                    $command,
                    $clan_args,
                    $clan_snapshot,
                    $rust_snapshot,
                    $runner,
                    $exact_match_message,
                );
            }
        )+
    };
}

transform_parity_tests! {
    golden_flo_mor_gra => {
        file: "tiers/mor-gra.cha",
        command: "flo",
        clan_args: &[],
        clan_snapshot: "flo_mor_gra@clan",
        rust_snapshot: "flo_mor_gra@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: Some("FLO output should match legacy CLAN exactly")
    };
    golden_flo_eng => {
        file: "languages/eng-conversation.cha",
        command: "flo",
        clan_args: &[],
        clan_snapshot: "flo_eng@clan",
        rust_snapshot: "flo_eng@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: Some("FLO output should match legacy CLAN exactly")
    };
    // Pronoun "I" is preserved as uppercase, matching CLAN.
    golden_lowcase_mor_gra => {
        file: "tiers/mor-gra.cha",
        command: "lowcase",
        clan_args: &[],
        clan_snapshot: "lowcase_mor_gra@clan",
        rust_snapshot: "lowcase_mor_gra@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: None
    };
    golden_retrace_retrace => {
        file: "annotation/retrace.cha",
        command: "retrace",
        clan_args: &[],
        clan_snapshot: "retrace_retrace@clan",
        rust_snapshot: "retrace_retrace@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: Some("RETRACE output should match legacy CLAN exactly")
    };
    // NOTE: CLAN bug — CLAN erroneously marks fragments (`&~frag`) as repetitions
    // with `[+ rep]`. Fragments are not revisions or repetitions; they are
    // incomplete word attempts. Our Rust implementation correctly skips fragments.
    // We intentionally diverge from CLAN output here.
    golden_repeat_retrace => {
        file: "annotation/retrace.cha",
        command: "repeat",
        clan_args: &["+t*CHI"],
        clan_snapshot: "repeat_retrace@clan",
        rust_snapshot: "repeat_retrace@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: None
    };
    // NOTE: CLAN bug — CLAN uses dependent tier bullet `2061689` as the next
    // main tier start, producing `2061690_2061691`. Our Rust implementation
    // correctly uses the main tier bullet `2042652` and preserves duration,
    // producing `2051689_2052652`. The file's own `@Comment` says "dependent
    // tier bullet should not count in start checking." We intentionally diverge.
    golden_fixbullets_bullets => {
        file: "content/media-bullets.cha",
        command: "fixbullets",
        clan_args: &[],
        clan_snapshot: "fixbullets_bullets@clan",
        rust_snapshot: "fixbullets_bullets@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: None
    };
    golden_delim_retrace => {
        file: "annotation/retrace.cha",
        command: "delim",
        clan_args: &[],
        clan_snapshot: "delim_retrace@clan",
        rust_snapshot: "delim_retrace@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: None
    };
    golden_tierorder_pho => {
        file: "tiers/pho.cha",
        command: "tierorder",
        clan_args: &[],
        clan_snapshot: "tierorder_pho@clan",
        rust_snapshot: "tierorder_pho@rust",
        runner: ClanTransformRunner::Stdin,
        exact_match_message: None
    };
    golden_dates_speaker_info => {
        file: "core/headers-speaker-info.cha",
        command: "dates",
        clan_args: &[],
        clan_snapshot: "dates_speaker_info@clan",
        rust_snapshot: "dates_speaker_info@rust",
        runner: ClanTransformRunner::FileArgument,
        exact_match_message: None
    };
    golden_compound_basic => {
        file: "core/basic-conversation.cha",
        command: "compound",
        clan_args: &[],
        clan_snapshot: "compound_basic@clan",
        rust_snapshot: "compound_basic@rust",
        runner: ClanTransformRunner::FileArgument,
        exact_match_message: None
    };
}

// ── CHSTRING golden tests ────────────────────────────────────────────
//
// CHSTRING requires a changes file with find/replace pairs. We create
// a temporary changes file and run the Rust transform directly.

#[test]
fn golden_chstring_basic() {
    use clan_core::transforms::chstring::ChstringCommand;

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    // Create a changes file: replace "cookies" with "candy"
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let changes_path = temp_dir.path().join("changes.cut");
    std::fs::write(&changes_path, "cookies\ncandy\n").expect("Failed to write changes file");

    let cmd = ChstringCommand::new(changes_path);
    cmd.transform(&mut chat_file)
        .expect("CHSTRING transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("chstring_basic@rust", &rust_output);
}

// ── COMBTIER golden tests ────────────────────────────────────────────
//
// COMBTIER requires a file with multiple instances of the same dependent
// tier on a single utterance. The reference corpus doesn't have this, so
// we create a test CHAT file with duplicate %com tiers.

#[test]
fn golden_combtier_coding() {
    use clan_core::transforms::combtier::{CombtierCommand, CombtierConfig};

    // Create a file with multiple %com tiers per utterance
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let input_path = temp_dir.path().join("combtier-input.cha");
    std::fs::write(
        &input_path,
        "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tCHI Child, MOT Mother\n\
@ID:\teng|corpus|CHI|3;00.||||Child|||\n\
@ID:\teng|corpus|MOT|||||Mother|||\n\
*CHI:\thello there .\n\
%com:\tfirst comment\n\
%com:\tsecond comment\n\
*MOT:\tgoodbye now .\n\
%com:\tonly comment\n\
@End\n",
    )
    .expect("Failed to write input file");

    let content = std::fs::read_to_string(&input_path).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    let cmd = CombtierCommand::new(CombtierConfig {
        tier: "com".to_owned(),
        separator: " ".to_owned(),
    });
    cmd.transform(&mut chat_file)
        .expect("COMBTIER transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("combtier_coding@rust", &rust_output);
}

// ── DATACLEAN golden tests ──────────────────────────────────────────
//
// DATACLEAN applies text-level formatting fixes (bracket spacing, ellipsis
// normalization, etc.). It uses a custom run function on serialized text.

#[test]
fn golden_dataclean_retrace() {
    use clan_core::transforms::dataclean::clean_chat_text;

    let file = corpus_file("annotation/retrace.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");
    let serialized = chat_file.to_chat_string();
    let rust_output = clean_chat_text(&serialized);

    insta::assert_snapshot!("dataclean_retrace@rust", &rust_output);
}

// ── LINES golden tests ──────────────────────────────────────────────
//
// LINES adds or removes line number prefixes on non-header lines.

#[test]
fn golden_lines_add_basic() {
    use clan_core::transforms::lines::add_line_numbers;

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");
    let serialized = chat_file.to_chat_string();
    let rust_output = add_line_numbers(&serialized);

    insta::assert_snapshot!("lines_add_basic@rust", &rust_output);
}

#[test]
fn golden_lines_roundtrip_basic() {
    use clan_core::transforms::lines::{add_line_numbers, remove_line_numbers};

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");
    let serialized = chat_file.to_chat_string();
    let numbered = add_line_numbers(&serialized);
    let rust_output = remove_line_numbers(&numbered);

    // After add then remove, we should get back the original (modulo trailing newlines)
    assert_eq!(
        rust_output.trim(),
        serialized.trim(),
        "LINES add+remove roundtrip should produce original text"
    );
}

// ── MAKEMOD golden tests ────────────────────────────────────────────
//
// MAKEMOD requires a CMU-format pronunciation lexicon file. We create
// a minimal lexicon with entries for words in the test file.

#[test]
fn golden_makemod_basic() {
    use clan_core::transforms::makemod::{MakemodCommand, MakemodConfig};

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    // Create a minimal CMU lexicon
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let lexicon_path = temp_dir.path().join("cmulex.cut");
    std::fs::write(
        &lexicon_path,
        "# CMU Pronunciation Dictionary (minimal test)\n\
I  AY1\n\
WANT  W AA1 N T\n\
SOME  S AH1 M\n\
COOKIES  K UH1 K IY0 Z\n\
WHAT  W AH1 T\n\
KIND  K AY1 N D\n\
OF  AH1 V\n\
THE  DH AH0\n\
CHOCOLATE  CH AA1 K L AH0 T\n\
ONES  W AH1 N Z\n\
WE  W IY1\n\
CAN  K AE1 N\n\
MAKE  M EY1 K\n\
TOGETHER  T AH0 G EH1 DH ER0\n",
    )
    .expect("Failed to write lexicon file");

    let config = MakemodConfig {
        lexicon_path: lexicon_path.clone(),
        all_alternatives: false,
    };
    let cmd = MakemodCommand::new(config).expect("MAKEMOD init failed");
    cmd.transform(&mut chat_file)
        .expect("MAKEMOD transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("makemod_basic@rust", &rust_output);
}

// ── ORT golden tests ────────────────────────────────────────────────
//
// ORT applies orthographic conversion from a dictionary file. We create
// a minimal dictionary and apply it to a corpus file.

#[test]
fn golden_ort_basic() {
    use clan_core::transforms::ort::{OrtCommand, OrtConfig};

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    // Create a minimal orthographic conversion dictionary
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let dict_path = temp_dir.path().join("ort.cut");
    std::fs::write(
        &dict_path,
        "# Orthographic conversion dictionary (minimal test)\n\
cookies\tbiscuits\n\
chocolate\tchoco\n",
    )
    .expect("Failed to write dictionary file");

    let config = OrtConfig {
        dictionary_path: dict_path.clone(),
    };
    let cmd = OrtCommand::new(config).expect("ORT init failed");
    cmd.transform(&mut chat_file).expect("ORT transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("ort_basic@rust", &rust_output);
}

// ── POSTMORTEM golden tests ─────────────────────────────────────────
//
// POSTMORTEM can rewrite user-defined text tiers, but typed %mor rewrites are
// now intentionally unsupported until implemented through the AST.

#[test]
fn golden_postmortem_mor_gra() {
    use clan_core::transforms::postmortem::{PostmortemCommand, PostmortemConfig};

    let file = corpus_file("tiers/mor-gra.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    // Create a minimal postmortem rules file
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let rules_path = temp_dir.path().join("postmortem.cut");
    std::fs::write(
        &rules_path,
        "# Postmortem rules (minimal test)\n\
pron|I-Prs-Nom-S1 => pron|I-Prs-Nom-P1\n\
noun|cookie-Plur => noun|biscuit-Plur\n",
    )
    .expect("Failed to write rules file");

    let config = PostmortemConfig {
        rules_path: rules_path.clone(),
        target_tier: "mor".to_owned(),
    };
    let cmd = PostmortemCommand::new(config).expect("POSTMORTEM init failed");
    let err = cmd
        .transform(&mut chat_file)
        .expect_err("POSTMORTEM should reject typed %mor rewrites");
    let msg = err.to_string();
    assert!(msg.contains("does not support degrading %mor"));
    assert!(msg.contains("AST-based %mor rewrite"));
}

// ── QUOTES golden tests ─────────────────────────────────────────────
//
// QUOTES extracts quoted text marked with [+ "] postcodes into separate
// continuation utterances. It operates on serialized text via run_quotes().

#[test]
fn golden_quotes_quotations() {
    use clan_core::transforms::quotes::run_quotes;

    let file = corpus_file("content/quotations.cha");
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output.cha");

    run_quotes(&file, Some(&output_path)).expect("QUOTES transform failed");

    let rust_output = std::fs::read_to_string(&output_path).expect("Failed to read output file");
    insta::assert_snapshot!("quotes_quotations@rust", &rust_output);
}

// ── LONGTIER golden tests ──────────────────────────────────────────
//
// LONGTIER folds continuation lines (newline+tab) in CHAT files.
// We compare against the CLAN longtier binary output.

#[test]
fn golden_longtier_mor_gra() {
    use clan_core::transforms::longtier::fold_continuation_lines;

    let file = corpus_file("tiers/mor-gra.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let result = fold_continuation_lines(&content);
    let rust_output = result.content;

    // Compare against CLAN binary if available
    if clan_command_available("longtier")
        && let Some(clan_output) = run_clan_transform("longtier", &file, &[])
    {
        insta::assert_snapshot!("longtier_mor_gra@clan", &clan_output);
    }

    insta::assert_snapshot!("longtier_mor_gra@rust", &rust_output);
}

// ── FIXIT golden tests ─────────────────────────────────────────────
//
// FIXIT normalizes CHAT formatting via parse-serialize roundtrip.

#[test]
fn golden_fixit_mor_gra() {
    use clan_core::transforms::fixit::FixitCommand;

    let file = corpus_file("tiers/mor-gra.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    FixitCommand
        .transform(&mut chat_file)
        .expect("FIXIT transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("fixit_mor_gra@rust", &rust_output);
}

// ── GEM golden tests ────────────────────────────────────────────────
//
// GEM extracts utterances within @Bg/@Eg gem boundaries.

#[test]
fn golden_gem_episodes() {
    use clan_core::transforms::gem::GemCommand;

    let file = corpus_file("core/headers-episodes.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    GemCommand::default()
        .transform(&mut chat_file)
        .expect("GEM transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("gem_episodes@rust", &rust_output);
}

#[test]
fn golden_gem_filtered() {
    use clan_core::transforms::gem::GemCommand;

    let file = corpus_file("core/headers-episodes.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    GemCommand::new(vec!["afternoon play".to_owned()])
        .transform(&mut chat_file)
        .expect("GEM transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("gem_filtered@rust", &rust_output);
}

// ── INDENT golden tests ─────────────────────────────────────────────
//
// NOTE: CLAN's indent binary has an infinite-loop bug on some inputs,
// so no CLAN parity comparison is possible. Rust-only snapshot.

#[test]
fn golden_indent_overlaps() {
    use clan_core::transforms::indent::indent_text;

    let file = corpus_file("ca/overlaps.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let rust_output = indent_text(&content, "overlaps.cha");

    insta::assert_snapshot!("indent_overlaps@rust", &rust_output);
}

// ── ROLES golden tests ──────────────────────────────────────────────
//
// ROLES renames speaker codes throughout a CHAT file. Rust-only snapshot
// since this is an AST-level operation with no direct CLAN binary comparison.

#[test]
fn golden_roles_rename() {
    use clan_core::transforms::roles::{RolesCommand, RolesConfig};

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    let cmd = RolesCommand {
        config: RolesConfig {
            renames: vec![("CHI".to_owned(), "TAR".to_owned())],
        },
    };
    cmd.transform(&mut chat_file)
        .expect("ROLES transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("roles_rename@rust", &rust_output);
}

// ── TRIM golden tests ───────────────────────────────────────────────
//
// TRIM removes selected dependent tiers. Rust-only snapshot.

#[test]
fn golden_trim_exclude_mor() {
    use clan_core::transforms::trim::{TrimCommand, TrimConfig};

    let file = corpus_file("tiers/mor-gra.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    let cmd = TrimCommand {
        config: TrimConfig {
            include_tiers: vec![],
            exclude_tiers: vec!["mor".into()],
        },
    };
    cmd.transform(&mut chat_file)
        .expect("TRIM transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("trim_exclude_mor@rust", &rust_output);
}

#[test]
fn golden_trim_exclude_all() {
    use clan_core::transforms::trim::{TrimCommand, TrimConfig};

    let file = corpus_file("tiers/mor-gra.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read input file");
    let options = talkbank_model::ParseValidateOptions::default();
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, options).expect("Failed to parse file");

    let cmd = TrimCommand {
        config: TrimConfig {
            include_tiers: vec![],
            exclude_tiers: vec!["*".into()],
        },
    };
    cmd.transform(&mut chat_file)
        .expect("TRIM transform failed");

    let rust_output = chat_file.to_chat_string();
    insta::assert_snapshot!("trim_exclude_all@rust", &rust_output);
}
