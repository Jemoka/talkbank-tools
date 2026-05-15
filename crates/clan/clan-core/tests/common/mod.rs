#![allow(dead_code)]

// Shared integration-test support is compiled into several separate test
// binaries, and each suite intentionally uses only the helpers it needs.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Workspace root, located by walking up from `CARGO_MANIFEST_DIR` to the
/// nearest ancestor whose `Cargo.toml` declares `[workspace]`.
///
/// We check the `[workspace]` table explicitly rather than just
/// `Cargo.lock` because stale orphan lockfiles inside sub-crates DO
/// happen — `crates/clan-core/Cargo.lock` and
/// `apps/batchalign/dashboard-desktop/src-tauri/Cargo.lock` were both observed as
/// orphans on 2026-04-30, and a `Cargo.lock`-only walk-up will silently
/// return the wrong path. Reading the Cargo.toml content is slightly
/// slower (one stat + one small read per ancestor) but rejects orphans
/// by construction.
/// Mirrored in `talkbank-parser-re2c/tests/fixture_utils.rs::workspace_root`.
pub fn workspace_root() -> PathBuf {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    for ancestor in start.ancestors() {
        let toml = ancestor.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&toml)
            && content.contains("[workspace]")
        {
            return ancestor.to_path_buf();
        }
    }
    panic!(
        "workspace_root(): no Cargo.toml with [workspace] found above {} \
         — was the crate relocated outside its workspace?",
        start.display()
    );
}

/// Meta-repository root (parent of [`workspace_root`]) — where the
/// out-of-workspace `OSX-CLAN/` lives.
pub fn meta_repo_root() -> PathBuf {
    let ws = workspace_root();
    ws.parent()
        .unwrap_or_else(|| {
            panic!(
                "meta_repo_root(): workspace_root {} has no parent",
                ws.display()
            )
        })
        .to_path_buf()
}

/// Return the CLAN binary directory from `CLAN_BIN_DIR`, if configured.
pub fn clan_bin_dir() -> Option<PathBuf> {
    std::env::var("CLAN_BIN_DIR").ok().map(PathBuf::from)
}

/// Check whether one legacy CLAN command binary is available.
pub fn clan_command_available(command: &str) -> bool {
    clan_bin_dir()
        .map(|dir| dir.join(command).exists())
        .unwrap_or(false)
}

/// Return whether one legacy CLAN command is available, printing a consistent
/// skip message when it is not.
pub fn require_clan_command(command: &str, skip_context: &str) -> bool {
    if clan_command_available(command) {
        true
    } else {
        eprintln!("CLAN binary '{command}' not found, {skip_context}");
        false
    }
}

/// Shared temp-directory-backed CLAN test run.
pub struct ClanTempDirRun {
    temp_dir: tempfile::TempDir,
}

impl ClanTempDirRun {
    /// Run a CLAN command by piping the input file through standard input.
    pub fn from_stdin(command: &str, input_file: &Path, args: &[&str]) -> Option<Self> {
        let temp_dir = tempfile::tempdir().ok()?;
        let file_content = std::fs::read(input_file).ok()?;

        run_clan_process(
            command,
            args,
            temp_dir.path(),
            Some(&file_content),
            Option::<&Path>::None,
        )?;

        Some(Self { temp_dir })
    }

    /// Run a CLAN command by copying the input file into a temp directory and
    /// passing that copied path as a positional argument.
    pub fn with_file_argument(
        command: &str,
        input_file: &Path,
        args: &[&str],
        input_name: &str,
    ) -> Option<Self> {
        let temp_dir = tempfile::tempdir().ok()?;
        let temp_file = temp_dir.path().join(input_name);
        std::fs::copy(input_file, &temp_file).ok()?;

        run_clan_process(
            command,
            args,
            temp_dir.path(),
            None,
            Some(temp_file.as_path()),
        )?;

        Some(Self { temp_dir })
    }

    /// Read one specific output file from the temp directory.
    pub fn read_named_file(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.temp_dir.path().join(name)).ok()
    }

    /// Read the first file in the temp directory with the given extension.
    pub fn read_first_with_extension(&self, extension: &str) -> Option<String> {
        self.read_first_matching_file(|path| path.extension().is_some_and(|ext| ext == extension))
    }

    /// Read the first output file matching a caller-supplied predicate.
    pub fn read_first_matching_file(
        &self,
        mut predicate: impl FnMut(&Path) -> bool,
    ) -> Option<String> {
        std::fs::read_dir(self.temp_dir.path())
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| predicate(path))
            .and_then(|path| std::fs::read_to_string(path).ok())
    }
}

/// Run a CLAN command by piping an input file through standard input and
/// returning stdout.
pub fn run_clan_stdout_from_stdin(
    command: &str,
    input_file: &Path,
    args: &[&str],
) -> Option<String> {
    let file_content = std::fs::read(input_file).ok()?;
    let output = run_clan_process(
        command,
        args,
        Path::new("."),
        Some(&file_content),
        Option::<&Path>::None,
    )?;
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_clan_process(
    command: &str,
    args: &[&str],
    working_dir: &Path,
    stdin_content: Option<&[u8]>,
    input_arg: Option<&Path>,
) -> Option<std::process::Output> {
    let bin = clan_bin_dir()?.join(command);
    if !bin.exists() {
        return None;
    }

    let mut child = Command::new(&bin);
    child
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if stdin_content.is_some() {
        child.stdin(Stdio::piped());
    }

    if let Some(input_arg) = input_arg {
        child.arg(input_arg);
    }

    child
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;

            if let Some(stdin_content) = stdin_content
                && let Some(ref mut stdin) = child.stdin
            {
                stdin.write_all(stdin_content)?;
            }

            child.wait_with_output()
        })
        .ok()
}

/// Path to the reference corpus.
pub fn corpus_dir() -> PathBuf {
    workspace_root().join("resources/corpus/reference")
}

/// Resolve a reference corpus file path.
pub fn corpus_file(name: &str) -> PathBuf {
    corpus_dir().join(name)
}

/// Path to the converter fixture directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Resolve one converter fixture path.
pub fn fixture_file(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}
