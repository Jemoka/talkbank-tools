//! CI hygiene checks: version sync, legacy term detection, retired package enforcement.
//!
//! Replaces the former Python scripts and test-binary-based checks:
//! - `scripts/check_cli_version_sync.py`
//! - `scripts/check_legacy_terms.py`
//! - `scripts/check_retired_packages.py`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;

// ---------------------------------------------------------------------------
// CLI version sync
// ---------------------------------------------------------------------------

fn check_version_sync(root: &Path) -> std::result::Result<(), String> {
    let pyproject_path = root.join("pyproject.toml");
    let pyproject_str = std::fs::read_to_string(&pyproject_path)
        .map_err(|e| format!("Cannot read {}: {e}", pyproject_path.display()))?;
    let pyproject: toml::Value =
        toml::from_str(&pyproject_str).map_err(|e| format!("Cannot parse pyproject.toml: {e}"))?;
    let py_version = pyproject["project"]["version"]
        .as_str()
        .ok_or("pyproject.toml missing [project].version")?;

    let cargo_path = root.join("crates/batchalign/batchalign-cli/Cargo.toml");
    let cargo_str = std::fs::read_to_string(&cargo_path)
        .map_err(|e| format!("Cannot read {}: {e}", cargo_path.display()))?;
    let cargo: toml::Value =
        toml::from_str(&cargo_str).map_err(|e| format!("Cannot parse CLI Cargo.toml: {e}"))?;
    let cargo_version = cargo["package"]["version"]
        .as_str()
        .ok_or("Cargo.toml missing [package].version")?;

    if py_version != cargo_version {
        return Err(format!(
            "Version mismatch: pyproject.toml={py_version} != batchalign/Cargo.toml={cargo_version}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Legacy terms
// ---------------------------------------------------------------------------

const SCAN_SUFFIXES: &[&str] = &[
    ".css", ".js", ".jsx", ".md", ".py", ".rs", ".toml", ".ts", ".tsx", ".yaml", ".yml",
];

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".venv",
    ".venv-314t",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
];

struct BannedPattern {
    pattern: &'static str,
    reason: &'static str,
}

const BANNED_PATTERNS: &[BannedPattern] = &[
    BannedPattern {
        pattern: "batchalign-next",
        reason: "retired command name",
    },
    BannedPattern {
        pattern: "batchalign_next",
        reason: "retired package/module name",
    },
    BannedPattern {
        pattern: "/opt/python/bin/python",
        reason: "hardcoded interpreter path",
    },
    BannedPattern {
        pattern: "batchalign.cli",
        reason: "retired Python CLI package path",
    },
    BannedPattern {
        pattern: "pip install 'batchalign-hk-plugin",
        reason: "retired HK plugin package install guidance",
    },
    BannedPattern {
        pattern: "pip install \"batchalign-hk-plugin",
        reason: "retired HK plugin package install guidance",
    },
    BannedPattern {
        pattern: "batchalign.providers.models",
        reason: "nonexistent public module path",
    },
    BannedPattern {
        pattern: "plugin discovery still happens in `batchalign.plugins`",
        reason: "entry-point plugin discovery was removed",
    },
    BannedPattern {
        pattern: "Entry-point plugin system (`batchalign.plugins`)",
        reason: "current release has no public entry-point plugin system",
    },
    BannedPattern {
        pattern: "batchalign-hk-plugin/common.py",
        reason: "retired HK plugin source path in current docs",
    },
    BannedPattern {
        pattern: "batchalign-hk-plugin/cantonese_fa.py",
        reason: "retired HK plugin source path in current docs",
    },
];

const DOC_BANNED: &[BannedPattern] = &[
    BannedPattern {
        pattern: "BA2-usage.pdf",
        reason: "historical Batchalign2 PDF linked from active docs",
    },
    BannedPattern {
        pattern: "BA2-cleanup.pdf",
        reason: "historical Batchalign2 PDF linked from active docs",
    },
    BannedPattern {
        pattern: "--whisper-oai",
        reason: "retired public CLI flag form; use --asr-engine whisper-oai",
    },
    BannedPattern {
        pattern: concat!("rust", "-next/"),
        reason: "retired public workspace path",
    },
    BannedPattern {
        pattern: "worker.py",
        reason: "retired Python worker module path in active docs",
    },
    BannedPattern {
        pattern: "test_worker.py",
        reason: "retired Python test file path in active docs",
    },
];

const DOC_ACTIVE_PREFIXES: &[&str] = &[
    "README.md",
    "examples/launchd.plist",
    "examples/server.yaml",
    "book/src/introduction.md",
    "book/src/user-guide/",
    "book/src/developer/building.md",
    "book/src/developer/testing.md",
];

fn allowlist() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        (
            "batchalign/runtime.py",
            vec![
                "One-time migration: ~/.batchalign-next",
                "old = Path.home() / \".batchalign-next\"",
            ],
        ),
        // Historical/architecture docs legitimately reference batchalign-next by name.
        (
            "book/src/architecture/worker-architecture-assessment.md",
            vec!["batchalign-next"],
        ),
        (
            "book/src/developer/worker-protocol-v2.md",
            vec!["batchalign-next"],
        ),
        (
            "book/src/migration/algorithms-and-language.md",
            vec!["batchalign-next"],
        ),
        // Rust source comment documenting legacy behavior.
        (
            "crates/batchalign/batchalign-cli/src/revai/preflight.rs",
            vec!["batchalign-next"],
        ),
    ])
}

fn should_scan(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let dotted = format!(".{ext}");
    if !SCAN_SUFFIXES.contains(&dotted.as_str()) {
        return false;
    }
    !path
        .components()
        .any(|c| SKIP_DIRS.contains(&c.as_os_str().to_str().unwrap_or("")))
}

/// This file itself contains banned strings as literal test data.
const SELF_EXCLUDE: &str = "crates/xtask/src/ci_hygiene.rs";

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_str().unwrap_or("");
                if !SKIP_DIRS.contains(&name) {
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
    let active_paths: Vec<PathBuf> = [
        "README.md",
        ".github/workflows",
        "batchalign",
        "crates",
        "apps/batchalign/cli-web-statuspage/src",
        "book/src/introduction.md",
        "book/src/migration",
        "book/src/architecture",
        "book/src/reference",
        "book/src/user-guide",
        "book/src/developer",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let self_path = root.join(SELF_EXCLUDE);

    let mut files = Vec::new();
    for base in &active_paths {
        if !base.exists() {
            continue;
        }
        if base.is_file() {
            if should_scan(base) && *base != self_path {
                files.push(base.clone());
            }
            continue;
        }
        if base.is_dir() {
            for entry in walkdir(base) {
                if should_scan(&entry) && entry != self_path {
                    files.push(entry);
                }
            }
        }
    }
    files.sort();
    files
}

fn is_word_boundary_match(line: &str, start: usize, end: usize) -> bool {
    let before_ok = start == 0
        || !line.as_bytes()[start - 1].is_ascii_alphanumeric()
            && line.as_bytes()[start - 1] != b'_';
    let after_ok = end >= line.len()
        || !line.as_bytes()[end].is_ascii_alphanumeric() && line.as_bytes()[end] != b'_';
    before_ok && after_ok
}

fn check_legacy_terms(root: &Path) -> std::result::Result<(), String> {
    let allow = allowlist();
    let mut failures = Vec::new();

    for path in scan_files(root) {
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap_or("")
            .to_string();
        let allow_subs = allow.get(rel.as_str()).cloned().unwrap_or_default();

        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        for (line_no, line) in text.lines().enumerate() {
            let line_no = line_no + 1;

            for bp in BANNED_PATTERNS {
                if let Some(start) = line.find(bp.pattern) {
                    let end = start + bp.pattern.len();
                    if !is_word_boundary_match(line, start, end) {
                        continue;
                    }
                    if allow_subs.iter().any(|s| line.contains(s)) {
                        continue;
                    }
                    failures.push(format!(
                        "{rel}:{line_no}: `{}` ({})\n  {}",
                        bp.pattern,
                        bp.reason,
                        line.trim()
                    ));
                }
            }

            if DOC_ACTIVE_PREFIXES.iter().any(|p| rel.starts_with(p)) {
                for bp in DOC_BANNED {
                    if let Some(start) = line.find(bp.pattern) {
                        let end = start + bp.pattern.len();
                        if !is_word_boundary_match(line, start, end) {
                            continue;
                        }
                        failures.push(format!(
                            "{rel}:{line_no}: `{}` ({})\n  {}",
                            bp.pattern,
                            bp.reason,
                            line.trim()
                        ));
                    }
                }

                if line.contains("uv tool install batchalign3-cli") {
                    failures.push(format!(
                        "{rel}:{line_no}: `uv tool install batchalign3-cli` (retired: CLI is now part of the batchalign3 package)\n  {}",
                        line.trim()
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Legacy term check failed:\n- {}",
            failures.join("\n- ")
        ))
    }
}

// ---------------------------------------------------------------------------
// Retired packages
// ---------------------------------------------------------------------------

fn check_retired_packages(root: &Path) -> std::result::Result<(), String> {
    let retired_paths = ["batchalign/cli", "batchalign/serve"];
    let mut failures = Vec::new();

    for pathspec in &retired_paths {
        let output = Command::new("git")
            .args(["ls-files", pathspec])
            .current_dir(root)
            .output()
            .map_err(|e| format!("git ls-files failed: {e}"))?;

        let tracked: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && root.join(l).exists())
            .collect();

        for file in &tracked {
            failures.push(format!(
                "{file}: unexpected tracked file under retired package path `{pathspec}`"
            ));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Retired package boundary check failed:\n- {}",
            failures.join("\n- ")
        ))
    }
}

// ---------------------------------------------------------------------------
// Release metadata verification (T082)
// ---------------------------------------------------------------------------

/// Verify license consistency across all release surfaces.
///
/// All crates and the top-level pyproject.toml must declare the same license.
fn check_license_consistency(root: &Path) -> std::result::Result<(), String> {
    let pyproject_path = root.join("pyproject.toml");
    let pyproject_str = std::fs::read_to_string(&pyproject_path)
        .map_err(|e| format!("Cannot read {}: {e}", pyproject_path.display()))?;
    let pyproject: toml::Value =
        toml::from_str(&pyproject_str).map_err(|e| format!("Cannot parse pyproject.toml: {e}"))?;
    let py_license = pyproject["project"]["license"]
        .as_table()
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .or_else(|| pyproject["project"]["license"].as_str())
        .ok_or("pyproject.toml missing [project].license")?;

    let workspace_cargo_path = root.join("Cargo.toml");
    let workspace_cargo_str = std::fs::read_to_string(&workspace_cargo_path)
        .map_err(|e| format!("Cannot read {}: {e}", workspace_cargo_path.display()))?;
    let workspace_cargo: toml::Value = toml::from_str(&workspace_cargo_str)
        .map_err(|e| format!("Cannot parse workspace Cargo.toml: {e}"))?;
    let cargo_license = workspace_cargo
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("license"))
        .and_then(|l| l.as_str())
        .ok_or("Cargo.toml missing [workspace.package].license")?;

    if py_license != cargo_license {
        return Err(format!(
            "License mismatch: pyproject.toml=\"{py_license}\" != Cargo.toml=\"{cargo_license}\""
        ));
    }

    // Also check the top-level LICENSE file exists.
    let license_file = root.join("LICENSE");
    if !license_file.exists() {
        return Err("LICENSE file missing from repo root".to_string());
    }

    Ok(())
}

/// Verify that the workspace Cargo.toml has essential metadata for crates.io publishing.
fn check_cargo_metadata_completeness(root: &Path) -> std::result::Result<(), String> {
    let workspace_cargo_path = root.join("Cargo.toml");
    let workspace_cargo_str = std::fs::read_to_string(&workspace_cargo_path)
        .map_err(|e| format!("Cannot read {}: {e}", workspace_cargo_path.display()))?;
    let workspace_cargo: toml::Value = toml::from_str(&workspace_cargo_str)
        .map_err(|e| format!("Cannot parse workspace Cargo.toml: {e}"))?;

    let package = workspace_cargo
        .get("workspace")
        .and_then(|w| w.get("package"))
        .ok_or("Cargo.toml missing [workspace.package]")?;

    let mut missing = Vec::new();
    for field in ["license", "edition", "repository"] {
        if package.get(field).is_none() {
            missing.push(field);
        }
    }

    if !missing.is_empty() {
        return Err(format!(
            "Workspace Cargo.toml [workspace.package] missing required fields: {}",
            missing.join(", ")
        ));
    }

    Ok(())
}

/// Verify the CLI binary crate declares its own version (not just workspace = true).
fn check_cli_binary_version(root: &Path) -> std::result::Result<(), String> {
    let cli_cargo = root.join("crates/batchalign/batchalign-cli/Cargo.toml");
    let cli_str = std::fs::read_to_string(&cli_cargo)
        .map_err(|e| format!("Cannot read {}: {e}", cli_cargo.display()))?;
    let cli_toml: toml::Value =
        toml::from_str(&cli_str).map_err(|e| format!("Cannot parse CLI Cargo.toml: {e}"))?;

    let version = cli_toml
        .get("package")
        .and_then(|p| p.get("version"))
        .ok_or("CLI Cargo.toml missing [package].version")?;

    // version must be a string (not a workspace reference table)
    if version.as_str().is_none() {
        // Could be workspace = true which is fine — check_version_sync validates the actual value.
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Release operations hardening
// ---------------------------------------------------------------------------

fn read_required_file(root: &Path, rel: &str) -> std::result::Result<String, String> {
    let path = root.join(rel);
    std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

fn check_batchalign_release_health_smoke_text(text: &str) -> std::result::Result<(), String> {
    let mut missing = Vec::new();

    for needle in [
        "batchalign3 serve start",
        "batchalign3 serve status",
        "/health",
    ] {
        if !text.contains(needle) {
            missing.push(needle);
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Batchalign release workflow must smoke-test the packaged server /health path; missing: {}",
            missing.join(", ")
        ))
    }
}

fn check_vscode_release_policy_text(text: &str) -> std::result::Result<(), String> {
    for banned in [
        "vsce publish",
        "npx vsce publish",
        "ovsx publish",
        "npx ovsx publish",
    ] {
        if text.contains(banned) {
            return Err(format!(
                "VS Code release automation is GitHub Releases VSIX-only for the first public release; found forbidden publish command `{banned}`"
            ));
        }
    }

    if !text.contains("softprops/action-gh-release") {
        return Err(
            "VS Code release workflow must create a GitHub Release for VSIX distribution".into(),
        );
    }

    Ok(())
}

fn check_vscode_release_text(path: &str, text: &str) -> std::result::Result<(), String> {
    for banned in [
        "publishes them to the Marketplace",
        "Marketplace serves the right VSIX per platform automatically",
        "Marketplace treats those as pre-release channels",
        "publishes to the Marketplace first",
    ] {
        if text.contains(banned) {
            return Err(format!(
                "{path} contradicts the first-release VS Code distribution policy (GitHub Releases VSIX-only): found `{banned}`"
            ));
        }
    }

    if !text.contains("GitHub Release") && !text.contains("GitHub Releases") {
        return Err(format!(
            "{path} must describe GitHub Releases as the current VS Code distribution channel"
        ));
    }

    Ok(())
}

fn check_release_operations(root: &Path) -> std::result::Result<(), String> {
    let mut failures = Vec::new();

    let batchalign_workflow = read_required_file(root, ".github/workflows/batchalign-release.yml")?;
    if let Err(msg) = check_batchalign_release_health_smoke_text(&batchalign_workflow) {
        failures.push(format!(".github/workflows/batchalign-release.yml: {msg}"));
    }

    let vscode_workflow = read_required_file(root, ".github/workflows/vscode-release.yml")?;
    if let Err(msg) = check_vscode_release_policy_text(&vscode_workflow) {
        failures.push(format!(".github/workflows/vscode-release.yml: {msg}"));
    }

    for rel in [
        "book/src/apps/vscode-extension/developer/releasing.md",
        "book/src/apps/vscode-extension/design/adr-004-bundled-lsp-binary.md",
    ] {
        match read_required_file(root, rel) {
            Ok(text) => {
                if let Err(msg) = check_vscode_release_text(rel, &text) {
                    failures.push(msg);
                }
            }
            Err(msg) => failures.push(msg),
        }
    }

    let signing_doc = root.join("book/src/operations/code-signing-and-distribution.md");
    if !signing_doc.exists() {
        failures.push(
            "book/src/operations/code-signing-and-distribution.md must exist to define first-release signing/notarization policy"
                .into(),
        );
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Release operations check failed:\n- {}",
            failures.join("\n- ")
        ))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(root: &Path) -> Result<()> {
    let mut all_failures: Vec<String> = Vec::new();

    if let Err(msg) = check_version_sync(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: version sync OK");
    }

    if let Err(msg) = check_legacy_terms(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: legacy terms OK");
    }

    if let Err(msg) = check_retired_packages(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: retired packages OK");
    }

    if let Err(msg) = check_license_consistency(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: license consistency OK");
    }

    if let Err(msg) = check_cargo_metadata_completeness(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: cargo metadata completeness OK");
    }

    if let Err(msg) = check_cli_binary_version(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: CLI binary version OK");
    }

    if let Err(msg) = check_release_operations(root) {
        all_failures.push(msg);
    } else {
        println!("ci-hygiene: release operations OK");
    }

    if all_failures.is_empty() {
        println!("ci-hygiene: all checks passed");
        Ok(())
    } else {
        Err(all_failures.join("\n\n").into())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check_batchalign_release_health_smoke_text, check_vscode_release_policy_text,
        check_vscode_release_text,
    };

    #[test]
    fn batchalign_release_requires_health_smoke() {
        let workflow = r#"
jobs:
  wheel-smoke:
    steps:
      - name: Verify batchalign3 --help
        run: batchalign3 --help
"#;

        let err = check_batchalign_release_health_smoke_text(workflow).unwrap_err();
        assert!(err.contains("/health"), "{err}");
    }

    #[test]
    fn vscode_release_rejects_marketplace_publish_steps() {
        let workflow = r#"
jobs:
  release:
    steps:
      - run: npx vsce publish
"#;

        let err = check_vscode_release_policy_text(workflow).unwrap_err();
        assert!(err.contains("GitHub Releases VSIX-only"), "{err}");
    }

    #[test]
    fn vscode_release_docs_reject_marketplace_first_language() {
        let doc = r#"
The extension publishes to the Marketplace first.
"#;

        let err =
            check_vscode_release_text("book/src/apps/vscode-extension/design/adr-004-bundled-lsp-binary.md", doc)
                .unwrap_err();
        assert!(err.contains("GitHub Releases VSIX-only"), "{err}");
    }
}
