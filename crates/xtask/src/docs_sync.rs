//! CI guardrails for public docs that describe the `chatter` surface.
//!
//! Checks that public-facing documentation does not reference removed CLI
//! flags, retired commands, or maintainer-local paths.

use std::path::{Path, PathBuf};

use crate::Result;

const SCAN_PATHS: &[&str] = &[
    "README.md",
    "book/src/user-guide",
    "book/src/integrating",
    "crates/chatter/chatter-cli/README.md",
];

const SKIP_DIRS: &[&str] = &[".git", "target"];

struct BannedPattern {
    pattern: &'static str,
    reason: &'static str,
}

const BANNED_PATTERNS: &[BannedPattern] = &[
    BannedPattern {
        pattern: "--fail-fast",
        reason: "removed validate flag; use --max-errors",
    },
    BannedPattern {
        pattern: "chatter analyze",
        reason: "removed top-level command; use `chatter clan ...`",
    },
    BannedPattern {
        pattern: "talkbank-private",
        reason: "private archive path should not appear in public user/integrator docs",
    },
    BannedPattern {
        pattern: "~/java-chatter",
        reason: "maintainer-local historical path",
    },
    BannedPattern {
        pattern: "~/OSX-CLAN",
        reason: "maintainer-local historical path",
    },
];

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !SKIP_DIRS.iter().any(|skip| *skip == name) {
                    result.extend(walkdir(&path));
                }
            } else {
                result.push(path);
            }
        }
    }
    result
}

fn scan_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for rel in SCAN_PATHS {
        let path = root.join(rel);
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            files.extend(walkdir(&path));
        }
    }
    files.sort();
    files
}

pub fn run(root: &Path) -> Result<()> {
    let mut failures = Vec::new();

    for path in scan_files(root) {
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };

        for (line_no, line) in text.lines().enumerate() {
            for banned in BANNED_PATTERNS {
                if line.contains(banned.pattern) {
                    failures.push(format!(
                        "{rel}:{}: `{}` ({})\n  {}",
                        line_no + 1,
                        banned.pattern,
                        banned.reason,
                        line.trim()
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        println!("docs sync: OK");
        Ok(())
    } else {
        Err(format!(
            "public docs contain stale or private references:\n{}",
            failures.join("\n\n")
        )
        .into())
    }
}
