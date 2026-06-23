//! Metrics writer (spec2.md §20).
//!
//! Spec authored the original draft against polars. This crate keeps the
//! engine lean: the core crate exposes `MetricsArtifact` with a long-format
//! `MetricsTable` (header + rows of stringified cells), and we render CSV
//! directly. No polars dep here; downstream tooling can still re-read the
//! file with polars or pandas.
//!
//! Picks file extension per `MetricsKind` (`compare.csv`, `benchmark.csv`,
//! `metrics.csv`). The given `path` is the base; we re-extension it.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use batchalign_core::{MetricsArtifact, MetricsKind};
use serde_json::Value as JsonValue;

/// Render a single JSON value as a CSV cell.
fn json_cell(v: &JsonValue) -> String {
    match v {
        JsonValue::Null => String::new(),
        JsonValue::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// File extension picker per metrics kind. Mirrors the spec exactly.
fn extension_for(kind: MetricsKind) -> &'static str {
    match kind {
        MetricsKind::Compare => "compare.csv",
        MetricsKind::Benchmark => "benchmark.csv",
        MetricsKind::Custom => "metrics.csv",
    }
}

/// Writes `artifact` to `path.<kind-extension>` in CSV form.
///
/// CSV escaping is minimal-but-correct (RFC 4180): fields containing
/// `,`, `"`, newlines, or carriage returns are quoted; embedded `"` is
/// doubled. UTF-8 throughout. No BOM (downstream tools shouldn't need one).
pub fn write(artifact: &MetricsArtifact, path: &Path) -> Result<()> {
    let target = path.with_extension(extension_for(artifact.kind));
    let file = File::create(&target)
        .with_context(|| format!("metrics_writer: create {}", target.display()))?;
    let mut out = BufWriter::new(file);

    let table = &artifact.table;
    write_csv_row(&mut out, &table.schema)?;
    for row in &table.rows {
        let cells: Vec<String> = table
            .schema
            .iter()
            .map(|col| row.columns.get(col).map(json_cell).unwrap_or_default())
            .collect();
        write_csv_row(&mut out, &cells)?;
    }
    out.flush()
        .with_context(|| format!("metrics_writer: flush {}", target.display()))?;
    Ok(())
}

fn write_csv_row<W: Write>(out: &mut W, cells: &[String]) -> Result<()> {
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.write_all(b",")?;
        }
        write_csv_cell(out, cell)?;
    }
    out.write_all(b"\n")?;
    Ok(())
}

fn write_csv_cell<W: Write>(out: &mut W, cell: &str) -> Result<()> {
    let needs_quotes = cell
        .chars()
        .any(|c| c == ',' || c == '"' || c == '\n' || c == '\r');
    if needs_quotes {
        out.write_all(b"\"")?;
        for c in cell.chars() {
            if c == '"' {
                out.write_all(b"\"\"")?;
            } else {
                // small per-char write; cells are typically short
                let mut buf = [0u8; 4];
                out.write_all(c.encode_utf8(&mut buf).as_bytes())?;
            }
        }
        out.write_all(b"\"")?;
    } else {
        out.write_all(cell.as_bytes())?;
    }
    Ok(())
}
