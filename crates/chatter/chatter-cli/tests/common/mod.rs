#![allow(dead_code)]

//! Shared CLI integration-test harness for `chatter`.

pub mod command_surface;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;
use talkbank_parser_tests::test_error::TestError;
use tempfile::{TempDir, tempdir};

/// Isolated integration-test harness for running the `chatter` binary.
#[allow(dead_code)]
pub struct CliHarness {
    _scratch: TempDir,
    home_dir: PathBuf,
    xdg_cache_home: PathBuf,
}

#[allow(dead_code)]
impl CliHarness {
    /// Create a new harness with isolated HOME and XDG cache roots.
    pub fn new() -> Result<Self, TestError> {
        let scratch = tempdir()?;
        let home_dir = scratch.path().join("home");
        let xdg_cache_home = home_dir.join(".cache");
        fs::create_dir_all(&xdg_cache_home)?;

        Ok(Self {
            _scratch: scratch,
            home_dir,
            xdg_cache_home,
        })
    }

    /// Build a `chatter` command configured to avoid user-machine cache state.
    pub fn chatter_cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("chatter");
        cmd.env("HOME", self.home_dir());
        cmd.env("XDG_CACHE_HOME", self.xdg_cache_home());
        cmd
    }

    /// Run `chatter` and capture the subprocess output.
    pub fn run_output(&self, args: &[&str]) -> Result<Output, TestError> {
        Ok(self.chatter_cmd().args(args).output()?)
    }

    /// Run `chatter validate` for a file or directory path.
    pub fn run_validate(&self, path: &Path, extra_args: &[&str]) -> Result<Output, TestError> {
        let mut cmd = self.chatter_cmd();
        cmd.arg("validate");
        cmd.args(extra_args);
        cmd.arg(path);
        Ok(cmd.output()?)
    }

    /// HOME directory injected into the CLI process.
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    /// XDG cache root injected into the CLI process.
    pub fn xdg_cache_home(&self) -> &Path {
        &self.xdg_cache_home
    }
}

/// Write one test fixture relative to a temp root, creating parent dirs first.
pub fn write_fixture(path: &Path, relative: &str, content: &str) -> Result<PathBuf, TestError> {
    let file_path = path.join(relative);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Decode one subprocess stdout payload as UTF-8 lossily.
pub fn stdout_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Decode one subprocess stderr payload as UTF-8 lossily.
pub fn stderr_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Assert a CLI subprocess succeeded and print captured output on failure.
pub fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout_string(output),
        stderr_string(output)
    );
}

/// Assert a CLI subprocess failed and print captured output if it unexpectedly passed.
pub fn assert_failure(output: &Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout_string(output),
        stderr_string(output)
    );
}

/// Parse one subprocess stdout payload as JSON.
pub fn parse_json(output: &Output) -> Result<Value, TestError> {
    serde_json::from_slice(&output.stdout)
        .map_err(|error| TestError::Failure(format!("expected JSON output: {error}")))
}
