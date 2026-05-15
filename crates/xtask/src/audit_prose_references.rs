//! Layer 1 CI gate: prose references to deleted crates / moved book paths.
//!
//! Walks every markdown file under the repo root, applies the
//! high-severity patterns from `audit_docs::FLAG_PATTERNS`, and exits
//! non-zero if any non-allow-listed hit is found. Catalog-independent:
//! does not touch the `audit.db` in the meta-repo, so it runs cleanly
//! in talkbank-tools CI where that database is not checked in.
//!
//! The audit method calls this "Layer 1 mechanical CI gate". See
//! `<workspace>/docs/release-doc-audit/audit-method.md`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::WalkDir;

use crate::audit_docs::FLAG_PATTERNS;

/// One historical-content allow-list entry. A hit on `pattern_name`
/// inside `path` is treated as expected and does not fail the gate.
///
/// Only add an entry here when the doc's subject is *the removed
/// dependency itself* (e.g. a "What was removed" section, a release
/// changelog, a panic-audit snapshot). Never allow-list a current-state
/// doc that happens to lag a rename — fix that doc instead.
struct AllowEntry {
    /// Repo-relative path to a single markdown file, forward slashes.
    path: &'static str,
    /// Pattern name (matches `FLAG_PATTERNS.name`).
    pattern_name: &'static str,
    /// Short justification recorded inline for future readers.
    #[allow(dead_code)]
    rationale: &'static str,
}

/// Historical surfaces that legitimately name deleted crates / moved
/// paths. Reviewed 2026-05-11 against `flag-staleness` output after the
/// `overview.md` Crate Dependency Graph fix.
const ALLOW_LIST: &[AllowEntry] = &[AllowEntry {
    path: "book/src/batchalign/developer/maturin-pyo3-surface.md",
    pattern_name: "deleted-crate-batchalign-revai",
    rationale: "The '### What was removed' table documents \
                    dependencies dropped from the slim PyO3 surface; \
                    naming `batchalign-revai` is the subject of the table.",
}];

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) pattern_name: &'static str,
    pub(crate) excerpt: String,
}

pub fn run(repo_root: &Path) -> crate::Result<()> {
    let docs = collect_markdown(repo_root);
    let total_files = docs.len();
    let violations = scan_docs(&docs)?;

    if violations.is_empty() {
        println!(
            "xtask audit-prose-references: 0 violations across {} markdown files \
             ({} allow-listed historical surfaces respected)",
            total_files,
            ALLOW_LIST.len()
        );
        return Ok(());
    }

    eprintln!(
        "xtask audit-prose-references: {} violation(s) — prose names a deleted crate or moved book path",
        violations.len()
    );
    for v in &violations {
        eprintln!(
            "  {}:{}  [{}]  {}",
            v.path, v.line, v.pattern_name, v.excerpt
        );
    }
    eprintln!();
    eprintln!(
        "Fix the prose to match current code, or — if the doc legitimately \
         describes historical state — add an entry to ALLOW_LIST in \
         crates/xtask/src/audit_prose_references.rs with a rationale."
    );

    Err(format!("audit-prose-references: {} violation(s)", violations.len()).into())
}

/// Scan a set of `(rel_path, content)` pairs for high-severity prose-
/// reference hits. Deduplicated by `(path, line, pattern)` so a single
/// line referenced by overlapping section ranges is reported once.
pub(crate) fn scan_docs(docs: &[(String, String)]) -> crate::Result<Vec<Violation>> {
    let high_severity: Vec<(&'static str, Regex)> = FLAG_PATTERNS
        .iter()
        .filter(|p| p.severity == "high")
        .map(|p| Regex::new(p.pattern).map(|re| (p.name, re)))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| -> crate::DynError { Box::new(e) })?;

    let mut violations: Vec<Violation> = Vec::new();
    let mut seen: BTreeSet<(String, usize, &'static str)> = BTreeSet::new();

    for (rel, content) in docs {
        for (line_idx, line) in content.lines().enumerate() {
            let line_no = line_idx + 1;
            for (name, regex) in &high_severity {
                if let Some(m) = regex.find(line) {
                    let key = (rel.clone(), line_no, *name);
                    if !seen.insert(key) {
                        continue;
                    }
                    if is_allow_listed(rel, name) {
                        continue;
                    }
                    violations.push(Violation {
                        path: rel.clone(),
                        line: line_no,
                        pattern_name: name,
                        excerpt: m.as_str().to_owned(),
                    });
                }
            }
        }
    }

    violations.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.pattern_name.cmp(b.pattern_name))
    });
    Ok(violations)
}

fn is_allow_listed(path: &str, pattern_name: &str) -> bool {
    ALLOW_LIST
        .iter()
        .any(|entry| entry.path == path && entry.pattern_name == pattern_name)
}

fn collect_markdown(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.file_name().to_string_lossy().as_ref()))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel: PathBuf = match path.strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        out.push((rel_str, content));
    }
    out
}

fn is_excluded(name: &str) -> bool {
    matches!(name, "target" | "node_modules" | "build" | ".git") || name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::{Violation, scan_docs};

    fn doc(path: &str, body: &str) -> (String, String) {
        (path.to_owned(), body.to_owned())
    }

    #[test]
    fn clean_tree_passes() {
        let docs = vec![
            doc("README.md", "# Hello\n\nNothing stale here.\n"),
            doc("book/src/foo.md", "Some prose about batchalign.\n"),
        ];
        let v = scan_docs(&docs).unwrap();
        assert!(v.is_empty(), "expected no violations, got {v:?}");
    }

    #[test]
    fn deleted_crate_in_current_doc_is_flagged() {
        let docs = vec![doc(
            "book/src/arch.md",
            "We use `batchalign-app` for the server.\n",
        )];
        let v = scan_docs(&docs).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pattern_name, "deleted-crate-batchalign-app");
        assert_eq!(v[0].line, 1);
        assert_eq!(v[0].path, "book/src/arch.md");
    }

    #[test]
    fn allow_listed_surface_is_silent() {
        let docs = vec![doc(
            "book/src/batchalign/developer/maturin-pyo3-surface.md",
            "| `batchalign-revai` | Dead code — server uses Rev.AI directly |\n",
        )];
        let v = scan_docs(&docs).unwrap();
        assert!(v.is_empty(), "expected allow-list to suppress, got {v:?}");
    }

    #[test]
    fn allow_list_is_scoped_to_specific_pattern() {
        // The maturin-pyo3-surface allow-list entry covers batchalign-revai
        // ONLY — a hit for batchalign-app on the same path must still fail.
        let docs = vec![doc(
            "book/src/batchalign/developer/maturin-pyo3-surface.md",
            "Used to depend on `batchalign-app`.\n",
        )];
        let v = scan_docs(&docs).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pattern_name, "deleted-crate-batchalign-app");
    }

    #[test]
    fn same_line_counted_once() {
        let docs = vec![doc(
            "book/src/x.md",
            "## Parent\n\nbatchalign-app exists here\n\n### Child\n\nUnrelated\n",
        )];
        let v: Vec<Violation> = scan_docs(&docs).unwrap();
        assert_eq!(v.len(), 1, "got {v:?}");
        assert_eq!(v[0].line, 3);
    }

    #[test]
    fn multiple_files_report_in_sorted_order() {
        let docs = vec![
            doc("z/last.md", "batchalign-app\n"),
            doc("a/first.md", "batchalign-revai\n"),
        ];
        let v = scan_docs(&docs).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].path, "a/first.md");
        assert_eq!(v[1].path, "z/last.md");
    }
}
