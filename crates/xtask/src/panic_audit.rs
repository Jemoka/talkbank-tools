//! Catalogue every panic-producing call site in the workspace's `.rs`
//! files so the Phase B audit (see `book/src/operations/panic-audit/`) can triage them
//! crate-by-crate.
//!
//! Each site lands in one of three classifications during triage:
//! (a) infallible-by-construction — annotate with a localized
//! `#[allow(clippy::*_used)]` carrying a written rationale;
//! (b) recoverable-error-masked — replace with `Result` plumbing through
//! a `thiserror` domain error; (c) untyped-invariant — introduce a
//! newtype that makes the panic structurally impossible. Patterns
//! covered: `.unwrap()`, `.expect(...)`, `panic!`, `todo!`,
//! `unimplemented!`, `unreachable!`.
//!
//! Detection is deliberately heuristic (line-by-line, after stripping
//! line comments and whole-line block comments) following the existing
//! xtask convention of brace-counting over `syn`. False positives inside
//! string literals are accepted; they're rare and the audit's reviewer
//! filters them out by hand.
//!
//! Contributor entrypoints:
//! - `cargo run -q -p xtask -- panic-audit` for a human-readable summary
//! - `cargo run -q -p xtask -- panic-audit --json` for the machine-readable
//!   per-site catalogue, which feeds Phase B per-crate work
//! - `cargo run -q -p xtask -- panic-audit --crate <name>` to focus on
//!   one workspace member at a time

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::Result;
use crate::rust_scan::{is_test_path, rust_scan_roots, walkdir};

/// Which `panic`-producing call shape was matched. Strings are stable
/// because `--json` consumers (the Phase B per-crate scripts) key off
/// them.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PanicPattern {
    Unwrap,
    Expect,
    Panic,
    Todo,
    Unimplemented,
    Unreachable,
}

impl PanicPattern {
    fn label(self) -> &'static str {
        match self {
            Self::Unwrap => "unwrap",
            Self::Expect => "expect",
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
        }
    }

    /// All variants in stable order for table headers and totals.
    fn all() -> &'static [Self] {
        &[
            Self::Unwrap,
            Self::Expect,
            Self::Panic,
            Self::Todo,
            Self::Unimplemented,
            Self::Unreachable,
        ]
    }
}

/// Coarse directory-of-origin used for the priority-ordered audit:
/// long-lived library crates (`talkbank-core`) and the runtime app
/// surfaces (`batchalign`) carry the no-panic standard most strictly,
/// while tools (`xtask`, `experiments`) are exempt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DirectoryClass {
    TalkbankCore,
    Batchalign,
    Pyo3,
    Xtask,
    Experiments,
    Other,
}

impl DirectoryClass {
    fn label(self) -> &'static str {
        match self {
            Self::TalkbankCore => "talkbank-core",
            Self::Batchalign => "batchalign",
            Self::Pyo3 => "crates/batchalign-pyo3",
            Self::Xtask => "xtask",
            Self::Experiments => "experiments",
            Self::Other => "other",
        }
    }

    fn classify(relative_path: &str) -> Self {
        if relative_path.starts_with("crates/talkbank-") {
            Self::TalkbankCore
        } else if relative_path.starts_with("crates/batchalign-") {
            Self::Batchalign
        } else if relative_path.starts_with("crates/batchalign/batchalign-pyo3/") {
            Self::Pyo3
        } else if relative_path.starts_with("xtask/") {
            Self::Xtask
        } else if relative_path.starts_with("experiments/") {
            Self::Experiments
        } else {
            Self::Other
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::TalkbankCore,
            Self::Batchalign,
            Self::Pyo3,
            Self::Xtask,
            Self::Experiments,
            Self::Other,
        ]
    }
}

/// One panic-producing call site. Serialised verbatim into the JSON
/// catalogue.
#[derive(Clone, Debug, Serialize)]
struct PanicSite {
    /// Workspace-relative path with forward slashes.
    path: String,
    /// 1-based line number.
    line: usize,
    pattern: PanicPattern,
    class: DirectoryClass,
    /// `true` when the site lives in test-classified code per
    /// [`is_test_path`]. Inline `#[cfg(test)] mod tests { ... }` blocks
    /// are *not* detected — they remain in `is_test=false` and must be
    /// filtered by the per-crate audit. Documented limitation.
    is_test: bool,
    /// The line text, trimmed of leading/trailing whitespace, for
    /// human triage.
    excerpt: String,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    total: usize,
    by_pattern: BTreeMap<&'static str, usize>,
    by_class_non_test: BTreeMap<&'static str, usize>,
    by_class_test: BTreeMap<&'static str, usize>,
    by_crate_non_test: BTreeMap<String, usize>,
    sites: Vec<PanicSite>,
}

/// Run modes selected from the CLI.
#[derive(Clone, Debug, Default)]
struct Options {
    /// Emit the full per-site catalogue as JSON instead of a summary.
    json: bool,
    /// Restrict scanning to files whose workspace-relative path begins
    /// with this crate prefix (e.g. `crates/chatter-lsp`). Empty means
    /// scan the full workspace.
    crate_filter: Option<String>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = Options::default();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--json" => opts.json = true,
                "--crate" => {
                    let value = iter.next().ok_or_else(|| {
                        "panic-audit: --crate requires a workspace-relative path prefix".to_string()
                    })?;
                    opts.crate_filter = Some(value.clone());
                }
                other => {
                    return Err(format!("panic-audit: unrecognized argument: {other}").into());
                }
            }
        }
        Ok(opts)
    }
}

pub fn run(root: &Path, args: Vec<String>) -> Result<()> {
    let opts = Options::parse(&args)?;
    let report = build_report(root, &opts)?;

    if opts.json {
        let body = serde_json::to_string_pretty(&report)?;
        println!("{body}");
    } else {
        print_summary(&report, &opts);
    }

    Ok(())
}

fn build_report(root: &Path, opts: &Options) -> Result<AuditReport> {
    let files = collect_files(root, opts);
    let mut sites: Vec<PanicSite> = Vec::new();

    for path in &files {
        let relative = path
            .strip_prefix(root)
            .map(|stripped| stripped.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

        let is_test = is_test_path(&relative);
        let class = DirectoryClass::classify(&relative);
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };

        scan_file(&text, &relative, class, is_test, &mut sites);
    }

    sites.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then(a.path.cmp(&b.path))
            .then(a.line.cmp(&b.line))
    });

    let mut by_pattern: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_class_non_test: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_class_test: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_crate_non_test: BTreeMap<String, usize> = BTreeMap::new();

    for pattern in PanicPattern::all() {
        by_pattern.insert(pattern.label(), 0);
    }
    for class in DirectoryClass::all() {
        by_class_non_test.insert(class.label(), 0);
        by_class_test.insert(class.label(), 0);
    }

    for site in &sites {
        *by_pattern.entry(site.pattern.label()).or_insert(0) += 1;
        if site.is_test {
            *by_class_test.entry(site.class.label()).or_insert(0) += 1;
        } else {
            *by_class_non_test.entry(site.class.label()).or_insert(0) += 1;
            if let Some(crate_dir) = crate_dir_of(&site.path) {
                *by_crate_non_test.entry(crate_dir).or_insert(0) += 1;
            }
        }
    }

    Ok(AuditReport {
        total: sites.len(),
        by_pattern,
        by_class_non_test,
        by_class_test,
        by_crate_non_test,
        sites,
    })
}

/// Audit roots specific to the panic-audit. The shared
/// `rust_scan_roots` convention excludes `xtask`, `pyo3`, `experiments`,
/// and `desktop`, but the no-panic standard applies to all of them
/// (especially the long-lived `pyo3` FFI surface), so we add them here
/// rather than diluting the convention.
fn panic_audit_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = rust_scan_roots(root);
    for extra in [
        "xtask",
        "crates/batchalign-pyo3",
        "experiments",
        "apps/chatter/chatter-gui/src-tauri",
        "apps",
    ] {
        roots.push(root.join(extra));
    }
    roots
}

fn collect_files(root: &Path, opts: &Options) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = panic_audit_roots(root)
        .into_iter()
        .filter(|path| path.exists())
        .flat_map(|root_path| walkdir(&root_path))
        .collect();

    if let Some(filter) = &opts.crate_filter {
        let filter_norm = filter.trim_end_matches('/').to_owned();
        files.retain(|path| {
            let Ok(stripped) = path.strip_prefix(root) else {
                return false;
            };
            let rel = stripped.to_string_lossy().replace('\\', "/");
            rel.starts_with(&format!("{filter_norm}/")) || rel == filter_norm
        });
    }

    files.sort();
    files.dedup();
    files
}

/// Workspace-relative crate root for grouping summary stats. Returns
/// `Some("crates/chatter-lsp")` for any path under that crate; `None`
/// for top-level files like `crates/xtask/src/main.rs` (which fall back to the
/// directory class).
fn crate_dir_of(relative_path: &str) -> Option<String> {
    if let Some(rest) = relative_path.strip_prefix("crates/") {
        let crate_name = rest.split('/').next()?;
        return Some(format!("crates/{crate_name}"));
    }
    if relative_path.starts_with("xtask/") {
        return Some("xtask".to_owned());
    }
    if relative_path.starts_with("crates/batchalign/batchalign-pyo3/") {
        return Some("crates/batchalign-pyo3".to_owned());
    }
    None
}

/// Walk one file and append every panic-pattern hit to `sites`. Doc
/// comments (`///`, `//!`) are skipped wholesale because their code
/// blocks are doctests, not runtime code; line comments and whole-line
/// block comments are also skipped.
///
/// Inline `#[cfg(test)] mod ... { ... }` blocks promote the per-site
/// `is_test` flag from the file-level default: tests inside such a
/// block always classify as test code regardless of file path. The
/// detection is brace-counted from the `mod` line; the previous
/// version of this scanner missed inline test modules and overcounted
/// non-test sites by ~10–20% in lsp/server crates.
fn scan_file(
    text: &str,
    relative_path: &str,
    class: DirectoryClass,
    file_is_test: bool,
    sites: &mut Vec<PanicSite>,
) {
    let mut in_block_comment = false;
    let mut pending_cfg_test = false;
    let mut pending_test_fn = false;
    let mut test_mod_depth_start: Option<i32> = None;
    let mut test_fn_depth_start: Option<i32> = None;
    let mut brace_depth: i32 = 0;

    for (idx, raw_line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim_start();

        // Whole-line block-comment tracking. We only handle the common
        // case where `/*`/`*/` starts at column 0 of the trimmed line,
        // because that's how rustfmt'd code formats comments.
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") && !trimmed.contains("*/") {
            in_block_comment = true;
            continue;
        }

        // Doc-comment lines (doctests are exempt) and ordinary line
        // comments are skipped. `///` and `//!` are caught by the
        // `//` prefix.
        if trimmed.starts_with("//") {
            continue;
        }

        // Strip an inline `//` comment so we don't match patterns
        // appearing in trailing comments. String-literal `//` is the
        // documented false-negative source.
        let code = strip_inline_line_comment(raw_line);

        // Track a freshly-seen `#[cfg(test)]` so the next `mod` line
        // (with its opening brace) gets recognized as a test module,
        // OR a `#[test]` attribute so the next `fn` line gets
        // recognized as a test function.
        if trimmed.starts_with("#[cfg(test)]") || trimmed == "#[cfg(test)]" {
            pending_cfg_test = true;
        } else if trimmed == "#[test]" || trimmed.starts_with("#[test]") {
            pending_test_fn = true;
        } else if pending_cfg_test
            && (trimmed.starts_with("mod ") || trimmed.starts_with("pub mod "))
        {
            if code.contains('{') && test_mod_depth_start.is_none() {
                test_mod_depth_start = Some(brace_depth);
            }
            pending_cfg_test = false;
        } else if pending_test_fn
            && (trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn "))
        {
            if code.contains('{') && test_fn_depth_start.is_none() {
                test_fn_depth_start = Some(brace_depth);
            }
            pending_test_fn = false;
        } else if !trimmed.starts_with('#') {
            // Any non-attribute line resets pending latches.
            pending_cfg_test = false;
            pending_test_fn = false;
        }

        // Resolve effective is_test from file path OR being inside an
        // inline #[cfg(test)] mod block OR inside a #[test]-annotated
        // function body.
        let inside_test_mod = test_mod_depth_start
            .map(|start| brace_depth >= start)
            .unwrap_or(false);
        let inside_test_fn = test_fn_depth_start
            .map(|start| brace_depth >= start)
            .unwrap_or(false);
        let is_test = file_is_test || inside_test_mod || inside_test_fn;

        for (pattern, needle) in PATTERNS {
            if code.contains(needle) {
                sites.push(PanicSite {
                    path: relative_path.to_owned(),
                    line: line_no,
                    pattern: *pattern,
                    class,
                    is_test,
                    excerpt: code.trim().to_owned(),
                });
            }
        }

        // Update brace depth from the code on THIS line. We do this
        // last so the `is_test` decision for sites on the same line as
        // an opening or closing brace uses the depth that was active
        // before the brace took effect; this matches rustfmt's habit
        // of putting the opening `{` of a fn/mod alone on its line.
        for ch in code.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if let Some(start) = test_mod_depth_start
                        && brace_depth <= start
                    {
                        test_mod_depth_start = None;
                    }
                    if let Some(start) = test_fn_depth_start
                        && brace_depth <= start
                    {
                        test_fn_depth_start = None;
                    }
                }
                _ => {}
            }
        }
    }
}

// Each macro is matched by its `name!` prefix only. Listing both
// `name!(` and `name!()` would double-count any line containing
// `name!()` (since `name!()` contains `name!(` as a substring),
// which was the source of the documented audit double-count.
const PATTERNS: &[(PanicPattern, &str)] = &[
    (PanicPattern::Unwrap, ".unwrap()"),
    (PanicPattern::Expect, ".expect("),
    (PanicPattern::Panic, "panic!"),
    (PanicPattern::Todo, "todo!"),
    (PanicPattern::Unimplemented, "unimplemented!"),
    (PanicPattern::Unreachable, "unreachable!"),
];

/// Trim everything from the first `//` that isn't inside a (very
/// permissively detected) string literal. Good enough for rustfmt'd
/// code; the audit doesn't need single-token precision. Returns a
/// borrowed slice — full-workspace audits walk ~5M lines per run, so
/// avoiding the per-line allocation matters.
fn strip_inline_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut prev = ' ';
    for (idx, ch) in line.char_indices() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        }
        if !in_string && ch == '/' && line[idx..].starts_with("//") {
            return &line[..idx];
        }
        prev = ch;
    }
    line
}

fn print_summary(report: &AuditReport, opts: &Options) {
    println!("==> panic-audit: {} sites total", report.total);
    if let Some(filter) = &opts.crate_filter {
        println!("==> panic-audit: filter applied: {filter}");
    }
    println!();

    println!("By pattern:");
    for pattern in PanicPattern::all() {
        let count = report.by_pattern.get(pattern.label()).copied().unwrap_or(0);
        println!("  {:<14} {count:>6}", pattern.label());
    }
    println!();

    println!("By directory class (non-test code):");
    for class in DirectoryClass::all() {
        let count = report
            .by_class_non_test
            .get(class.label())
            .copied()
            .unwrap_or(0);
        println!("  {:<14} {count:>6}", class.label());
    }
    println!();

    println!("By directory class (test code):");
    for class in DirectoryClass::all() {
        let count = report
            .by_class_test
            .get(class.label())
            .copied()
            .unwrap_or(0);
        println!("  {:<14} {count:>6}", class.label());
    }
    println!();

    println!("Top crates (non-test sites, descending):");
    let mut by_crate: Vec<(&String, &usize)> = report.by_crate_non_test.iter().collect();
    by_crate.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (crate_dir, count) in by_crate.iter().take(20) {
        println!("  {crate_dir:<40} {count:>6}");
    }
}
