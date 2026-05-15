//! Shared helper for resolving the current executable path.
//!
//! The preferred source of truth is `BATCHALIGN_SELF_EXE`, which the Python
//! wrapper sets before it `exec`s the packaged Rust binary under
//! `uv tool install`.
//!
//! When installed via `uv tool install`, `std::env::current_exe()` returns the
//! Python interpreter (e.g. `.../python3.12`) because the `batchalign3` command
//! is a console_scripts wrapper. Spawning `python3.12 serve start --foreground`
//! fails because Python tries to run a script named "serve".
//!
//! Detection: if `current_exe()` filename starts with "python", fall back to
//! bare `"batchalign3"` which `Command::new()` resolves via PATH lookup.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const SELF_EXE_ENV: &str = "BATCHALIGN_SELF_EXE";

/// Resolve the executable path for spawning a background server or daemon.
pub(crate) fn resolve_self_exe() -> PathBuf {
    if let Some(explicit) = resolve_self_exe_env(std::env::var_os(SELF_EXE_ENV)) {
        return explicit;
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("batchalign3"));
    resolve_self_exe_from(&exe)
}

fn resolve_self_exe_env(value: Option<OsString>) -> Option<PathBuf> {
    match value {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => None,
    }
}

/// Testable core of [`resolve_self_exe`].
pub(crate) fn resolve_self_exe_from(current_exe: &Path) -> PathBuf {
    let file_name = current_exe
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if file_name.starts_with("python") || file_name.starts_with("Python") {
        // current_exe is a Python interpreter — we're running through
        // console_scripts. Use PATH lookup instead.
        PathBuf::from("batchalign3")
    } else {
        current_exe.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_self_exe_uses_explicit_env_override() {
        let env = resolve_self_exe_env(Some(OsString::from("/tmp/batchalign3")));
        assert_eq!(env, Some(PathBuf::from("/tmp/batchalign3")));
    }

    #[test]
    fn resolve_self_exe_ignores_empty_env_override() {
        assert_eq!(resolve_self_exe_env(Some(OsString::new())), None);
    }

    #[test]
    fn resolve_self_exe_detects_python_interpreter() {
        let python =
            PathBuf::from("/Users/operator/.local/share/uv/tools/batchalign3/bin/python3.12");
        assert_eq!(resolve_self_exe_from(&python), PathBuf::from("batchalign3"));
    }

    #[test]
    fn resolve_self_exe_detects_python_no_version() {
        let python = PathBuf::from("/usr/bin/python3");
        assert_eq!(resolve_self_exe_from(&python), PathBuf::from("batchalign3"));
    }

    #[test]
    fn resolve_self_exe_keeps_native_binary() {
        let native = PathBuf::from("/usr/local/bin/batchalign3");
        assert_eq!(resolve_self_exe_from(&native), native);
    }

    #[test]
    fn resolve_self_exe_keeps_debug_binary() {
        let debug = PathBuf::from("/Users/operator/batchalign3/target/debug/batchalign3");
        assert_eq!(resolve_self_exe_from(&debug), debug);
    }
}
