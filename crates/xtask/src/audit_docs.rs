//! Doc-to-code provenance audit — talkbank-tools doc audit.
//!
//! Catalogs every markdown file in talkbank-tools into a SQLite
//! database; the per-section `vet_state` machine tracks whether a
//! reviewer (human or Claude-assisted) has confirmed the prose
//! matches the actual code at HEAD.
//!
//! Scope: **talkbank-tools only.** The meta-repo workspace contains
//! a lot of historical / now-stale prose and is not used as
//! evidence; citations and references in this catalog point at
//! talkbank-tools source exclusively.
//!
//! Buckets:
//!   - **A** — must-vet: user-facing public surface (book user-guide
//!     chapters, top-level READMEs, CLAN reference). Vetting Bucket A
//!     to completion is the release-readiness signal.
//!   - **B** — should-vet: dev-facing public surface (architecture,
//!     contributing, developer chapters). Sample-vetted; not
//!     release-blocking.
//!   - **C** — won't-vet: internal docs (postmortems, handoffs,
//!     contributor CLAUDE.md, etc.). One-time Status-header sweep
//!     ensures readers know the doc is historical or reference;
//!     no claim verification.
//!
//! Subcommands:
//!   - `scan` — populate / refresh `docs` and `sections`.
//!   - `flag-staleness` — re-run regex surface-scan into `staleness_flags`.
//!   - `status` — print queue head + streak + Bucket A progress.
//!   - `vet` — mark a section's verdict.
//!   - `streak` — print the daily-cadence streak count.

use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use regex::Regex;
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use walkdir::WalkDir;

use crate::Result;

/// Open the audit catalog. `create_if_missing` is on so the first
/// run on a fresh checkout produces an empty DB without operator
/// intervention.
async fn open_catalog(path: &Path) -> sqlx::Result<SqliteConnection> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&opts).await
}

/// Cutoff: the date of the talkbank-tools / batchalign3 monorepo merge.
/// Docs whose `Last updated` predates this are 'pre-merge' staleness.
const POST_MERGE_BASELINE: &str = "2026-04-28";

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".venv",
    ".venv-314t",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
    // Meta-repo embeds talkbank-tools as a sub-clone; skip the nested
    // copy so we don't double-count.
    "talkbank-tools",
    // Audit working files (catalog can't audit itself).
    "release-doc-audit",
    // Frozen review artifacts.
    "batchalign3-review-book",
    // Build outputs that may contain rendered markdown copies.
    "html",
    "site",
];

/// The single repo the catalog audits. Kept as a const rather than
/// a single-variant enum because there's only one source of evidence;
/// the column on `docs` is preserved for schema continuity but always
/// carries this string.
const REPO_TALKBANK_TOOLS: &str = "talkbank-tools";

/// Bucket assignment — deterministic function of file path. Recomputed
/// on every scan; never hand-edited. See classify_bucket().
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Must-vet: user-facing public surface. Release-readiness gate.
    A,
    /// Should-vet: dev-facing public surface. Sample-vetted.
    B,
    /// Won't-vet: internal-only. Status-header sweep only.
    C,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Bucket::A => "A",
            Bucket::B => "B",
            Bucket::C => "C",
        }
    }
}

impl fmt::Display for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    /// `Last updated` >= POST_MERGE_BASELINE.
    Fresh,
    /// `Last updated` < POST_MERGE_BASELINE; likeliest to reference
    /// deleted crates / moved book paths from the reorgs.
    PreMerge,
    /// No parsable `Last updated` header.
    Unknown,
}

impl Staleness {
    fn as_str(self) -> &'static str {
        match self {
            Staleness::Fresh => "fresh",
            Staleness::PreMerge => "pre-merge",
            Staleness::Unknown => "unknown",
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub db: PathBuf,
    pub talkbank_tools_root: PathBuf,
}

pub async fn run(args: Args) -> Result<()> {
    let mut conn = open_catalog(&args.db).await?;
    // WAL + relaxed sync makes the per-doc transaction commits far
    // cheaper without giving up crash-safety against the OS losing
    // the last few writes (which we don't care about — the next scan
    // re-derives identical state from source).
    sqlx::raw_sql(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )
    .execute(&mut conn)
    .await?;
    apply_migrations(&mut conn).await?;

    let scanned_at = iso_now();
    let root = args.talkbank_tools_root.as_path();

    let mut total_docs = 0u64;
    let mut new_docs = 0u64;
    let mut total_sections = 0u64;
    let mut bucket_counts: std::collections::HashMap<Bucket, u64> =
        std::collections::HashMap::new();

    for path in walk_markdown(root) {
        let rel = match path.strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().to_string();

        if rel_str.contains("release-doc-audit/") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let doc_hash = blake3_hex(content.as_bytes());
        let (status_label, last_modified_doc) = parse_meta_headers(&content);
        let audience = classify_audience(&rel_str);
        let priority = classify_priority(&rel_str, &content);
        let ba2_vs_ba3 = classify_ba2_vs_ba3(&rel_str, &content);
        let staleness = classify_staleness(last_modified_doc.as_deref());
        let bucket = classify_bucket(&rel_str);
        *bucket_counts.entry(bucket).or_insert(0) += 1;

        // One transaction per doc keeps the upsert + section
        // reconciliation atomic and amortizes the WAL fsync.
        let mut tx = sqlx::Connection::begin(&mut conn).await?;
        let (doc_id, outcome) = upsert_doc(
            &mut *tx,
            &rel_str,
            &audience,
            priority,
            ba2_vs_ba3,
            staleness,
            bucket,
            status_label.as_deref(),
            last_modified_doc.as_deref(),
            &doc_hash,
            &scanned_at,
        )
        .await?;
        if outcome == UpsertOutcome::Inserted {
            new_docs += 1;
        }
        total_docs += 1;

        let sections = parse_sections(&content);
        total_sections += sections.len() as u64;
        sync_sections(&mut *tx, doc_id, &sections).await?;
        tx.commit().await?;
    }

    println!("xtask audit-docs scan complete:");
    println!("  scanned_at:        {scanned_at}");
    println!("  docs total:        {total_docs}");
    println!("  docs new:          {new_docs}");
    println!("  sections in tree:  {total_sections}");
    println!(
        "  bucket A / B / C:  {} / {} / {}",
        bucket_counts.get(&Bucket::A).copied().unwrap_or(0),
        bucket_counts.get(&Bucket::B).copied().unwrap_or(0),
        bucket_counts.get(&Bucket::C).copied().unwrap_or(0),
    );
    println!();
    print_summary(&mut conn).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Walk
// ---------------------------------------------------------------------------

fn walk_markdown(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // SKIP_DIRS applies only at depth > 0 so the walker enters its own
    // root even when the root's last path component matches a skip-name
    // (e.g. walking `/Users/chen/talkbank/talkbank-tools`).
    let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let n = e.file_name().to_string_lossy();
        !SKIP_DIRS.iter().any(|s| n == *s)
    });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(entry.path().to_path_buf());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Section parsing
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ParsedSection {
    level: u8,
    heading: String,
    anchor: String,
    ordinal: i64,
    line_start: i64,
    line_end: i64,
    body_hash: String,
}

fn parse_sections(content: &str) -> Vec<ParsedSection> {
    let lines: Vec<&str> = content.lines().collect();

    // Single-pass: track code-fence state while collecting H2/H3 heads.
    let mut heads: Vec<(usize, u8, String)> = Vec::new();
    let mut in_code_fence = false;
    let mut fence_marker: Option<&'static str> = None;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !in_code_fence {
            if trimmed.starts_with("```") {
                in_code_fence = true;
                fence_marker = Some("```");
                continue;
            }
            if trimmed.starts_with("~~~") {
                in_code_fence = true;
                fence_marker = Some("~~~");
                continue;
            }
            if let Some((level, heading)) = parse_heading(line) {
                if level == 2 || level == 3 {
                    heads.push((idx, level, heading));
                }
            }
        } else if let Some(marker) = fence_marker {
            if trimmed.starts_with(marker) {
                in_code_fence = false;
                fence_marker = None;
            }
        }
    }

    let mut anchor_counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::with_capacity(heads.len());
    for (i, (line_idx, level, heading)) in heads.iter().enumerate() {
        // Section ends at the next heading of level <= current OR EOF.
        let mut end_line = lines.len();
        for (j_line, j_level, _) in heads.iter().skip(i + 1) {
            if *j_level <= *level {
                end_line = *j_line;
                break;
            }
        }
        let body_text = lines[(line_idx + 1)..end_line].join("\n");
        let body_hash = blake3_hex(body_text.as_bytes());

        let base_anchor = slugify(heading);
        let count = anchor_counts.entry(base_anchor.clone()).or_insert(0);
        let anchor = if *count == 0 {
            base_anchor.clone()
        } else {
            format!("{base_anchor}-{count}")
        };
        *count += 1;

        out.push(ParsedSection {
            level: *level,
            heading: heading.clone(),
            anchor,
            ordinal: i as i64,
            line_start: (*line_idx as i64) + 1,
            line_end: end_line as i64,
            body_hash,
        });
    }

    out
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let mut level = 0u8;
    let mut chars = trimmed.chars();
    for c in chars.by_ref() {
        if c == '#' {
            level += 1;
            if level > 6 {
                return None;
            }
        } else if c == ' ' {
            break;
        } else {
            return None;
        }
    }
    if level == 0 {
        return None;
    }
    let rest: String = chars.collect();
    let heading = rest.trim().trim_end_matches('#').trim().to_string();
    if heading.is_empty() {
        return None;
    }
    Some((level, heading))
}

/// GitHub-style anchor slug. Approximate; not byte-identical to mdBook
/// in every Unicode edge case. Sufficient for keying sections.
fn slugify(heading: &str) -> String {
    let mut out = String::with_capacity(heading.len());
    for c in heading.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
    }
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !prev_dash && !collapsed.is_empty() {
                collapsed.push(c);
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    collapsed.trim_end_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Metadata extraction
// ---------------------------------------------------------------------------

fn parse_meta_headers(content: &str) -> (Option<String>, Option<String>) {
    let mut status = None;
    let mut last_modified = None;
    for line in content.lines().take(30) {
        if status.is_none() {
            if let Some(v) = extract_field(line, "Status") {
                status = Some(v);
            }
        }
        if last_modified.is_none() {
            if let Some(v) = extract_field(line, "Last updated") {
                last_modified = Some(v);
            } else if let Some(v) = extract_field(line, "Last modified") {
                last_modified = Some(v);
            }
        }
        if status.is_some() && last_modified.is_some() {
            break;
        }
    }
    (status, last_modified)
}

fn extract_field(line: &str, name: &str) -> Option<String> {
    let bold = format!("**{}:**", name);
    if let Some(idx) = line.find(&bold) {
        let v = line[idx + bold.len()..].trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let plain = format!("{}:", name);
    if line.trim_start().starts_with(&plain) {
        let v = line.trim_start()[plain.len()..].trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Classification heuristics
// ---------------------------------------------------------------------------

fn classify_audience(rel: &str) -> String {
    if rel.ends_with("CLAUDE.md") {
        return "dev".to_string();
    }
    if rel.starts_with("book/src/")
        && (rel.contains("/user-guide/")
            || rel.contains("/getting-started/")
            || rel.contains("/install/")
            || rel.contains("/quickstart/")
            || rel.contains("/chat-format/"))
    {
        return "user".to_string();
    }
    if rel.starts_with("book/src/")
        && (rel.contains("/architecture/")
            || rel.contains("/developer/")
            || rel.contains("/contributing/")
            || rel.contains("/decisions/"))
    {
        return "dev".to_string();
    }
    if rel.starts_with("book/src/") {
        return "mixed".to_string();
    }
    if rel == "README.md"
        || rel.ends_with("/README.md")
        || rel == "CONTRIBUTING.md"
        || rel == "SECURITY.md"
        || rel == "INTERFACE_MAP.md"
    {
        return "mixed".to_string();
    }
    if rel.starts_with("docs/") {
        return "dev".to_string();
    }
    "mixed".to_string()
}

fn classify_priority(rel: &str, content: &str) -> i64 {
    if classify_ba2_vs_ba3(rel, content) {
        return 1;
    }
    if rel == "README.md" || rel == "CONTRIBUTING.md" || rel == "SECURITY.md" {
        return 1;
    }
    if rel.ends_with("CLAUDE.md") {
        return 2;
    }
    if rel.starts_with("book/src/")
        && (rel.contains("/user-guide/")
            || rel.contains("/getting-started/")
            || rel.contains("/quickstart/")
            || rel.contains("/install/"))
    {
        return 2;
    }
    if rel.starts_with("book/src/") {
        return 3;
    }
    if rel.starts_with("docs/") {
        return 4;
    }
    4
}

/// Bucket assignment, deterministic from path. Recomputed every
/// scan; never hand-edited.
///
/// **Bucket A — must-vet, user-facing public surface.** Every section
/// here gets a verdict before the public release. This is the
/// release-readiness gate.
///
/// **Bucket B — should-vet, dev-facing public surface.** Sample-vetted
/// opportunistically. Not release-blocking.
///
/// **Bucket C — won't-vet, internal-only.** One-time Status-header
/// sweep ensures readers know the doc is historical or reference;
/// no claim verification.
fn classify_bucket(rel: &str) -> Bucket {
    // Bucket C: internal-only docs, contributor guidance, postmortems,
    // handoffs, investigations, decision records.
    if rel.ends_with("CLAUDE.md")
        || rel.starts_with("docs/postmortems/")
        || rel.starts_with("docs/handoffs/")
        || rel.starts_with("docs/investigations/")
        || rel.starts_with("docs/migration/")
        || rel.starts_with("docs/decisions/")
        || rel.starts_with("docs/internal/")
    {
        return Bucket::C;
    }

    // Bucket A: user-facing public surface and load-bearing top-level
    // entry points an external reader will see.
    if rel == "README.md"
        || rel == "CONTRIBUTING.md"
        || rel == "SECURITY.md"
        || rel.ends_with("/README.md")  // per-crate READMEs visible on docs.rs / crates.io
        || rel.starts_with("book/src/chatter/")
        || rel.starts_with("book/src/clan-reference/")
        || rel.starts_with("book/src/chat-format/")
        || rel.starts_with("book/src/apps/vscode-extension/")
        || (rel.starts_with("book/src/batchalign/")
            && rel.contains("/user-guide/"))
        || rel == "book/src/SUMMARY.md"
        || rel == "book/src/introduction.md"
    {
        return Bucket::A;
    }

    // Bucket B: dev-facing public surface — visible on GitHub / mdBook
    // but written for contributors and integrators.
    if rel.starts_with("book/src/architecture/")
        || rel.starts_with("book/src/contributing/")
        || rel.starts_with("book/src/")
            && (rel.contains("/architecture/")
                || rel.contains("/developer/")
                || rel.contains("/contributing/"))
    {
        return Bucket::B;
    }

    // Default: anything else under book/src/ that we haven't bucketed
    // explicitly is dev-facing public surface (Bucket B); anything
    // under docs/ that escaped the C list is internal (Bucket C).
    if rel.starts_with("book/src/") {
        return Bucket::B;
    }
    Bucket::C
}

/// Compare the doc's `Last updated:` header to the post-merge
/// baseline. Header format is operator-controlled (typically
/// `YYYY-MM-DD HH:MM TZ`); we look for an ISO-style date prefix and
/// fall back to `Unknown` for anything else.
fn classify_staleness(last_modified: Option<&str>) -> Staleness {
    let raw = match last_modified {
        Some(s) => s.trim(),
        None => return Staleness::Unknown,
    };
    let prefix = raw.get(..10).unwrap_or("");
    let parsed = NaiveDate::parse_from_str(prefix, "%Y-%m-%d");
    let cutoff = NaiveDate::parse_from_str(POST_MERGE_BASELINE, "%Y-%m-%d");
    match (parsed, cutoff) {
        (Ok(d), Ok(c)) if d >= c => Staleness::Fresh,
        (Ok(_), Ok(_)) => Staleness::PreMerge,
        _ => Staleness::Unknown,
    }
}

fn classify_ba2_vs_ba3(rel: &str, content: &str) -> bool {
    if rel.contains("/migration/") || rel.contains("ba2-") || rel.contains("BA2") {
        return true;
    }
    let mut limit = content.len().min(4096);
    while limit > 0 && !content.is_char_boundary(limit) {
        limit -= 1;
    }
    let head = &content[..limit];
    head.contains("BA2 ") || head.contains("Batchalign2") || head.contains("batchalign2")
}

// ---------------------------------------------------------------------------
// SQLite upserts
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn upsert_doc(
    conn: &mut SqliteConnection,
    path: &str,
    audience: &str,
    priority: i64,
    ba2_vs_ba3: bool,
    staleness: Staleness,
    bucket: Bucket,
    status_label: Option<&str>,
    last_modified_doc: Option<&str>,
    content_hash: &str,
    scanned_at: &str,
) -> Result<(i64, UpsertOutcome)> {
    // Two-step rather than RETURNING: SQLite has no direct "was-this-an-
    // insert-or-an-update" signal, so we look up the prior id once and
    // report new-vs-existing from that.
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM docs WHERE repo = ? AND path = ?")
            .bind(REPO_TALKBANK_TOOLS)
            .bind(path)
            .fetch_optional(&mut *conn)
            .await?;
    let outcome = match existing {
        Some(_) => UpsertOutcome::Updated,
        None => UpsertOutcome::Inserted,
    };
    sqlx::query(
        "INSERT INTO docs
            (repo, path, audience, priority, ba2_vs_ba3, staleness, bucket,
             status_label, last_modified_doc, content_hash, scanned_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(repo, path) DO UPDATE SET
            audience = excluded.audience,
            priority = excluded.priority,
            ba2_vs_ba3 = excluded.ba2_vs_ba3,
            staleness = excluded.staleness,
            bucket = excluded.bucket,
            status_label = excluded.status_label,
            last_modified_doc = excluded.last_modified_doc,
            content_hash = excluded.content_hash,
            scanned_at = excluded.scanned_at",
    )
    .bind(REPO_TALKBANK_TOOLS)
    .bind(path)
    .bind(audience)
    .bind(priority)
    .bind(i64::from(ba2_vs_ba3))
    .bind(staleness.as_str())
    .bind(bucket.as_str())
    .bind(status_label)
    .bind(last_modified_doc)
    .bind(content_hash)
    .bind(scanned_at)
    .execute(&mut *conn)
    .await?;
    let id: i64 = sqlx::query_scalar("SELECT id FROM docs WHERE repo = ? AND path = ?")
        .bind(REPO_TALKBANK_TOOLS)
        .bind(path)
        .fetch_one(&mut *conn)
        .await?;
    Ok((id, outcome))
}

/// Reconcile section rows for one doc against the freshly-parsed list.
/// Anchors absent from the new parse are deleted (cascading their claims
/// and citations). Anchors with unchanged `content_hash` keep their
/// `vet_state`; changed content resets to `unvetted` while preserving
/// the row id so attached claims/citations remain available for diffing.
async fn sync_sections(
    conn: &mut SqliteConnection,
    doc_id: i64,
    sections: &[ParsedSection],
) -> Result<()> {
    // One SELECT per doc fetches every existing (anchor, content_hash);
    // the per-section logic then runs entirely from this map. Avoids
    // the previous N+1 SELECT pattern.
    let existing: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT anchor, content_hash FROM sections WHERE doc_id = ?",
    )
    .bind(doc_id)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    let existing_anchors: BTreeSet<&str> = existing.keys().map(String::as_str).collect();
    let current_anchors: BTreeSet<&str> = sections.iter().map(|s| s.anchor.as_str()).collect();

    for anchor in existing_anchors.difference(&current_anchors) {
        sqlx::query("DELETE FROM sections WHERE doc_id = ? AND anchor = ?")
            .bind(doc_id)
            .bind(*anchor)
            .execute(&mut *conn)
            .await?;
    }

    for s in sections {
        match existing.get(&s.anchor) {
            None => {
                sqlx::query(
                    "INSERT INTO sections
                        (doc_id, level, heading, anchor, ordinal,
                         line_start, line_end, content_hash)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(doc_id)
                .bind(i64::from(s.level))
                .bind(&s.heading)
                .bind(&s.anchor)
                .bind(s.ordinal)
                .bind(s.line_start)
                .bind(s.line_end)
                .bind(&s.body_hash)
                .execute(&mut *conn)
                .await?;
            }
            Some(prev_hash) if *prev_hash == s.body_hash => {
                sqlx::query(
                    "UPDATE sections SET
                        level = ?, heading = ?, ordinal = ?,
                        line_start = ?, line_end = ?
                     WHERE doc_id = ? AND anchor = ?",
                )
                .bind(i64::from(s.level))
                .bind(&s.heading)
                .bind(s.ordinal)
                .bind(s.line_start)
                .bind(s.line_end)
                .bind(doc_id)
                .bind(&s.anchor)
                .execute(&mut *conn)
                .await?;
            }
            Some(_) => {
                sqlx::query(
                    "UPDATE sections SET
                        level = ?, heading = ?, ordinal = ?,
                        line_start = ?, line_end = ?,
                        content_hash = ?,
                        vet_state = 'unvetted',
                        reviewer = NULL,
                        reviewed_at = NULL
                     WHERE doc_id = ? AND anchor = ?",
                )
                .bind(i64::from(s.level))
                .bind(&s.heading)
                .bind(s.ordinal)
                .bind(s.line_start)
                .bind(s.line_end)
                .bind(&s.body_hash)
                .bind(doc_id)
                .bind(&s.anchor)
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Migrations
// ---------------------------------------------------------------------------

/// Apply additive schema migrations on every run. Idempotent: each
/// step checks the current state via `pragma_table_info` before
/// adding columns, and `CREATE TABLE IF NOT EXISTS` covers new tables.
///
/// This lets a databases created with an older version of the schema
/// pick up newer columns without a manual migration step.
/// Idempotent schema migration. Runs on every scan / status / vet
/// invocation. Bootstraps a fresh DB with `CREATE TABLE IF NOT EXISTS`,
/// upgrades older catalogs incrementally, and (Phase 0 v2 cutover)
/// drops out-of-scope rows + retired tables.
async fn apply_migrations(conn: &mut SqliteConnection) -> Result<()> {
    // Bootstrap: docs and sections schemas. `IF NOT EXISTS` makes
    // this safe on existing DBs (the table is left alone) and
    // unblocks fresh-checkout runs (which previously hit
    // "no such table: docs"). Indexes that reference newly-added
    // columns (e.g. `bucket`) are created later, AFTER the
    // incremental ALTERs below, so they can't fire on a still-
    // missing column on a legacy catalog.
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS docs (
            id INTEGER PRIMARY KEY,
            repo TEXT NOT NULL,
            path TEXT NOT NULL,
            audience TEXT NOT NULL,
            priority INTEGER NOT NULL,
            ba2_vs_ba3 INTEGER NOT NULL DEFAULT 0,
            staleness TEXT NOT NULL DEFAULT 'unknown',
            bucket TEXT NOT NULL DEFAULT 'C',
            status_label TEXT,
            last_modified_doc TEXT,
            content_hash TEXT NOT NULL,
            scanned_at TEXT NOT NULL,
            UNIQUE(repo, path)
         );
         CREATE TABLE IF NOT EXISTS sections (
            id INTEGER PRIMARY KEY,
            doc_id INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
            level INTEGER NOT NULL,
            heading TEXT NOT NULL,
            anchor TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            line_start INTEGER NOT NULL,
            line_end INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            vet_state TEXT NOT NULL DEFAULT 'unvetted',
            fix_commit_hash TEXT,
            reviewer TEXT,
            reviewed_at TEXT,
            notes TEXT,
            UNIQUE(doc_id, anchor)
         );",
    )
    .execute(&mut *conn)
    .await?;

    // Incremental column adds for catalogs that pre-date these
    // columns. column_exists() returns false for missing tables
    // too, but the CREATE TABLE IF NOT EXISTS above guarantees the
    // tables exist by the time we get here.
    if !column_exists(conn, "docs", "staleness").await? {
        sqlx::raw_sql("ALTER TABLE docs ADD COLUMN staleness TEXT NOT NULL DEFAULT 'unknown';")
            .execute(&mut *conn)
            .await?;
    }
    if !column_exists(conn, "docs", "bucket").await? {
        sqlx::raw_sql("ALTER TABLE docs ADD COLUMN bucket TEXT NOT NULL DEFAULT 'C';")
            .execute(&mut *conn)
            .await?;
    }
    if !column_exists(conn, "sections", "fix_commit_hash").await? {
        sqlx::raw_sql("ALTER TABLE sections ADD COLUMN fix_commit_hash TEXT;")
            .execute(&mut *conn)
            .await?;
    }
    if !column_exists(conn, "sections", "notes").await? {
        sqlx::raw_sql("ALTER TABLE sections ADD COLUMN notes TEXT;")
            .execute(&mut *conn)
            .await?;
    }

    // Phase 0 v2 cutover: scope catalog to talkbank-tools only.
    // Meta-repo docs are no longer evidence (too much historical
    // staleness); the prior catalog may carry rows where
    // `repo = 'meta'`. Drop them; cascade deletes flow through
    // sections + claims + citations + staleness_flags.
    sqlx::raw_sql("DELETE FROM docs WHERE repo != 'talkbank-tools';")
        .execute(&mut *conn)
        .await?;

    // Phase 0 v2 cutover: retire claims/citations tables. The
    // section-level `vet_state` is the new unit of work (lighter
    // discipline; reviewer judgment, not commit-hash-pinned
    // citation chains). DROP IF EXISTS is safe on catalogs that
    // never had them.
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS citations;
         DROP TABLE IF EXISTS claims;",
    )
    .execute(&mut *conn)
    .await?;

    // staleness_flags table: idempotent, retained for surface-scan
    // regex hits.
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS staleness_flags (
            id INTEGER PRIMARY KEY,
            section_id INTEGER NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
            pattern_name TEXT NOT NULL,
            pattern_severity TEXT NOT NULL,
            match_line INTEGER,
            match_excerpt TEXT,
            flagged_at TEXT NOT NULL
         );",
    )
    .execute(&mut *conn)
    .await?;

    // Indexes go LAST: created after every column they reference is
    // guaranteed to exist (either by the CREATE TABLE bootstrap above
    // or by an incremental ALTER ADD COLUMN). On a legacy catalog
    // missing `bucket`, building idx_docs_bucket inside the bootstrap
    // block would error before the ALTER had a chance to fire.
    sqlx::raw_sql(
        "CREATE INDEX IF NOT EXISTS idx_sections_doc ON sections(doc_id);
         CREATE INDEX IF NOT EXISTS idx_docs_priority ON docs(priority, ba2_vs_ba3 DESC);
         CREATE INDEX IF NOT EXISTS idx_docs_bucket ON docs(bucket);
         CREATE INDEX IF NOT EXISTS idx_staleness_flags_section
            ON staleness_flags(section_id);
         CREATE INDEX IF NOT EXISTS idx_staleness_flags_pattern
            ON staleness_flags(pattern_name);",
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn column_exists(conn: &mut SqliteConnection, table: &str, column: &str) -> Result<bool> {
    // pragma_table_info is a SQLite table-valued function that does not
    // accept normal parameter binding — sqlx 0.8.x's prepared-statement
    // pipeline returns spurious matches when the table-name is bound
    // via `?`. Inline the table name (validated against an allowlist
    // by the caller's flow — apply_migrations only passes literals)
    // and bind only the column name.
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("column_exists: refusing non-identifier table name '{table}'").into());
    }
    let sql = format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?");
    let exists: Option<i64> = sqlx::query_scalar(&sql)
        .bind(column)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(exists.is_some())
}

// ---------------------------------------------------------------------------
// Staleness flagging (xtask audit-docs flag-staleness)
// ---------------------------------------------------------------------------

pub(crate) struct FlagPattern {
    pub(crate) name: &'static str,
    pub(crate) severity: &'static str,
    /// Regex to match against each line of every section body.
    pub(crate) pattern: &'static str,
}

pub(crate) const FLAG_PATTERNS: &[FlagPattern] = &[
    FlagPattern {
        name: "deleted-crate-chat-ops",
        severity: "high",
        pattern: r"\bbatchalign-chat-ops\b",
    },
    FlagPattern {
        name: "deleted-crate-batchalign-app",
        severity: "high",
        pattern: r"\bbatchalign-app\b",
    },
    FlagPattern {
        name: "deleted-crate-batchalign-revai",
        severity: "high",
        pattern: r"\bbatchalign-revai\b",
    },
    FlagPattern {
        name: "deleted-crate-talkbank-redact",
        severity: "high",
        pattern: r"\btalkbank-redact\b",
    },
    FlagPattern {
        name: "renamed-crate-talkbank-re2c-parser",
        severity: "medium",
        pattern: r"\btalkbank-re2c-parser\b",
    },
    FlagPattern {
        name: "moved-book-path-batchalign-book",
        severity: "high",
        pattern: r"\bbatchalign-book/",
    },
    FlagPattern {
        name: "moved-book-path-vscode-book",
        severity: "high",
        pattern: r"\bvscode/book/",
    },
    FlagPattern {
        name: "moved-book-path-clan-book",
        severity: "high",
        pattern: r"\bcrates/clan-core/book/",
    },
    FlagPattern {
        name: "absolute-crate-count",
        severity: "medium",
        pattern: r"\b(\d+) crates\b",
    },
    FlagPattern {
        name: "absolute-test-count",
        severity: "low",
        pattern: r"\b(\d+) tests\b",
    },
    FlagPattern {
        name: "absolute-file-count",
        severity: "low",
        pattern: r"\b(\d+) (?:files|sections|chapters|pages)\b",
    },
    FlagPattern {
        name: "phase-still-pending",
        severity: "medium",
        // P1-P10 have all landed; treat phase mentions as suspect.
        pattern: r"\b(?:P[1-9]|P10)\s+(?:remains|is|will be|TBD|pending)\b",
    },
];

pub async fn run_flag_staleness(args: Args) -> Result<()> {
    let mut conn = open_catalog(&args.db).await?;
    sqlx::raw_sql(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )
    .execute(&mut conn)
    .await?;
    apply_migrations(&mut conn).await?;

    let regexes: Vec<(usize, Regex)> = FLAG_PATTERNS
        .iter()
        .enumerate()
        .map(|(i, p)| Ok((i, Regex::new(p.pattern)?)))
        .collect::<Result<Vec<_>>>()?;

    // Wipe and rebuild — patterns can change between runs and
    // ambiguous "what was here last run?" semantics aren't worth
    // keeping. Cheap operation against an indexed table.
    sqlx::query("DELETE FROM staleness_flags")
        .execute(&mut conn)
        .await?;

    // Walk every section, re-read its source file, scan body lines.
    // Catalog is talkbank-tools-only since the v2 cutover, so the
    // root is the single talkbank-tools clone passed in args.
    let now = iso_now();
    let rows: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT s.id, s.line_start, s.line_end, d.path
         FROM sections s JOIN docs d ON d.id = s.doc_id",
    )
    .fetch_all(&mut conn)
    .await?;

    // Cache file contents per path so we don't re-read for every
    // section in the same file.
    let mut file_cache: HashMap<String, Option<Vec<String>>> = HashMap::new();

    let mut tx = sqlx::Connection::begin(&mut conn).await?;
    let mut total_flags = 0u64;
    for (section_id, line_start, line_end, rel_path) in rows {
        let lines = file_cache.entry(rel_path.clone()).or_insert_with(|| {
            let root: &Path = args.talkbank_tools_root.as_path();
            std::fs::read_to_string(root.join(&rel_path))
                .ok()
                .map(|s| s.lines().map(str::to_owned).collect())
        });
        let Some(lines) = lines else { continue };
        if lines.is_empty() {
            continue;
        }

        // Bound to body lines only: heading is at line_start (1-based);
        // body runs from line_start+1 to line_end (inclusive).
        let body_start = line_start as usize; // 0-based offset of the line AFTER the heading
        let body_end = (line_end as usize).min(lines.len());
        if body_start >= body_end {
            continue;
        }

        for body_idx in body_start..body_end {
            let line = &lines[body_idx];
            for (i, regex) in &regexes {
                let pat = &FLAG_PATTERNS[*i];
                if let Some(m) = regex.find(line) {
                    let mut excerpt = m.as_str().to_string();
                    if excerpt.len() > 200 {
                        // char-boundary safe truncate
                        let mut end = 200;
                        while end > 0 && !excerpt.is_char_boundary(end) {
                            end -= 1;
                        }
                        excerpt.truncate(end);
                    }
                    sqlx::query(
                        "INSERT INTO staleness_flags
                            (section_id, pattern_name, pattern_severity,
                             match_line, match_excerpt, flagged_at)
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(section_id)
                    .bind(pat.name)
                    .bind(pat.severity)
                    .bind((body_idx as i64) + 1)
                    .bind(&excerpt)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                    total_flags += 1;
                }
            }
        }
    }
    tx.commit().await?;

    println!("xtask audit-docs flag-staleness complete:");
    println!("  flags inserted: {total_flags}");
    println!();
    print_flag_summary(&mut conn).await?;
    Ok(())
}

async fn print_flag_summary(conn: &mut SqliteConnection) -> Result<()> {
    println!("By pattern (top hits):");
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT pattern_name, pattern_severity, COUNT(*) AS hits,
                COUNT(DISTINCT section_id) AS sections
         FROM staleness_flags
         GROUP BY pattern_name
         ORDER BY hits DESC",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (pattern, severity, hits, sections) in rows {
        println!("  {hits:>5} hits in {sections:>4} sections  [{severity}]  {pattern}");
    }
    println!();

    println!("Top 10 sections by flag count:");
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT d.repo, d.path, s.heading, COUNT(*) AS flag_count
         FROM staleness_flags f
         JOIN sections s ON s.id = f.section_id
         JOIN docs d ON d.id = s.doc_id
         GROUP BY f.section_id
         ORDER BY flag_count DESC
         LIMIT 10",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (repo, path, heading, flag_count) in rows {
        println!("  {flag_count:>3}  [{repo}] {path} :: {heading}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

async fn print_summary(conn: &mut SqliteConnection) -> Result<()> {
    println!("By audience / priority bucket:");
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT audience, priority, COUNT(*)
         FROM docs
         GROUP BY audience, priority
         ORDER BY priority, audience",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (audience, priority, count) in rows {
        println!("  P{priority}  {audience:<10}  {count:>5} docs");
    }
    println!();

    let total_sections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sections")
        .fetch_one(&mut *conn)
        .await?;
    let unvetted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sections WHERE vet_state = 'unvetted'")
            .fetch_one(&mut *conn)
            .await?;
    println!("Sections: {total_sections} total, {unvetted} unvetted");

    let ba2_docs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM docs WHERE ba2_vs_ba3 = 1")
        .fetch_one(&mut *conn)
        .await?;
    println!("BA2-vs-BA3 docs flagged: {ba2_docs}");

    println!();
    println!("Top 20 BA2-vs-BA3 sections by body line count:");
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT d.repo, d.path, s.heading, (s.line_end - s.line_start) AS body_lines
         FROM sections s
         JOIN docs d ON d.id = s.doc_id
         WHERE d.ba2_vs_ba3 = 1
         ORDER BY body_lines DESC
         LIMIT 20",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (repo, path, heading, body_lines) in rows {
        println!("  {body_lines:>4}  [{repo}] {path} :: {heading}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn iso_now() -> String {
    Local::now().format("%Y-%m-%d %H:%M %Z").to_string()
}

// ---------------------------------------------------------------------------
// Daily-cadence subcommands: status / streak / vet
// ---------------------------------------------------------------------------

// Terminal vet_state values are: 'no-claims', 'vetted-accurate',
// 'fixed'. The other states ('unvetted', 'in-review', 'needs-fix')
// keep the section in the active queue. This list lives in the SQL
// queries below as a literal `IN (...)` clause; centralizing it here
// would require dynamic SQL building, which adds more complexity than
// it removes for this catalog.

/// Print the queue head + Bucket A progress + streak. The single
/// command an operator runs at the start of every audit session:
/// answers "where am I, what's next, am I keeping the streak."
async fn run_status(db: &Path) -> Result<()> {
    let mut conn = open_catalog(db).await?;
    apply_migrations(&mut conn).await?;

    // Bucket A progress: how much of the must-vet surface is done?
    let bucket_a_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sections s
         JOIN docs d ON d.id = s.doc_id
         WHERE d.bucket = 'A'",
    )
    .fetch_one(&mut conn)
    .await?;
    let bucket_a_terminal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sections s
         JOIN docs d ON d.id = s.doc_id
         WHERE d.bucket = 'A'
           AND s.vet_state IN ('no-claims', 'vetted-accurate', 'fixed')",
    )
    .fetch_one(&mut conn)
    .await?;
    let bucket_a_pct = if bucket_a_total > 0 {
        (bucket_a_terminal as f64) * 100.0 / (bucket_a_total as f64)
    } else {
        0.0
    };

    let needs_fix_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sections WHERE vet_state = 'needs-fix'")
            .fetch_one(&mut conn)
            .await?;

    println!("audit-docs status");
    println!("─────────────────");
    println!(
        "Bucket A: {bucket_a_terminal} / {bucket_a_total} sections vetted ({bucket_a_pct:.1}%)"
    );
    if needs_fix_count > 0 {
        println!("needs-fix outstanding: {needs_fix_count}");
    }

    let streak = compute_streak(&mut conn, Local::now().date_naive()).await?;
    println!("Streak: {streak} day(s)");
    println!();

    // Top 5 unvetted Bucket A sections, sorted by priority + flag count.
    println!("Top 5 unvetted Bucket A sections (next from queue):");
    let queue: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT s.id, d.path, s.heading,
                (SELECT COUNT(*) FROM staleness_flags WHERE section_id = s.id) AS flags
         FROM sections s
         JOIN docs d ON d.id = s.doc_id
         WHERE d.bucket = 'A'
           AND s.vet_state IN ('unvetted', 'in-review')
         ORDER BY d.priority, flags DESC, d.path, s.ordinal
         LIMIT 5",
    )
    .fetch_all(&mut conn)
    .await?;
    if queue.is_empty() {
        println!("  (none — Bucket A is empty; release-readiness gate is open)");
    } else {
        for (id, path, heading, flags) in queue {
            let flag_marker = if flags > 0 {
                format!(" [{flags} flags]")
            } else {
                String::new()
            };
            println!("  §{id:>5}{flag_marker}  {path} :: {heading}");
        }
    }

    Ok(())
}

/// Print just the streak count. Useful for shell prompts / status-line
/// integrations that don't want the full status output.
async fn run_streak(db: &Path) -> Result<()> {
    let mut conn = open_catalog(db).await?;
    apply_migrations(&mut conn).await?;
    let streak = compute_streak(&mut conn, Local::now().date_naive()).await?;
    println!("{streak}");
    Ok(())
}

/// Mark a section's vet_state. Records reviewer + reviewed_at; for
/// `fixed`, also records the fix commit hash. Verifies the verdict
/// is a known value before writing.
async fn run_vet(
    db: &Path,
    section_id: i64,
    verdict: &str,
    reviewer: Option<&str>,
    notes: Option<&str>,
    fix_commit: Option<&str>,
) -> Result<()> {
    const VALID_VERDICTS: &[&str] = &[
        "unvetted",
        "in-review",
        "no-claims",
        "vetted-accurate",
        "needs-fix",
        "fixed",
    ];
    if !VALID_VERDICTS.contains(&verdict) {
        return Err(format!(
            "invalid verdict '{verdict}'; expected one of: {}",
            VALID_VERDICTS.join(", ")
        )
        .into());
    }
    if verdict == "fixed" && fix_commit.is_none() {
        return Err("verdict 'fixed' requires --fix-commit".into());
    }

    let mut conn = open_catalog(db).await?;
    apply_migrations(&mut conn).await?;

    // Confirm the section exists; surface a clear error rather than
    // silently UPDATE-zero-rows.
    let exists: Option<(String, String)> = sqlx::query_as(
        "SELECT d.path, s.heading
         FROM sections s JOIN docs d ON d.id = s.doc_id
         WHERE s.id = ?",
    )
    .bind(section_id)
    .fetch_optional(&mut conn)
    .await?;
    let (path, heading) = exists.ok_or_else(|| format!("section §{section_id} not found"))?;

    let now = iso_now();
    sqlx::query(
        "UPDATE sections
         SET vet_state = ?,
             reviewer = COALESCE(?, reviewer),
             reviewed_at = ?,
             notes = COALESCE(?, notes),
             fix_commit_hash = COALESCE(?, fix_commit_hash)
         WHERE id = ?",
    )
    .bind(verdict)
    .bind(reviewer)
    .bind(&now)
    .bind(notes)
    .bind(fix_commit)
    .bind(section_id)
    .execute(&mut conn)
    .await?;

    println!("§{section_id} {path} :: {heading}");
    println!("  → {verdict} (at {now})");
    Ok(())
}

/// Count consecutive days with ≥1 vet (transition out of 'unvetted'
/// recorded in `reviewed_at`). Walks backward from `today`; stops at
/// the first day with no vet activity. Today counts whether or not
/// the day already has a vet — a fresh morning still has the prior
/// day's streak intact, encouraging "do today's section now."
///
/// `today` is injected (rather than read from `chrono::Local::now()`
/// internally) so tests can pin both sides of the day-boundary
/// comparison deterministically; the SQL extracts `reviewed_at`'s
/// **local** date via the `'localtime'` modifier so an evening EDT
/// vet (stored with a `-04:00` offset that normalizes to the next
/// UTC day) is attributed to the operator's local day, not UTC.
async fn compute_streak(conn: &mut SqliteConnection, today: NaiveDate) -> Result<i64> {
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT DATE(reviewed_at, 'localtime') AS d FROM sections
         WHERE reviewed_at IS NOT NULL
         ORDER BY d DESC",
    )
    .fetch_all(&mut *conn)
    .await?;
    if dates.is_empty() {
        return Ok(0);
    }

    let mut streak = 0i64;
    let mut cursor = today;
    for raw in dates {
        let parsed = match NaiveDate::parse_from_str(&raw, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => break,
        };
        // Allow today to be missing without breaking the streak; the
        // operator may not have vetted yet today but yesterday's
        // streak is still valid.
        if parsed == cursor {
            streak += 1;
            cursor = cursor.pred_opt().ok_or("date arithmetic underflow")?;
        } else if parsed == cursor.pred_opt().unwrap_or(cursor) && streak == 0 {
            // First iteration: today missing, yesterday present.
            streak = 1;
            cursor = parsed
                .pred_opt()
                .ok_or("date arithmetic underflow on yesterday")?;
        } else {
            break;
        }
    }
    Ok(streak)
}

// ---------------------------------------------------------------------------
// CLI dispatch
// ---------------------------------------------------------------------------

/// Parse `audit-docs` CLI flags. Catalog scope is talkbank-tools-only
/// since the v2 cutover; the meta-repo workspace contains historical
/// staleness and is no longer evidence.
///
/// Required env vars (each overridable by the matching `--flag`):
///   TB_AUDIT_DB       — path to the SQLite catalog (e.g.
///                       `<workspace>/docs/release-doc-audit/audit.db`)
///   TB_AUDIT_TT_ROOT  — path to the talkbank-tools clone being audited
///
/// The `vet` and `streak` subcommands need only `TB_AUDIT_DB`;
/// `scan` and `flag-staleness` also need `TB_AUDIT_TT_ROOT`. `status`
/// only reads from the catalog and needs only `TB_AUDIT_DB`.
pub fn parse_and_run(rest: Vec<String>) -> Result<()> {
    let usage = "usage: cargo run -q -p xtask -- audit-docs \
         <scan|flag-staleness|status|streak|vet> [args...]\n\
         common: [--db PATH] [--talkbank-tools PATH]\n\
         vet:    --id <section_id> --verdict <unvetted|in-review|no-claims|vetted-accurate|needs-fix|fixed> \
                 [--reviewer <name>] [--notes <text>] [--fix-commit <hash>]\n\
         (env: TB_AUDIT_DB, TB_AUDIT_TT_ROOT)";

    let sub = rest.first().map(|s| s.as_str()).ok_or(usage)?;

    let mut db: Option<PathBuf> = env::var_os("TB_AUDIT_DB").map(PathBuf::from);
    let mut tt_root: Option<PathBuf> = env::var_os("TB_AUDIT_TT_ROOT").map(PathBuf::from);
    let mut vet_id: Option<i64> = None;
    let mut vet_verdict: Option<String> = None;
    let mut vet_reviewer: Option<String> = None;
    let mut vet_notes: Option<String> = None;
    let mut vet_fix_commit: Option<String> = None;

    let mut iter = rest.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db" => {
                db = Some(PathBuf::from(iter.next().ok_or("--db requires a value")?));
            }
            "--talkbank-tools" => {
                tt_root = Some(PathBuf::from(
                    iter.next().ok_or("--talkbank-tools requires a value")?,
                ));
            }
            "--id" => {
                let raw = iter.next().ok_or("--id requires a value")?;
                vet_id = Some(raw.parse().map_err(|e| format!("--id: {e}"))?);
            }
            "--verdict" => {
                vet_verdict = Some(iter.next().ok_or("--verdict requires a value")?.clone());
            }
            "--reviewer" => {
                vet_reviewer = Some(iter.next().ok_or("--reviewer requires a value")?.clone());
            }
            "--notes" => {
                vet_notes = Some(iter.next().ok_or("--notes requires a value")?.clone());
            }
            "--fix-commit" => {
                vet_fix_commit = Some(iter.next().ok_or("--fix-commit requires a value")?.clone());
            }
            other => return Err(format!("unknown audit-docs flag: {other}").into()),
        }
    }

    let db = db.ok_or("audit-docs: --db or TB_AUDIT_DB is required (no default)")?;

    // Subcommands are async-on-sqlx; rest of xtask is sync, so spin
    // up a single-threaded tokio runtime and block on the result.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        match sub {
            "scan" => {
                let tt_root = tt_root
                    .ok_or("audit-docs scan: --talkbank-tools or TB_AUDIT_TT_ROOT required")?;
                run(Args {
                    db,
                    talkbank_tools_root: tt_root,
                })
                .await
            }
            "flag-staleness" => {
                let tt_root = tt_root.ok_or(
                    "audit-docs flag-staleness: --talkbank-tools or TB_AUDIT_TT_ROOT required",
                )?;
                run_flag_staleness(Args {
                    db,
                    talkbank_tools_root: tt_root,
                })
                .await
            }
            "status" => run_status(&db).await,
            "streak" => run_streak(&db).await,
            "vet" => {
                let id = vet_id.ok_or("audit-docs vet: --id required")?;
                let verdict = vet_verdict.ok_or("audit-docs vet: --verdict required")?;
                run_vet(
                    &db,
                    id,
                    &verdict,
                    vet_reviewer.as_deref(),
                    vet_notes.as_deref(),
                    vet_fix_commit.as_deref(),
                )
                .await
            }
            other => Err(format!("audit-docs: unknown subcommand '{other}'\n{usage}").into()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Executor;

    /// Regression test for the streak-counter TZ bug.
    ///
    /// `reviewed_at` is stored as a local-time-with-offset string
    /// (e.g. `"2026-05-11 20:39 -04:00"`). SQLite's `DATE()` on such a
    /// string normalizes the moment to UTC and returns the **UTC** date
    /// — so an evening EDT vet shows up under tomorrow's UTC date and
    /// the streak walk fails to match `today` (local).
    ///
    /// This test pins the process TZ to `America/New_York`, inserts one
    /// vet at the operator's local 20:39 (= 00:39 UTC the next day),
    /// then asks `compute_streak` whether that counts toward a streak
    /// for the local day `2026-05-11`. With the prior `DATE(reviewed_at)`
    /// SQL the answer is 0 (test fails red); with the
    /// `DATE(reviewed_at, 'localtime')` fix the answer is 1.
    /// RAII guard that pins the process `TZ` env var for the lifetime of
    /// the test and restores the prior value on drop, so a test that
    /// mutates TZ does not leak that mutation into sibling tests run
    /// from the same xtask binary.
    struct TzGuard {
        prior: Option<std::ffi::OsString>,
    }

    impl TzGuard {
        fn pin(value: &str) -> Self {
            let prior = std::env::var_os("TZ");
            // SAFETY: env mutation is `unsafe` in 2024 edition because
            // it races with concurrent reads; tokio tests are
            // serialized by default and the matching `Drop` impl
            // restores state before the next test starts.
            unsafe {
                std::env::set_var("TZ", value);
            }
            Self { prior }
        }
    }

    impl Drop for TzGuard {
        fn drop(&mut self) {
            // SAFETY: same justification as `pin`.
            unsafe {
                match &self.prior {
                    Some(p) => std::env::set_var("TZ", p),
                    None => std::env::remove_var("TZ"),
                }
            }
        }
    }

    #[tokio::test]
    async fn compute_streak_respects_local_time_boundary() -> Result<()> {
        let _tz = TzGuard::pin("America/New_York");

        let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
        conn.execute("CREATE TABLE sections (id INTEGER PRIMARY KEY, reviewed_at TEXT);")
            .await?;
        // Operator vetted at 2026-05-11 20:39 EDT, which iso_now() writes
        // as "2026-05-11 20:39 -04:00" — the exact format observed in the
        // live catalog. Without 'localtime', SQLite's DATE() returns
        // '2026-05-12' (UTC) for this moment.
        conn.execute(
            "INSERT INTO sections (id, reviewed_at) \
             VALUES (1, '2026-05-11 20:39 -04:00');",
        )
        .await?;

        let today = NaiveDate::from_ymd_opt(2026, 5, 11)
            .ok_or("test setup: NaiveDate::from_ymd_opt(2026, 5, 11) returned None")?;

        let streak = compute_streak(&mut conn, today).await?;
        assert_eq!(
            streak, 1,
            "vet at 2026-05-11 20:39 EDT (= 2026-05-12 00:39 UTC) must \
             count toward the today=2026-05-11 streak when SQLite \
             applies 'localtime' to reviewed_at"
        );
        Ok(())
    }
}
