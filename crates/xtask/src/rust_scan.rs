//! Shared Rust-source scanning primitives for xtask audit modules.
//!
//! Both `wide_struct_audit` and `dead_variant_audit` walk the workspace's
//! `.rs` files and parse declarations using brace-depth tracking. Those
//! primitives live here so they can evolve in one place; the audits
//! consume them via `pub(crate)` imports.
//!
//! Why brace-counting and not `syn`: the rest of the workspace's
//! tooling (xtask, spec/tools, spec/runtime-tools) deliberately stays at
//! brace-counting — see the wide-struct audit for precedent. Pulling
//! `syn` into xtask would balloon compile time for a marginal precision
//! win on declarations that are well-behaved by convention.

use std::path::{Path, PathBuf};

/// The conventional `.rs`-bearing roots of the workspace. Other tools
/// in xtask (`wide_struct_audit`) use this same list.
pub fn rust_scan_roots(root: &Path) -> Vec<PathBuf> {
    ["src", "crates", "tests", "spec/tools", "examples", "fuzz"]
        .iter()
        .map(|relative| root.join(relative))
        .collect()
}

/// Recursively enumerate `.rs` files under `dir`, skipping `.git`,
/// `target`, `grammar` (CHAT grammar / generated parser), and Python
/// `__pycache__` dirs.
pub fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if !matches!(name, ".git" | "target" | "grammar" | "__pycache__") {
                    result.extend(walkdir(&path));
                }
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                result.push(path);
            }
        }
    }
    result
}

/// Net change in `{`/`}` count on one line. Positive = opens more than
/// it closes; negative = closes more than it opens. Brace-counting is
/// fragile inside string literals, but the convention across this
/// workspace's `pub enum` and `pub struct` declarations is well-behaved.
pub fn brace_delta(line: &str) -> isize {
    line.chars().fold(0isize, |delta, ch| match ch {
        '{' => delta + 1,
        '}' => delta - 1,
        _ => delta,
    })
}

/// True for paths that look like test/dev-tool code: tests
/// directories (`tests/`, inline `tests.rs`, `test_*.rs`,
/// `*_tests.rs`), bench harnesses (`benches/`), and runnable
/// examples (`examples/`). Both audits exclude these — production
/// code should construct production variants; dev tooling is
/// exempt from the no-panic rule by convention (examples and benches
/// can `unwrap` on fixture data; tests use assertion macros).
///
/// The bare filename `tests.rs` is the canonical 2018+ form for
/// declaring an inline `mod tests;` test module from `mod.rs` —
/// without it, the panic-audit misclassifies every
/// `<crate>/src/.../tests.rs` as production code.
pub fn is_test_path(relative_path: &str) -> bool {
    relative_path.contains("/tests/")
        || relative_path.starts_with("tests/")
        || relative_path.contains("/benches/")
        || relative_path.starts_with("benches/")
        || relative_path.contains("/examples/")
        || relative_path.starts_with("examples/")
        || relative_path
            .rsplit('/')
            .next()
            .map(|file| {
                file.starts_with("test_")
                    || file.ends_with("_tests.rs")
                    || file == "tests.rs"
                    // build.rs runs at build time, not runtime —
                    // panics here fail `cargo build`, which is the
                    // intended behaviour. Exempt from the
                    // no-panic-in-runtime-code rule.
                    || file == "build.rs"
            })
            .unwrap_or(false)
}
