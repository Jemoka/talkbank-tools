//! Stateful integration coverage for `chatter` runtime seams.

use std::fs;
use std::path::Path;

use serde_json::Value;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;

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

const INVALID_CHAT_MISSING_END: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
";

const CHAT_WITH_ALIGNMENT_ERROR: &str = "@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want .
@Comment:	ERROR: Missing n|cookie in %mor
@End
";

fn parse_json_lines_file(path: &Path) -> Result<Vec<Value>, TestError> {
    fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| {
                TestError::Failure(format!("expected audit JSONL output: {error}"))
            })
        })
        .collect()
}

fn cache_stats_json(harness: &CliHarness) -> Result<Value, TestError> {
    let stats = harness.run_output(&["cache", "stats", "--json"])?;
    assert_success(&stats, "cache stats --json");
    parse_json(&stats)
}

fn parse_json_events(output: &std::process::Output) -> Result<Vec<Value>, TestError> {
    stdout_string(output)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|error| TestError::Failure(format!("expected JSONL output: {error}")))
        })
        .collect()
}

fn first_file_event(events: &[Value]) -> Result<&Value, TestError> {
    events
        .iter()
        .find(|event| event["type"].as_str() == Some("file"))
        .ok_or_else(|| TestError::Failure("missing file event in validation JSONL".to_string()))
}

#[test]
fn audit_mode_reuses_cached_valid_results_without_cache_writes() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus = dir.path().join("corpus");
    fs::create_dir_all(&corpus)?;
    write_fixture(&corpus, "good.cha", VALID_CHAT)?;
    write_fixture(&corpus, "bad.cha", INVALID_CHAT_MISSING_END)?;

    let priming = harness.run_validate(&corpus, &["--format", "json"])?;
    assert_failure(&priming, "prime mixed corpus cache");
    let initial_stats = cache_stats_json(&harness)?;
    assert_eq!(initial_stats["total_entries"].as_u64(), Some(2));

    let audit_path = dir.path().join("audit.jsonl");
    let mut audit_cmd = harness.chatter_cmd();
    audit_cmd
        .arg("validate")
        .arg(&corpus)
        .arg("--audit")
        .arg(&audit_path);
    let audit = audit_cmd.output()?;

    assert_failure(&audit, "validate --audit with primed cache");
    let stdout = stdout_string(&audit);
    assert!(
        stdout.contains("Cache hits: 1"),
        "audit run should report one cached valid file\nstdout:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Cache misses: 1"),
        "audit run should reprocess the cached-invalid file\nstdout:\n{}",
        stdout
    );

    let records = parse_json_lines_file(&audit_path)?;
    assert_eq!(
        records.len(),
        1,
        "audit should emit one record for the invalid file"
    );
    assert_eq!(
        records[0]["file"]
            .as_str()
            .map(|path| path.ends_with("bad.cha")),
        Some(true)
    );

    let final_stats = cache_stats_json(&harness)?;
    assert_eq!(final_stats["total_entries"].as_u64(), Some(2));

    Ok(())
}

#[test]
fn audit_mode_overwrites_existing_output_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus = dir.path().join("audit-corpus");
    fs::create_dir_all(&corpus)?;
    write_fixture(&corpus, "bad.cha", INVALID_CHAT_MISSING_END)?;

    let audit_path = dir.path().join("audit.jsonl");
    fs::write(&audit_path, "stale output\n")?;

    let mut audit_cmd = harness.chatter_cmd();
    audit_cmd
        .arg("validate")
        .arg(&corpus)
        .arg("--audit")
        .arg(&audit_path);
    let audit = audit_cmd.output()?;

    assert_failure(&audit, "validate --audit should fail on invalid corpus");
    let audit_text = fs::read_to_string(&audit_path)?;
    assert!(
        !audit_text.contains("stale output"),
        "audit output should overwrite any pre-existing file contents"
    );

    let records = parse_json_lines_file(&audit_path)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["code"].as_str(), Some("E502"));

    Ok(())
}

#[test]
fn validate_force_respects_directory_boundaries_for_cache_clears() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let corpus = dir.path().join("corpus");
    let corpus_shadow = dir.path().join("corpus-shadow");
    fs::create_dir_all(&corpus)?;
    fs::create_dir_all(&corpus_shadow)?;
    let file_a = write_fixture(&corpus, "a.cha", VALID_CHAT)?;
    let file_shadow = write_fixture(&corpus_shadow, "b.cha", VALID_CHAT)?;

    let validate_a = harness.run_validate(&file_a, &["--format", "json"])?;
    let validate_shadow = harness.run_validate(&file_shadow, &["--format", "json"])?;
    assert_success(&validate_a, "prime corpus file cache");
    assert_success(&validate_shadow, "prime shadow corpus cache");

    let initial_stats = cache_stats_json(&harness)?;
    assert_eq!(initial_stats["total_entries"].as_u64(), Some(2));

    let mut force_cmd = harness.chatter_cmd();
    force_cmd
        .arg("validate")
        .arg(&corpus)
        .arg("--force")
        .arg("--format")
        .arg("json");
    let forced = force_cmd.output()?;
    assert_success(&forced, "validate directory --force");
    assert!(
        stderr_string(&forced).contains("Cleared 1 cache entries"),
        "--force should only clear entries for the requested directory\nstderr:\n{}",
        stderr_string(&forced)
    );

    let stats_after_force = cache_stats_json(&harness)?;
    assert_eq!(stats_after_force["total_entries"].as_u64(), Some(2));

    let shadow_after_force = harness.run_validate(&file_shadow, &["--format", "json"])?;
    assert_success(
        &shadow_after_force,
        "shadow file should stay cached after unrelated --force",
    );
    let shadow_events = parse_json_events(&shadow_after_force)?;
    let shadow_file = first_file_event(&shadow_events)?;
    assert_eq!(shadow_file["status"].as_str(), Some("valid"));
    assert_eq!(shadow_file["cache_hit"].as_bool(), Some(true));

    let corpus_after_force = harness.run_validate(&file_a, &["--format", "json"])?;
    assert_success(
        &corpus_after_force,
        "forced directory should repopulate its cache entry",
    );
    let corpus_events = parse_json_events(&corpus_after_force)?;
    let corpus_file = first_file_event(&corpus_events)?;
    assert_eq!(corpus_file["status"].as_str(), Some("valid"));
    assert_eq!(corpus_file["cache_hit"].as_bool(), Some(true));

    Ok(())
}

#[test]
fn skip_alignment_uses_distinct_cache_entries() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = write_fixture(dir.path(), "alignment_issue.cha", CHAT_WITH_ALIGNMENT_ERROR)?;

    let skip_alignment =
        harness.run_validate(&file_path, &["--skip-alignment", "--format", "json"])?;
    assert_success(&skip_alignment, "validate --skip-alignment --format json");
    let skip_alignment_events = parse_json_events(&skip_alignment)?;
    let skip_alignment_file = first_file_event(&skip_alignment_events)?;
    assert_eq!(skip_alignment_file["status"].as_str(), Some("valid"));
    assert_eq!(skip_alignment_file["cache_hit"].as_bool(), Some(false));

    let stats_after_skip = cache_stats_json(&harness)?;
    assert_eq!(stats_after_skip["total_entries"].as_u64(), Some(1));

    let aligned = harness.run_validate(&file_path, &["--format", "json"])?;
    assert_failure(&aligned, "validate with alignment should still fail");
    let aligned_events = parse_json_events(&aligned)?;
    let aligned_file = first_file_event(&aligned_events)?;
    assert_eq!(aligned_file["status"].as_str(), Some("invalid"));

    let stats_after_aligned = cache_stats_json(&harness)?;
    assert_eq!(stats_after_aligned["total_entries"].as_u64(), Some(2));

    let skip_alignment_again =
        harness.run_validate(&file_path, &["--skip-alignment", "--format", "json"])?;
    assert_success(
        &skip_alignment_again,
        "skip-alignment should reuse its own cached validation entry",
    );
    let skip_alignment_again_events = parse_json_events(&skip_alignment_again)?;
    let skip_alignment_again_file = first_file_event(&skip_alignment_again_events)?;
    assert_eq!(skip_alignment_again_file["status"].as_str(), Some("valid"));
    assert_eq!(skip_alignment_again_file["cache_hit"].as_bool(), Some(true));

    Ok(())
}
