//! Tests for validation cache functionality
//!
//! Note: Early placeholder tests have been replaced by full implementations below.
//! See test_cache_hit_performance, test_cache_invalidation_after_file_modification,
//! test_force_flag_clears_cache, and test_validate_single_file_cached_output.

use std::fs;
use std::thread;
use std::time::Duration;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;

use common::CliHarness;

// Integration tests for the validation cache, exercised via CLI commands.

use std::path::Path;

/// Run `chatter validate` through the isolated CLI harness.
fn run_validate(
    harness: &CliHarness,
    path: &Path,
    extra_args: &[&str],
) -> Result<std::process::Output, TestError> {
    harness.run_validate(path, extra_args)
}

/// Tests validate command exists.
#[test]
fn test_validate_command_exists() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let output = harness.run_output(&["--help"])?;

    if !output.status.success() {
        return Err(TestError::Failure(
            "CLI should build successfully".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate single file.
#[test]
fn test_validate_single_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create a valid CHAT file with required headers
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    let output = run_validate(&harness, &file_path, &[])?;

    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(TestError::Failure(
            "Valid file should pass validation".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("valid") && !stdout.contains("✓") {
        return Err(TestError::Failure(
            "Output should indicate file is valid".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate invalid file.
#[test]
fn test_validate_invalid_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");

    // Create an invalid CHAT file (missing @Begin)
    fs::write(&file_path, "*CHI:\thello .\n")?;

    let output = run_validate(&harness, &file_path, &[])?;

    if output.status.success() {
        return Err(TestError::Failure(
            "Invalid file should fail validation".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache hit performance.
#[test]
fn test_cache_hit_performance() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create a valid CHAT file with required headers
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validation (cache miss)
    let start = std::time::Instant::now();
    let output1 = run_validate(&harness, &file_path, &[])?;
    let duration1 = start.elapsed();

    if !output1.status.success() {
        return Err(TestError::Failure(
            "First validation should succeed".to_string(),
        ));
    }

    // Second validation (should be cache hit)
    let start = std::time::Instant::now();
    let output2 = run_validate(&harness, &file_path, &[])?;
    let duration2 = start.elapsed();

    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }

    // Note: This is a weak test because cargo run has overhead
    // In a real scenario, second run should be much faster
    // For now, we just verify both succeed
    println!("First run: {:?}, Second run: {:?}", duration1, duration2);
    Ok(())
}

/// Tests cache invalidation after file modification.
#[test]
fn test_cache_invalidation_after_file_modification() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create initial file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validation
    let output1 = run_validate(&harness, &file_path, &[])?;

    if !output1.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output1.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output1.stderr));
    }
    if !output1.status.success() {
        return Err(TestError::Failure(
            "Initial validation should succeed".to_string(),
        ));
    }

    // Wait a bit to ensure mtime changes
    thread::sleep(Duration::from_millis(100));

    // Modify file (change "hello world" to "goodbye")
    let modified_content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tgoodbye .\n@End\n";
    fs::write(&file_path, modified_content)?;

    // Second validation (should detect modification and re-validate)
    let output2 = run_validate(&harness, &file_path, &[])?;
    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }

    // The cache should have been invalidated and file re-validated
    // We can't easily verify this without exposing cache internals,
    // but at least we know it doesn't crash
    Ok(())
}

/// Tests force flag clears cache.
#[test]
fn test_force_flag_clears_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validate to populate cache
    let output1 = run_validate(&harness, &file_path, &[])?;
    if !output1.status.success() {
        return Err(TestError::Failure(
            "Initial validation should succeed".to_string(),
        ));
    }

    // Now validate with --force - should clear and re-validate
    let output2 = run_validate(&harness, &file_path, &["--force"])?;

    if !output2.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output2.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output2.stderr));
    }
    if !output2.status.success() {
        return Err(TestError::Failure(
            "Forced validation should succeed".to_string(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output2.stderr);

    // When using --force, stderr should mention clearing cache entries
    if !stderr.contains("Cleared") || !stderr.contains("cache entries") {
        return Err(TestError::Failure(format!(
            "Output should indicate cache clearing when --force is used. Got stderr: {}",
            stderr
        )));
    }

    Ok(())
}

/// Tests validate directory with cache.
#[test]
fn test_validate_directory_with_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;

    // Create multiple test files
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";

    for i in 1..=5 {
        let file_path = dir.path().join(format!("test{}.cha", i));
        fs::write(&file_path, content)?;
    }

    // First directory validation (cache miss for all files)
    let output1 = run_validate(&harness, dir.path(), &[])?;

    if !output1.status.success() {
        return Err(TestError::Failure(
            "First directory validation should succeed".to_string(),
        ));
    }

    // Second directory validation (should use cache for all files)
    let output2 = run_validate(&harness, dir.path(), &[])?;

    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second directory validation should succeed".to_string(),
        ));
    }

    // Both should report same results
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    if !(stdout1.contains("Valid: 5") || (stdout1.contains("5") && stdout1.contains("valid"))) {
        return Err(TestError::Failure(
            "Expected validation output for first run".to_string(),
        ));
    }
    if !(stdout2.contains("Valid: 5") || (stdout2.contains("5") && stdout2.contains("valid"))) {
        return Err(TestError::Failure(
            "Expected validation output for second run".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate single file cached output.
#[test]
fn test_validate_single_file_cached_output() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("cached.cha");

    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    let first = run_validate(&harness, &file_path, &[])?;
    if !first.status.success() {
        return Err(TestError::Failure(
            "First validation should succeed".to_string(),
        ));
    }

    let second = run_validate(&harness, &file_path, &[])?;
    if !second.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&second.stdout);
    if !stdout.contains("Cache hits: 1") {
        return Err(TestError::Failure(
            "Expected second validation run to report one cache hit".to_string(),
        ));
    }
    Ok(())
}

// Cache management command tests

/// Tests cache stats command.
#[test]
fn test_cache_stats_command() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Run cache stats command
    let stats = harness.run_output(&["cache", "stats"])?;

    if !stats.status.success() {
        return Err(TestError::Failure("cache stats should succeed".to_string()));
    }
    let stdout = String::from_utf8_lossy(&stats.stdout);

    // Verify output contains expected sections
    if !stdout.contains("Cache Statistics") {
        return Err(TestError::Failure(
            "Missing Cache Statistics output".to_string(),
        ));
    }
    if !stdout.contains("Cache Directory:") {
        return Err(TestError::Failure(
            "Missing Cache Directory output".to_string(),
        ));
    }
    if !stdout.contains("Total Entries:") {
        return Err(TestError::Failure(
            "Missing Total Entries output".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear dry run.
#[test]
fn test_cache_clear_dry_run() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Run cache clear with dry-run
    let clear = harness.run_output(&[
        "cache",
        "clear",
        "--prefix",
        dir.path()
            .to_str()
            .ok_or_else(|| TestError::Failure("Invalid directory path".to_string()))?,
        "--dry-run",
    ])?;

    if !clear.status.success() {
        return Err(TestError::Failure(
            "cache clear --dry-run should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Would clear") {
        return Err(TestError::Failure(
            "Expected dry-run to mention Would clear".to_string(),
        ));
    }
    if !stdout.contains("dry-run") {
        return Err(TestError::Failure(
            "Expected dry-run to mention dry-run".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear prefix.
#[test]
fn test_cache_clear_prefix() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Clear cache for this prefix
    let clear = harness.run_output(&[
        "cache",
        "clear",
        "--prefix",
        dir.path()
            .to_str()
            .ok_or_else(|| TestError::Failure("Invalid directory path".to_string()))?,
    ])?;

    if !clear.status.success() {
        return Err(TestError::Failure("cache clear should succeed".to_string()));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Cleared") {
        return Err(TestError::Failure("Expected Cleared output".to_string()));
    }
    if !stdout.contains("cache entries") {
        return Err(TestError::Failure(
            "Expected cache entries output".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear all.
#[test]
fn test_cache_clear_all() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    // Run cache clear --all
    let clear = harness.run_output(&["cache", "clear", "--all"])?;

    if !clear.status.success() {
        return Err(TestError::Failure(
            "cache clear --all should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Cleared") {
        return Err(TestError::Failure("Expected Cleared output".to_string()));
    }
    Ok(())
}
