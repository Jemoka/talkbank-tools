//! CLI integration tests
//!
//! These tests exercise the CLI commands end-to-end using assert_cmd.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::{NamedTempFile, tempdir};

// ============================================================================
// Test Fixtures
// ============================================================================

const VALID_CHAT: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hello world .
%mor:	n|hello n|world .
@End
"#;

const INVALID_CHAT_MISSING_END: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
"#;

const INVALID_CHAT_SYNTAX_ERROR: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@ world .
@End
"#;

const CHAT_WITH_ALIGNMENT_ERROR: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want .
@Comment:	ERROR: Missing n|cookie in %mor
@End
"#;

// ============================================================================
// Validate Command Tests
// ============================================================================

/// Tests validate valid file.
#[test]
fn test_validate_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total files: 1"))
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}

/// Tests validate invalid file missing end.
#[test]
fn test_validate_invalid_file_missing_end() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();
    Ok(())
}

/// Multi-file `validate` must process EVERY file passed on the command
/// line, even when an earlier file in the list is invalid. The early-exit
/// regression (fixed 2026-05-03) caused `chatter validate bad.cha good.cha`
/// to terminate after `bad.cha` and never visit `good.cha`.
#[test]
fn test_validate_multi_file_processes_every_argument() -> Result<(), TestError> {
    let dir = tempdir()?;
    let bad = dir.path().join("aa_invalid.cha");
    let good = dir.path().join("zz_valid.cha");
    fs::write(&bad, INVALID_CHAT_MISSING_END)?;
    fs::write(&good, VALID_CHAT)?;

    // Process exits non-zero (the bad file is invalid) and the summary
    // accounts for both files — proving the run did not bail out after the
    // first failure.
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&bad)
        .arg(&good)
        .assert()
        .failure()
        .stderr(predicate::str::contains("aa_invalid.cha"))
        .stdout(predicate::str::contains("Total files: 2"))
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 1"));
    Ok(())
}

/// Tests text-mode validation keeps human diagnostics on stderr.
#[test]
fn test_validate_invalid_file_text_mode_uses_stderr_for_diagnostics() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("Total files: 1"))
        .stdout(predicate::str::contains("Invalid: 1"))
        .stderr(predicate::str::contains("✗ Errors found in"))
        .stderr(predicate::str::contains("invalid.cha"))
        .stderr(predicate::str::contains(
            "Missing @End header at end of file",
        ));
    Ok(())
}

/// Tests validate invalid file syntax error.
#[test]
fn test_validate_invalid_file_syntax_error() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("syntax_error.cha");
    fs::write(&file_path, INVALID_CHAT_SYNTAX_ERROR)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();
    Ok(())
}

/// Tests JSON validation keeps machine-readable output off stderr.
#[test]
fn test_validate_invalid_file_json_mode_keeps_stderr_clean() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&file_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"status\":\"invalid\""))
        .stdout(predicate::str::contains("\"type\":\"summary\""))
        .stdout(predicate::str::contains("\"file\":\""))
        .stdout(predicate::str::contains(
            "Missing @End header at end of file",
        ))
        .stderr(predicate::str::is_empty());
    Ok(())
}

/// Tests validate file not found.
#[test]
fn test_validate_file_not_found() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg("/nonexistent/file.cha")
        .assert()
        .failure();
}

/// Tests validate quiet mode success.
#[test]
fn test_validate_quiet_mode_success() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .arg("--quiet")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("valid.cha")));
    Ok(())
}

/// Tests validate quiet mode failure.
#[test]
fn test_validate_quiet_mode_failure() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .arg("--quiet")
        .assert()
        .failure();
    Ok(())
}

/// Tests validate skip alignment.
#[test]
fn test_validate_skip_alignment() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("alignment_issue.cha");
    fs::write(&file_path, CHAT_WITH_ALIGNMENT_ERROR)?;

    // With alignment checking (default): should detect error
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();

    // With --skip-alignment: should pass (only validates structure)
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .arg("--skip-alignment")
        .assert()
        .success();
    Ok(())
}

/// Tests validate directory recursive.
#[test]
fn test_validate_directory_recursive() -> Result<(), TestError> {
    let dir = tempdir()?;

    // Create files in nested structure
    fs::write(dir.path().join("root.cha"), VALID_CHAT)?;

    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir)?;
    fs::write(subdir.join("nested.cha"), VALID_CHAT)?;

    // Directories are always validated recursively by default
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Total files: 2")); // Both files validated
    Ok(())
}

/// Tests validate json output.
#[test]
fn test_validate_json_output() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("{"));
    Ok(())
}

// ============================================================================
// Normalize Command Tests
// ============================================================================

/// Tests normalize to stdout.
#[test]
fn test_normalize_to_stdout() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("normalize")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("@UTF8"))
        .stdout(predicate::str::contains("*CHI:"));
    Ok(())
}

/// Tests normalize to file.
#[test]
fn test_normalize_to_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_path = dir.path().join("input.cha");
    let output_path = dir.path().join("output.cha");

    fs::write(&input_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("normalize")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify output file was created
    if !output_path.exists() {
        return Err(TestError::Failure("Expected output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    if !content.contains("@UTF8") || !content.contains("*CHI:") {
        return Err(TestError::Failure(
            "Normalized output missing expected headers".to_string(),
        ));
    }
    Ok(())
}

/// Tests normalize with validation.
#[test]
fn test_normalize_with_validation() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("normalize")
        .arg(&file_path)
        .arg("--validate")
        .assert()
        .failure();
    Ok(())
}

// ============================================================================
// ToJson Command Tests
// ============================================================================

/// Tests to json stdout.
#[test]
fn test_to_json_stdout() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("{"))
        .stdout(predicate::str::contains("lines"));
    Ok(())
}

/// Tests to json file.
#[test]
fn test_to_json_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_path = dir.path().join("input.cha");
    let output_path = dir.path().join("output.json");

    fs::write(&input_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify JSON file was created and is valid
    if !output_path.exists() {
        return Err(TestError::Failure("Expected JSON output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| TestError::Failure(format!("Output should be valid JSON: {err}")))?;
    Ok(())
}

/// Tests to json pretty vs compact.
#[test]
fn test_to_json_pretty_vs_compact() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    // Pretty (default)
    let pretty_output = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&file_path)
        .output()?;

    let pretty_json = String::from_utf8(pretty_output.stdout)
        .map_err(|err| TestError::Failure(format!("Invalid UTF-8: {err}")))?;

    // Compact
    let compact_output = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&file_path)
        .arg("--compact")
        .output()?;

    let compact_json = String::from_utf8(compact_output.stdout)
        .map_err(|err| TestError::Failure(format!("Invalid UTF-8: {err}")))?;

    // Pretty should have more whitespace
    if pretty_json.len() <= compact_json.len() {
        return Err(TestError::Failure(
            "Pretty JSON should be longer than compact JSON".to_string(),
        ));
    }
    if !pretty_json.contains("  ") {
        return Err(TestError::Failure(
            "Pretty JSON should contain indentation".to_string(),
        ));
    }
    Ok(())
}

/// Tests to json with validation.
#[test]
fn test_to_json_with_validation() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&file_path)
        .arg("--validate")
        .assert()
        .failure();
    Ok(())
}

// ============================================================================
// ToJson Directory Mode Tests
// ============================================================================

/// Tests to-json directory mode creates JSON files preserving structure.
#[test]
fn test_to_json_directory_mode() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    let sub_dir = input_dir.join("sub");
    fs::create_dir_all(&sub_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;
    fs::write(sub_dir.join("b.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("2 converted"));

    // Verify structure preserved
    assert!(output_dir.join("a.json").exists());
    assert!(output_dir.join("sub/b.json").exists());

    // Verify valid JSON
    let content = fs::read_to_string(output_dir.join("a.json"))?;
    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| TestError::Failure(format!("Invalid JSON: {e}")))?;
    Ok(())
}

/// Tests incremental mode skips up-to-date files.
#[test]
fn test_to_json_incremental() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run: should convert
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 converted, 0 up-to-date"));

    // Second run: should skip (up-to-date)
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("0 converted, 1 up-to-date"));
    Ok(())
}

/// Tests --force ignores mtime and reconverts all files.
#[test]
fn test_to_json_force() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success();

    // Force run: should reconvert despite up-to-date
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .arg("--force")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 converted, 0 up-to-date"));
    Ok(())
}

/// Tests --prune removes orphaned .json files.
#[test]
fn test_to_json_prune() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run: convert
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success();

    // Create an orphaned .json file
    fs::write(output_dir.join("orphan.json"), "{}")?;
    assert!(output_dir.join("orphan.json").exists());

    // Run with --prune
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .arg("--prune")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 pruned"));

    // Orphan should be gone, original should remain
    assert!(!output_dir.join("orphan.json").exists());
    assert!(output_dir.join("a.json").exists());
    Ok(())
}

/// Tests directory mode requires --output-dir.
#[test]
fn test_to_json_directory_requires_output_dir() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&input_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output-dir"));
    Ok(())
}

// ============================================================================
// FromJson Command Tests
// ============================================================================

/// Tests from json roundtrip.
#[test]
fn test_from_json_roundtrip() -> Result<(), TestError> {
    let dir = tempdir()?;
    let chat_path = dir.path().join("input.cha");
    let json_path = dir.path().join("intermediate.json");
    let output_path = dir.path().join("output.cha");

    fs::write(&chat_path, VALID_CHAT)?;

    // CHAT → JSON
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(&chat_path)
        .arg("--output")
        .arg(&json_path)
        .assert()
        .success();

    // JSON → CHAT
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("from-json")
        .arg(&json_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify output is valid CHAT
    if !output_path.exists() {
        return Err(TestError::Failure("Expected output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    if !content.contains("@UTF8") || !content.contains("*CHI:") {
        return Err(TestError::Failure(
            "Output should contain CHAT headers".to_string(),
        ));
    }
    Ok(())
}

/// Tests from json invalid json.
#[test]
fn test_from_json_invalid_json() -> Result<(), TestError> {
    let json_file = NamedTempFile::new()?;
    fs::write(json_file.path(), "{ invalid json ")?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("from-json")
        .arg(json_file.path())
        .assert()
        .failure();
    Ok(())
}

// ============================================================================
// ShowAlignment Command Tests
// ============================================================================

/// Tests show alignment basic.
#[test]
fn test_show_alignment_basic() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("show-alignment")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alignment"));
    Ok(())
}

/// Tests show alignment specific tier.
#[test]
fn test_show_alignment_specific_tier() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("show-alignment")
        .arg(&file_path)
        .arg("--tier")
        .arg("mor")
        .assert()
        .success();
    Ok(())
}

/// Tests show alignment compact mode.
#[test]
fn test_show_alignment_compact_mode() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    let normal_output = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("show-alignment")
        .arg(&file_path)
        .output()?;

    let compact_output = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("show-alignment")
        .arg(&file_path)
        .arg("--compact")
        .output()?;

    // Compact output should be shorter
    if compact_output.stdout.len() > normal_output.stdout.len() {
        return Err(TestError::Failure(
            "Compact mode should produce less or equal output".to_string(),
        ));
    }
    Ok(())
}

// ============================================================================
// Help and Version Tests
// ============================================================================

/// Tests help command.
#[test]
fn test_help_command() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("normalize"))
        .stdout(predicate::str::contains("to-json"));
}

/// Tests validate help.
#[test]
fn test_validate_help() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--skip-alignment"))
        .stdout(predicate::str::contains("--force"));
}

/// Tests no args shows help.
#[test]
fn test_no_args_shows_help() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Tests error exit codes.
#[test]
fn test_error_exit_codes() -> Result<(), TestError> {
    let dir = tempdir()?;

    // Valid file: exit code 0
    let valid_path = dir.path().join("valid.cha");
    fs::write(&valid_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&valid_path)
        .assert()
        .code(0);

    // Invalid file: exit code != 0
    let invalid_path = dir.path().join("invalid.cha");
    fs::write(&invalid_path, INVALID_CHAT_MISSING_END)?;

    let assert = assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&invalid_path)
        .assert()
        .failure();

    // Verify non-zero exit code
    assert.code(predicate::ne(0));
    Ok(())
}

/// Tests missing required argument.
#[test]
fn test_missing_required_argument() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ============================================================================
// Exit Code Contract Tests
// ============================================================================

/// Exit code 0 for a valid CHAT file (CI contract).
#[test]
fn exit_code_zero_for_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file = dir.path().join("valid.cha");
    fs::write(&file, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args(["validate", file.to_str().unwrap(), "--tui-mode", "disable"])
        .assert()
        .success(); // exit code 0
    Ok(())
}

/// Exit code 1 for an invalid CHAT file (CI contract).
#[test]
fn exit_code_nonzero_for_invalid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file = dir.path().join("invalid.cha");
    fs::write(&file, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args(["validate", file.to_str().unwrap(), "--tui-mode", "disable"])
        .assert()
        .failure(); // exit code != 0
    Ok(())
}

/// Exit code 1 for a nonexistent file path (CI contract).
#[test]
fn exit_code_nonzero_for_nonexistent_file() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args([
            "validate",
            "/tmp/nonexistent_file_12345.cha",
            "--tui-mode",
            "disable",
        ])
        .assert()
        .failure();
}

/// Exit code 2 for missing required arguments (clap usage error).
#[test]
fn exit_code_two_for_usage_error() {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args(["validate"])
        .assert()
        .code(2);
}

/// Tests that --help includes a Getting Started section for new users.
#[test]
fn help_text_includes_getting_started() -> Result<(), TestError> {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Getting started"));
    Ok(())
}

// ============================================================================
// Strict Linkers (--strict-linkers) Tests
// ============================================================================

/// CHAT file with self-completion linker (+,) and no preceding interrupted
/// utterance. Without --strict-linkers this should pass; with it, E351 fires.
const CHAT_WITH_SELF_COMPLETION_ORPHAN: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\t+, hello world .\n@End\n";

/// Self-completion orphan passes validation without --strict-linkers.
#[test]
fn strict_linkers_off_allows_orphan_self_completion() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("orphan.cha");
    fs::write(&file_path, CHAT_WITH_SELF_COMPLETION_ORPHAN)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success();
    Ok(())
}

// ============================================================================
// Cascading Error Hint Tests
// ============================================================================

/// When structural errors (E1xx-E5xx) are present but no alignment errors
/// (E7xx) were emitted, the validator should hint that alignment checks
/// may not have run because of the structural errors.
#[test]
fn cascading_error_hint_shown_for_structural_errors() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("missing_end.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "additional checks may not have run",
        ));
    Ok(())
}

/// A valid file should not show any cascading error hint.
#[test]
fn no_cascading_hint_for_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("additional checks may not have run").not())
        .stderr(predicate::str::contains("additional checks may not have run").not());
    Ok(())
}

// ============================================================================
// Strict Linkers (--strict-linkers) Tests
// ============================================================================

/// Self-completion orphan triggers E351 when --strict-linkers is enabled.
#[test]
fn strict_linkers_on_rejects_orphan_self_completion() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("orphan.cha");
    fs::write(&file_path, CHAT_WITH_SELF_COMPLETION_ORPHAN)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("validate")
        .arg("--strict-linkers")
        .arg(&file_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("E351"));
    Ok(())
}

// ============================================================================
// CLAN Help Grouping
// ============================================================================

// ============================================================================
// --list-checks
// ============================================================================

/// `chatter validate --list-checks` prints every check and exits 0 without
/// requiring a path argument. Output must mention both statuses so users know
/// the column exists.
#[test]
fn list_checks_shows_active_and_planned() -> Result<(), TestError> {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args(["validate", "--list-checks", "--tui-mode", "disable"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Active"))
        .stdout(predicate::str::contains("Planned"))
        // Spot-check a known Active and a known Planned code.
        .stdout(predicate::str::contains("E305"))
        .stdout(predicate::str::contains("E321"));
    Ok(())
}

#[test]
fn clan_help_shows_grouped_commands() -> Result<(), TestError> {
    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .args(["clan", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Analysis Commands"))
        .stdout(predicate::str::contains("Transform Commands"))
        .stdout(predicate::str::contains("Format Converters"))
        .stdout(predicate::str::contains("Compatibility Aliases"))
        .stdout(predicate::str::contains(
            "Not Available (use CLAN directly)",
        ));
    Ok(())
}

// ============================================================================
// Debug Sanitize Tests (`chatter debug sanitize`)
// ============================================================================

/// Source words must not appear in sanitize output written to stdout.
#[test]
fn test_sanitize_stdout_strips_words() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    fs::write(&input, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("debug")
        .arg("sanitize")
        .arg(&input)
        .assert()
        .success()
        .stdout(predicate::str::contains("hello").not())
        .stdout(predicate::str::contains("world").not())
        .stdout(predicate::str::contains("*CHI:"))
        .stdout(predicate::str::contains("w1"))
        .stdout(predicate::str::contains("w2"));
    Ok(())
}

/// `--output` writes the sanitized result to a file.
#[test]
fn test_sanitize_output_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    let output = dir.path().join("sanitized.cha");
    fs::write(&input, VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("debug")
        .arg("sanitize")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();

    let body = fs::read_to_string(&output)?;
    assert!(
        !body.contains("hello"),
        "source word leaked into file:\n{body}"
    );
    assert!(
        !body.contains("world"),
        "source word leaked into file:\n{body}"
    );
    assert!(body.contains("*CHI:"), "speaker code missing:\n{body}");
    assert!(body.contains("w1"), "placeholder w1 missing:\n{body}");
    Ok(())
}
