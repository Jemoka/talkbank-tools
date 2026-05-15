//! Python runtime resolution for worker subprocesses.
//!
//! Resolution order:
//! 1. `BATCHALIGN_PYTHON` (explicit override)
//! 2. `VIRTUAL_ENV` interpreter (preferring Python 3.12 names where available)
//! 3. Sibling python in the same directory as the binary (pip-installed case)
//! 4. Walk up from the binary looking for a `.venv` that has batchalign
//! 5. `python3.12` on Unix-like systems, or `python` on Windows

use std::path::{Path, PathBuf};

/// Runtime-owned inputs for resolving the worker Python executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonRuntime {
    explicit_python: Option<String>,
    venv_dir: Option<PathBuf>,
    current_exe: Option<PathBuf>,
}

impl PythonRuntime {
    /// Resolve Python runtime inputs from ambient environment variables and the
    /// current executable path.
    pub fn from_env() -> Self {
        let explicit_python = std::env::var("BATCHALIGN_PYTHON").ok();
        let venv_dir = std::env::var("VIRTUAL_ENV").ok();
        let current_exe = std::env::current_exe().ok();
        Self::from_sources(
            explicit_python.as_deref(),
            venv_dir.as_deref().map(Path::new),
            current_exe.as_deref(),
        )
    }

    /// Build the runtime inputs from explicit sources.
    pub fn from_sources(
        explicit_python: Option<&str>,
        venv_dir: Option<&Path>,
        current_exe: Option<&Path>,
    ) -> Self {
        Self {
            explicit_python: explicit_python
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(ToOwned::to_owned),
            venv_dir: venv_dir.map(Path::to_path_buf),
            current_exe: current_exe.map(Path::to_path_buf),
        }
    }

    /// Resolve the Python executable from the owned runtime inputs.
    pub fn resolve_executable(&self) -> String {
        if let Some(path) = &self.explicit_python {
            return path.clone();
        }

        if let Some(dir) = self.venv_dir.as_deref()
            && let Some(candidate) = first_existing_python(venv_python_candidates(dir))
        {
            return candidate.to_string_lossy().to_string();
        }

        if let Some(path) = discover_sibling_python(self.current_exe.as_deref()) {
            return path;
        }
        if let Some(path) = discover_venv_from_binary(self.current_exe.as_deref()) {
            return path;
        }

        default_python_command().to_string()
    }
}

fn venv_python_candidates(venv_dir: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![venv_dir.join("Scripts").join("python.exe")]
    } else {
        let bin = venv_dir.join("bin");
        vec![
            bin.join("python3.12"),
            bin.join("python3"),
            bin.join("python"),
        ]
    }
}

/// Check whether a Python executable can import `batchalign.worker`.
fn python_has_batchalign(python: &Path) -> bool {
    std::process::Command::new(python)
        .args(["-c", "import batchalign.worker"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn first_existing_python(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn sibling_python_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["python.exe"]
    } else {
        &["python3.12", "python3", "python"]
    }
}

fn default_python_command() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3.12"
    }
}

/// Check for a sibling Python in the same directory as the running binary.
///
/// When the binary is installed into a venv via `pip install` (e.g.
/// `.venv/bin/batchalign3`), the venv's Python sits right next to it at
/// `.venv/bin/python3.12` (or another preferred sibling name). This handles
/// the common case where `VIRTUAL_ENV`
/// is not set (venv not activated).
fn discover_sibling_python(current_exe: Option<&Path>) -> Option<String> {
    let exe = current_exe?;
    let bin_dir = exe.parent()?;
    for name in sibling_python_names() {
        let candidate = bin_dir.join(name);
        if candidate.is_file() && python_has_batchalign(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Walk up from the running binary looking for a `.venv` that has batchalign.
///
/// The binary typically lives at `batchalign3/target/{profile}/batchalign3`,
/// and the project venv is at `batchalign3/.venv/`. Walking up ancestors of the
/// binary will find it.
fn discover_venv_from_binary(current_exe: Option<&Path>) -> Option<String> {
    let exe = current_exe?;
    let mut dir = exe.parent();

    while let Some(d) = dir {
        for candidate in venv_python_candidates(&d.join(".venv")) {
            if candidate.is_file() && python_has_batchalign(&candidate) {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        dir = d.parent();
    }
    None
}

/// Resolve Python executable for worker subprocesses.
pub fn resolve_python_executable() -> String {
    PythonRuntime::from_env().resolve_executable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_runtime_explicit_wins() {
        let got = PythonRuntime::from_sources(Some("/x/python"), None, None).resolve_executable();
        assert_eq!(got, "/x/python");
    }

    #[test]
    fn python_runtime_venv_used_when_present() {
        let tmp = tempfile::tempdir().expect("tmp");
        let sub = if cfg!(windows) {
            tmp.path().join("Scripts")
        } else {
            tmp.path().join("bin")
        };
        std::fs::create_dir_all(&sub).expect("mkdir");
        let py = if cfg!(windows) {
            sub.join("python.exe")
        } else {
            sub.join("python3.12")
        };
        std::fs::write(&py, b"").expect("touch");

        let got = PythonRuntime::from_sources(None, Some(tmp.path()), None).resolve_executable();
        assert_eq!(got, py.to_string_lossy().to_string());
    }

    #[test]
    fn python_runtime_fallback_uses_supported_baseline() {
        let got = PythonRuntime::from_sources(None, None, None).resolve_executable();
        if cfg!(windows) {
            assert_eq!(got, "python");
        } else {
            assert_eq!(got, "python3.12");
        }
    }
}
