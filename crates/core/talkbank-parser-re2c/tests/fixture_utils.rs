//! Utilities for loading test fixtures.

// Each test binary that says `mod fixture_utils;` compiles this module
// independently and warns on any helper it doesn't use — matching the
// `tests/cli_common/mod.rs` convention in the merged batchalign crate.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Workspace root, located by walking up from `CARGO_MANIFEST_DIR` to the
/// nearest ancestor whose `Cargo.toml` declares `[workspace]`.
///
/// This replaces the rename-fragile pattern callers used to hard-code:
/// `env!("CARGO_MANIFEST_DIR").replace("/crates/<crate>", "")`, which
/// silently produced a wrong path the moment a crate was renamed (the P7
/// `batchalign-app`/`batchalign-cli` consolidation almost paid that bill).
///
/// We check the `[workspace]` table explicitly rather than just
/// `Cargo.lock` because stale orphan lockfiles inside sub-crates DO
/// happen — `crates/clan-core/Cargo.lock` and
/// `apps/batchalign/dashboard-desktop/src-tauri/Cargo.lock` were both observed as
/// orphans on 2026-04-30, and a `Cargo.lock`-only walk-up silently
/// returned the wrong path. Reading the Cargo.toml content is slightly
/// slower (one stat + one small read per ancestor) but rejects orphans
/// by construction.
/// Mirrored in `clan-core/tests/common/mod.rs::workspace_root`.
pub fn workspace_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
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
            "workspace_root(): no Cargo.toml with [workspace] found above \
             {} — was the crate relocated outside its workspace?",
            start.display()
        );
    })
}

/// Meta-repository root (the parent of [`workspace_root`]), where the
/// out-of-workspace `data/` corpora live.
///
/// Used by the two corpus-divergence tests (`quick_divergence_check.rs`,
/// `lexer_tests.rs`) that read CHAT files from `data/aphasia-data/`,
/// `data/biling-data/`, etc. — a tree that lives one level above
/// `talkbank-tools/` in the unified operator workspace and is also the
/// canonical layout for deployments (`~/0tb/data/...`).
pub fn meta_repo_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        workspace_root()
            .parent()
            .unwrap_or_else(|| {
                panic!(
                    "meta_repo_root(): workspace_root {} has no parent",
                    workspace_root().display()
                )
            })
            .to_path_buf()
    })
}

/// Load fixture lines from a file in tests/fixtures/.
/// Returns a Vec of logical CHAT lines (entries separated by blank lines).
/// Skips comment lines starting with #.
pub fn load_fixture(name: &str) -> Vec<String> {
    let path = format!("{}/tests/fixtures/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping fixture {name}: {e}");
            return vec![];
        }
    };

    let mut entries = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            if !current.is_empty() {
                entries.push(std::mem::take(&mut current));
            }
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        entries.push(current);
    }

    entries
}

/// Load fixtures and verify all lex cleanly (zero error tokens).
/// Returns the entries for further testing.
pub fn load_and_verify_lex(name: &str) -> Vec<String> {
    let entries = load_fixture(name);
    if entries.is_empty() {
        eprintln!("  {name}: no fixtures (skipped)");
        return entries;
    }
    let mut errors = 0;
    for entry in &entries {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        let result = talkbank_parser_re2c::lex(&input);
        if !result.is_clean() {
            errors += 1;
            if errors <= 3 {
                let snippet = entry.chars().take(60).collect::<String>();
                eprintln!("  LEX ERROR in {name}: {}", snippet.escape_debug());
                eprint!("{}", result.error_report(&input));
            }
        }
    }
    assert_eq!(
        errors,
        0,
        "{name}: {errors}/{} entries had lex errors",
        entries.len()
    );
    eprintln!("  {name}: {} entries, all lex clean", entries.len());
    entries
}
