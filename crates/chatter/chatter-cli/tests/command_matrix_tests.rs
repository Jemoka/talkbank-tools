//! Matrix-style integration coverage for representative `chatter` command families.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;

use common::command_surface::{CoverageExpectation, SurfaceFamily, surface_group};
use common::{
    CliHarness, assert_failure, assert_success, parse_json, stderr_string, stdout_string,
    write_fixture,
};

const VALID_CHAT: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hello world .
%mor:	n|hello n|world .
@End
";

const SECOND_VALID_CHAT: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;07.|female|||Target_Child|||
*CHI:	bye cookie .
%mor:	n|bye n|cookie .
@End
";

const INVALID_CHAT_MISSING_END: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%mor:	v|want n|cookie .
";

const MULTI_SPEAKER_UPPERCASE_CHAT: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT||female|||Mother|||
*CHI:	WANT COOKIE .
%mor:	v|want n|cookie .
*MOT:	WANT COOKIE TOO .
%mor:	v|want n|cookie adv|too .
@End
";

const WHOLE_UTTERANCE_SHORTCUT_CHAT: &str = "@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hola@s amiga@s .
%mor:	n|hola n|amiga .
@End
";

const WHOLE_UTTERANCE_PRECODE_CHAT: &str = "@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	[- spa] hola amiga .
%mor:	n|hola n|amiga .
@End
";

const MISSING_EXPLICIT_LANGUAGE_HEADER_CHAT: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hola@s:spa friend .
%mor:	n|hola n|friend .
@End
";

const APPENDED_EXPLICIT_LANGUAGE_HEADER_CHAT: &str = "@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hola@s:spa friend .
%mor:	n|hola n|friend .
@End
";

fn assert_manifest_contracts(
    family: SurfaceFamily,
    command: &str,
    expectations: &[CoverageExpectation],
) {
    let group = surface_group(family);
    assert!(
        group.commands.contains(&command),
        "{family:?} manifest is missing representative command `{command}`"
    );
    for expectation in expectations {
        assert!(
            group.coverage.contains(expectation),
            "{family:?} manifest is missing {:?} coverage",
            expectation
        );
    }
}

fn parse_json_lines(output: &Output) -> Result<Vec<Value>, TestError> {
    stdout_string(output)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|error| TestError::Failure(format!("expected JSONL output: {error}")))
        })
        .collect()
}

fn summary_event(events: &[Value]) -> &Value {
    events
        .iter()
        .find(|event| event["type"].as_str() == Some("summary"))
        .expect("validation JSONL output should include a summary event")
}

fn run_command(harness: &CliHarness, args: &[&str]) -> Result<Output, TestError> {
    harness.run_output(args)
}

fn run_path_command(
    harness: &CliHarness,
    prefix: &[&str],
    path: &Path,
    suffix: &[&str],
) -> Result<Output, TestError> {
    let mut cmd = harness.chatter_cmd();
    cmd.args(prefix);
    cmd.arg(path);
    cmd.args(suffix);
    Ok(cmd.output()?)
}

fn run_prefix_clear(
    harness: &CliHarness,
    prefix: &Path,
    dry_run: bool,
) -> Result<Output, TestError> {
    let mut args = vec![
        OsString::from("cache"),
        OsString::from("clear"),
        OsString::from("--prefix"),
        prefix.as_os_str().to_os_string(),
    ];
    if dry_run {
        args.push(OsString::from("--dry-run"));
    }
    Ok(harness.chatter_cmd().args(args).output()?)
}

fn cache_stats_json(harness: &CliHarness) -> Result<Value, TestError> {
    let stats = run_command(harness, &["cache", "stats", "--json"])?;
    assert_success(&stats, "cache stats --json");
    parse_json(&stats)
}

#[test]
fn debug_fix_s_rewrites_whole_utterance_switch_in_place() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "needs-fix.cha", WHOLE_UTTERANCE_SHORTCUT_CHAT)?;

    let output = run_path_command(&harness, &["debug", "fix-s"], &file_path, &[])?;
    assert_success(&output, "debug fix-s rewrite");

    let rewritten = fs::read_to_string(&file_path)?;
    assert_eq!(rewritten, WHOLE_UTTERANCE_PRECODE_CHAT);
    Ok(())
}

#[test]
fn debug_fix_s_leaves_already_correct_file_unmodified() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(
        dir.path(),
        "already-fixed.cha",
        WHOLE_UTTERANCE_PRECODE_CHAT,
    )?;

    sleep(Duration::from_secs(1));
    let before_modified = fs::metadata(&file_path)?.modified()?;

    let output = run_path_command(&harness, &["debug", "fix-s"], &file_path, &[])?;
    assert_success(&output, "debug fix-s no-op");

    let after_modified = fs::metadata(&file_path)?.modified()?;
    let content = fs::read_to_string(&file_path)?;
    assert_eq!(content, WHOLE_UTTERANCE_PRECODE_CHAT);
    assert_eq!(
        before_modified, after_modified,
        "fix-s should not rewrite files that need no changes"
    );
    Ok(())
}

#[test]
fn debug_fix_s_appends_missing_explicit_language_to_languages_header() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(
        dir.path(),
        "missing-language.cha",
        MISSING_EXPLICIT_LANGUAGE_HEADER_CHAT,
    )?;

    let output = run_path_command(&harness, &["debug", "fix-s"], &file_path, &[])?;
    assert_success(&output, "debug fix-s append language header");

    let rewritten = fs::read_to_string(&file_path)?;
    assert_eq!(rewritten, APPENDED_EXPLICIT_LANGUAGE_HEADER_CHAT);
    Ok(())
}

#[test]
fn validate_matrix_single_file_json_reports_cached_state() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::Validation,
        "validate",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::StatefulPath,
        ],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "sample.cha", VALID_CHAT)?;

    let first = harness.run_validate(&file_path, &["--format", "json"])?;
    assert_success(&first, "first validate --format json");
    let first_events = parse_json_lines(&first)?;
    let first_file = first_events
        .iter()
        .find(|event| event["type"].as_str() == Some("file"))
        .ok_or_else(|| {
            TestError::Failure("missing file event in first validation run".to_string())
        })?;
    let first_summary = summary_event(&first_events);
    assert_eq!(first_file["status"].as_str(), Some("valid"));
    assert_eq!(first_file["file"].as_str(), file_path.to_str());
    assert_eq!(first_file["cache_hit"].as_bool(), Some(false));
    assert_eq!(first_summary["cache_hits"].as_u64(), Some(0));
    assert_eq!(first_summary["cache_misses"].as_u64(), Some(1));

    let second = harness.run_validate(&file_path, &["--format", "json"])?;
    assert_success(&second, "second validate --format json");
    let second_events = parse_json_lines(&second)?;
    let second_file = second_events
        .iter()
        .find(|event| event["type"].as_str() == Some("file"))
        .ok_or_else(|| {
            TestError::Failure("missing file event in second validation run".to_string())
        })?;
    let second_summary = summary_event(&second_events);
    assert_eq!(second_file["status"].as_str(), Some("valid"));
    assert_eq!(second_file["file"].as_str(), file_path.to_str());
    assert_eq!(second_file["cache_hit"].as_bool(), Some(true));
    assert_eq!(second_summary["cache_hits"].as_u64(), Some(1));
    assert_eq!(second_summary["cache_misses"].as_u64(), Some(0));

    Ok(())
}

#[test]
fn validate_matrix_directory_json_stream_reports_summary_and_cache_hits() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::Validation,
        "validate",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::StatefulPath,
        ],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus = dir.path().join("corpus");
    fs::create_dir_all(&corpus)?;
    write_fixture(&corpus, "a.cha", VALID_CHAT)?;
    write_fixture(&corpus, "nested/b.cha", SECOND_VALID_CHAT)?;

    let first = harness.run_validate(&corpus, &["--format", "json"])?;
    assert_success(&first, "first directory validate --format json");
    let first_events = parse_json_lines(&first)?;
    let first_summary = summary_event(&first_events);
    let first_file_events: Vec<_> = first_events
        .iter()
        .filter(|event| event["type"].as_str() == Some("file"))
        .collect();

    assert_eq!(first_file_events.len(), 2);
    assert!(
        first_file_events
            .iter()
            .all(|event| event["status"].as_str() == Some("valid"))
    );
    assert!(
        first_file_events
            .iter()
            .all(|event| event["cache_hit"].as_bool() == Some(false))
    );
    assert_eq!(first_summary["total_files"].as_u64(), Some(2));
    assert_eq!(first_summary["valid"].as_u64(), Some(2));
    assert_eq!(first_summary["invalid"].as_u64(), Some(0));
    assert_eq!(first_summary["cache_hits"].as_u64(), Some(0));
    assert_eq!(first_summary["cache_misses"].as_u64(), Some(2));
    assert_eq!(first_summary["cancelled"].as_bool(), Some(false));

    let second = harness.run_validate(&corpus, &["--format", "json"])?;
    assert_success(&second, "second directory validate --format json");
    let second_events = parse_json_lines(&second)?;
    let second_summary = summary_event(&second_events);
    let second_file_events: Vec<_> = second_events
        .iter()
        .filter(|event| event["type"].as_str() == Some("file"))
        .collect();

    assert_eq!(second_file_events.len(), 2);
    assert!(
        second_file_events
            .iter()
            .all(|event| event["cache_hit"].as_bool() == Some(true))
    );
    assert_eq!(second_summary["cache_hits"].as_u64(), Some(2));
    assert_eq!(second_summary["cache_misses"].as_u64(), Some(0));
    assert_eq!(second_summary["cache_hit_rate"].as_f64(), Some(100.0));

    Ok(())
}

#[test]
fn validate_matrix_audit_mode_writes_jsonl_without_cache_writes() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::Validation,
        "validate",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::StatefulPath,
        ],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus = dir.path().join("audit-corpus");
    fs::create_dir_all(&corpus)?;
    write_fixture(&corpus, "good.cha", VALID_CHAT)?;
    write_fixture(&corpus, "bad.cha", INVALID_CHAT_MISSING_END)?;
    let audit_path = dir.path().join("audit.jsonl");

    let mut cmd = harness.chatter_cmd();
    cmd.arg("validate")
        .arg(&corpus)
        .arg("--audit")
        .arg(&audit_path);
    let output = cmd.output()?;

    assert_failure(&output, "validate --audit on invalid corpus");
    let stdout = stdout_string(&output);
    assert!(stdout.contains("VALIDATION AUDIT SUMMARY"));
    assert!(stdout.contains("Detailed errors written to"));
    assert!(
        stdout.contains(
            audit_path
                .to_str()
                .expect("temporary audit path should be valid UTF-8")
        )
    );
    assert!(audit_path.exists(), "audit output file should be created");

    let audit_records: Vec<Value> = fs::read_to_string(&audit_path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| {
                TestError::Failure(format!("expected audit JSONL output: {error}"))
            })
        })
        .collect::<Result<_, _>>()?;

    assert_eq!(audit_records.len(), 1);
    assert_eq!(
        audit_records[0]["file"]
            .as_str()
            .map(|path| path.ends_with("bad.cha")),
        Some(true)
    );
    assert_eq!(audit_records[0]["code"].as_str(), Some("E502"));
    assert!(
        audit_records[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("@End"))
    );

    let stats = cache_stats_json(&harness)?;
    assert_eq!(stats["total_entries"].as_u64(), Some(0));

    Ok(())
}

#[test]
fn cache_matrix_stats_and_clear_commands_track_state() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::Cache,
        "cache",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::StatefulPath,
        ],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus_a = dir.path().join("corpus-a");
    let corpus_b = dir.path().join("corpus-b");
    fs::create_dir_all(&corpus_a)?;
    fs::create_dir_all(&corpus_b)?;
    let file_a = write_fixture(&corpus_a, "a.cha", VALID_CHAT)?;
    let file_b = write_fixture(&corpus_b, "b.cha", SECOND_VALID_CHAT)?;

    let validate_a = harness.run_validate(&file_a, &[])?;
    let validate_b = harness.run_validate(&file_b, &[])?;
    assert_success(&validate_a, "validate cache fixture A");
    assert_success(&validate_b, "validate cache fixture B");

    let initial_stats = cache_stats_json(&harness)?;
    assert_eq!(initial_stats["total_entries"].as_u64(), Some(2));
    let cache_dir = PathBuf::from(
        initial_stats["cache_dir"]
            .as_str()
            .expect("cache stats should report a cache directory"),
    );
    assert!(
        cache_dir.starts_with(harness.home_dir())
            || cache_dir.starts_with(harness.xdg_cache_home()),
        "cache directory {cache_dir:?} should stay inside the isolated harness roots"
    );
    assert!(
        initial_stats["cache_size_bytes"]
            .as_u64()
            .is_some_and(|size| size > 0)
    );
    assert!(
        initial_stats["last_modified"]
            .as_str()
            .is_some_and(|timestamp| timestamp.contains('T'))
    );

    let dry_run = run_prefix_clear(&harness, &corpus_a, true)?;
    assert_success(&dry_run, "cache clear --prefix --dry-run");
    assert!(stdout_string(&dry_run).contains("Would clear"));
    let stats_after_dry_run = cache_stats_json(&harness)?;
    assert_eq!(stats_after_dry_run["total_entries"].as_u64(), Some(2));

    let clear_prefix = run_prefix_clear(&harness, &corpus_a, false)?;
    assert_success(&clear_prefix, "cache clear --prefix");
    assert!(stdout_string(&clear_prefix).contains("Cleared"));
    let stats_after_prefix = cache_stats_json(&harness)?;
    assert_eq!(stats_after_prefix["total_entries"].as_u64(), Some(1));

    let clear_all = run_command(&harness, &["cache", "clear", "--all"])?;
    assert_success(&clear_all, "cache clear --all");
    let final_stats = cache_stats_json(&harness)?;
    assert_eq!(final_stats["total_entries"].as_u64(), Some(0));

    Ok(())
}

#[test]
fn cache_matrix_invalid_clear_selector_cases_fail_fast() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::Cache,
        "cache",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::StatefulPath,
        ],
    );

    struct InvalidCacheCase {
        name: &'static str,
        args: Vec<OsString>,
        expected_stderr: &'static str,
    }

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let prefix = dir.path().join("prefix");

    let cases = vec![
        InvalidCacheCase {
            name: "missing selector",
            args: vec![OsString::from("cache"), OsString::from("clear")],
            expected_stderr: "Must specify either --all or --prefix <PATH>",
        },
        InvalidCacheCase {
            name: "conflicting selectors",
            args: vec![
                OsString::from("cache"),
                OsString::from("clear"),
                OsString::from("--all"),
                OsString::from("--prefix"),
                prefix.as_os_str().to_os_string(),
            ],
            expected_stderr: "cannot be used with '--prefix <PREFIX>'",
        },
    ];

    for case in cases {
        let output = harness.chatter_cmd().args(case.args).output()?;
        assert_failure(&output, case.name);
        assert!(
            stderr_string(&output).contains(case.expected_stderr),
            "{} stderr should mention `{}`\nactual stderr:\n{}",
            case.name,
            case.expected_stderr,
            stderr_string(&output)
        );
    }

    Ok(())
}

#[test]
fn clan_matrix_analysis_formats_and_filters_are_stable() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::ClanAnalysis,
        "freq",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::OutputContract,
        ],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "analysis.cha", MULTI_SPEAKER_UPPERCASE_CHAT)?;

    let text = run_path_command(&harness, &["clan", "freq"], &file_path, &[])?;
    assert_success(&text, "clan freq text");
    let text_stdout = stdout_string(&text);
    assert!(text_stdout.contains("Speaker: CHI"));
    assert!(text_stdout.contains("Speaker: MOT"));
    assert!(text_stdout.contains("cookie"));

    let json = run_path_command(
        &harness,
        &["clan", "freq"],
        &file_path,
        &["--speaker", "CHI", "--format", "json"],
    )?;
    assert_success(&json, "clan freq --speaker CHI --format json");
    let json_value = parse_json(&json)?;
    let speakers = json_value["speakers"]
        .as_array()
        .expect("clan freq JSON output should expose speakers");
    assert_eq!(speakers.len(), 1);
    assert_eq!(speakers[0]["speaker"].as_str(), Some("CHI"));
    assert_eq!(speakers[0]["total_tokens"].as_u64(), Some(2));
    let words: BTreeSet<_> = speakers[0]["entries"]
        .as_array()
        .expect("speaker entries should be an array")
        .iter()
        .map(|entry| {
            entry["word"]
                .as_str()
                .expect("entry word should be a string")
                .to_owned()
        })
        .collect();
    let display_forms: BTreeSet<_> = speakers[0]["entries"]
        .as_array()
        .expect("speaker entries should be an array")
        .iter()
        .map(|entry| {
            entry["display_form"]
                .as_str()
                .expect("entry display_form should be a string")
                .to_owned()
        })
        .collect();
    assert_eq!(
        words,
        BTreeSet::from(["cookie".to_owned(), "want".to_owned()])
    );
    assert_eq!(
        display_forms,
        BTreeSet::from(["COOKIE".to_owned(), "WANT".to_owned()])
    );

    let csv = run_path_command(
        &harness,
        &["clan", "freq"],
        &file_path,
        &["--format", "csv"],
    )?;
    assert_success(&csv, "clan freq --format csv");
    let csv_stdout = stdout_string(&csv);
    assert!(csv_stdout.starts_with("Speaker,CHI"));
    assert!(csv_stdout.contains("Speaker,MOT"));
    assert!(csv_stdout.contains("TTR,1.000"));

    Ok(())
}

#[test]
fn clan_matrix_transforms_and_compatibility_shims_hold() -> Result<(), TestError> {
    assert_manifest_contracts(
        SurfaceFamily::ClanTransform,
        "lowcase",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::OutputContract,
        ],
    );
    assert_manifest_contracts(
        SurfaceFamily::Formatting,
        "normalize",
        &[
            CoverageExpectation::OptionMatrix,
            CoverageExpectation::OutputContract,
        ],
    );
    assert_manifest_contracts(
        SurfaceFamily::ClanCompatibilityShim,
        "fixit",
        &[CoverageExpectation::LegacyCompatibility],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "transform.cha", MULTI_SPEAKER_UPPERCASE_CHAT)?;
    let output_path = dir.path().join("lowcase.cha");

    let mut lowcase_cmd = harness.chatter_cmd();
    lowcase_cmd
        .args(["clan", "lowcase"])
        .arg(&file_path)
        .arg("--output")
        .arg(&output_path);
    let lowcase = lowcase_cmd.output()?;

    assert_success(&lowcase, "clan lowcase --output");
    assert!(stdout_string(&lowcase).is_empty());
    let rewritten = fs::read_to_string(&output_path)?;
    assert!(rewritten.contains("*CHI:	want cookie ."));
    assert!(rewritten.contains("*MOT:	want cookie too ."));
    assert!(!rewritten.contains("*CHI:	WANT COOKIE ."));

    let normalize = run_path_command(&harness, &["normalize"], &file_path, &[])?;
    let fixit = run_path_command(&harness, &["clan", "fixit"], &file_path, &[])?;
    assert_success(&normalize, "normalize");
    assert_success(&fixit, "clan fixit");
    assert_eq!(stdout_string(&fixit), stdout_string(&normalize));

    let roles = run_path_command(
        &harness,
        &["clan", "roles"],
        &file_path,
        &["--rename", "BAD"],
    )?;
    assert_failure(&roles, "clan roles --rename BAD");
    assert!(stderr_string(&roles).contains("rename must be in format OLD=NEW"));

    Ok(())
}

#[test]
fn clan_matrix_legacy_commands_cover_success_invalid_and_placeholder_edges() -> Result<(), TestError>
{
    assert_manifest_contracts(
        SurfaceFamily::ClanCompatibilityShim,
        "check",
        &[CoverageExpectation::LegacyCompatibility],
    );
    assert_manifest_contracts(
        SurfaceFamily::ClanCompatibilityShim,
        "gemfreq",
        &[CoverageExpectation::LegacyCompatibility],
    );
    assert_manifest_contracts(
        SurfaceFamily::ClanCompatibilityPlaceholder,
        "mor",
        &[CoverageExpectation::LegacyCompatibility],
    );

    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "legacy.cha", VALID_CHAT)?;

    let list_errors = run_command(&harness, &["clan", "check", "--list-errors"])?;
    assert_success(&list_errors, "clan check --list-errors");
    let list_stdout = stdout_string(&list_errors);
    assert!(list_stdout.contains("1: Expected characters are: @ or % or *."));
    assert!(list_stdout.contains("@Begin"));

    let gemfreq = run_path_command(&harness, &["clan", "gemfreq"], &file_path, &[])?;
    assert_failure(&gemfreq, "clan gemfreq without --gem");
    let gemfreq_stderr = stderr_string(&gemfreq);
    assert!(gemfreq_stderr.contains("required arguments were not provided"));
    assert!(gemfreq_stderr.contains("--gem"));

    let mor = run_command(&harness, &["clan", "mor"])?;
    assert_failure(&mor, "clan mor");
    assert!(stderr_string(&mor).contains("deliberately not implemented"));

    Ok(())
}
