//! Output formatting for `chatter debug find`.
//!
//! Three formats:
//!
//! - [`FindOutputFormat::Paths`] — one file path per line. Ideal for
//!   piping into `xargs`, `fzf`, or similar shell tools.
//! - [`FindOutputFormat::Jsonl`] — one JSON object per line with full
//!   scan metadata. Stable schema for downstream typed consumers
//!   (e.g. the L2 morphotag evaluator in batchalign3).
//! - [`FindOutputFormat::Csv`] — tabular, for spreadsheet review.
//!
//! Sort order is independent of format: see [`FindSortOrder`].

use std::io::{self, Write};

use clap::ValueEnum;
use serde::Serialize;

use super::scanner::ChatFileScan;

/// Output format selector.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum FindOutputFormat {
    /// One path per line (default).
    #[default]
    Paths,
    /// JSON Lines with full per-file metadata.
    Jsonl,
    /// CSV with header row.
    Csv,
}

/// Output sort order.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum FindSortOrder {
    /// Sort by path lexicographically (default; deterministic).
    #[default]
    Path,
    /// Sort by token count descending (most interesting files first).
    TokenCountDesc,
}

/// Stable on-disk schema mirrored by `jsonl` and `csv` output.
#[derive(Debug, Serialize)]
struct FindRecord<'a> {
    path: &'a str,
    /// Comma-separated `@Languages` codes in header order.
    languages: String,
    at_s_count: u32,
    utterance_count: u32,
    file_bytes: u64,
}

impl<'a> FindRecord<'a> {
    fn from_scan(scan: &'a ChatFileScan) -> Self {
        let languages = scan
            .languages
            .iter()
            .map(|c| c.as_str().to_string())
            .collect::<Vec<_>>()
            .join(",");
        Self {
            path: scan.path.to_str().unwrap_or("<non-utf8 path>"),
            languages,
            at_s_count: scan.token_count.get(),
            utterance_count: scan.utterance_count.get(),
            file_bytes: scan.file_bytes,
        }
    }
}

/// Writes the scan results in the requested format. CSV and JSONL
/// formats use a stable column/field order so downstream tooling can
/// parse them without discovery.
pub fn write_results<W: Write>(
    writer: &mut W,
    format: FindOutputFormat,
    scans: &[ChatFileScan],
) -> io::Result<()> {
    match format {
        FindOutputFormat::Paths => write_paths(writer, scans),
        FindOutputFormat::Jsonl => write_jsonl(writer, scans),
        FindOutputFormat::Csv => write_csv(writer, scans),
    }
}

fn write_paths<W: Write>(writer: &mut W, scans: &[ChatFileScan]) -> io::Result<()> {
    for scan in scans {
        writeln!(
            writer,
            "{}",
            scan.path.to_str().unwrap_or("<non-utf8 path>")
        )?;
    }
    Ok(())
}

fn write_jsonl<W: Write>(writer: &mut W, scans: &[ChatFileScan]) -> io::Result<()> {
    for scan in scans {
        let record = FindRecord::from_scan(scan);
        let line = serde_json::to_string(&record).map_err(io::Error::other)?;
        writeln!(writer, "{}", line)?;
    }
    Ok(())
}

fn write_csv<W: Write>(writer: &mut W, scans: &[ChatFileScan]) -> io::Result<()> {
    // Stable column order.
    writeln!(
        writer,
        "path,languages,at_s_count,utterance_count,file_bytes"
    )?;
    for scan in scans {
        let record = FindRecord::from_scan(scan);
        // Escape quotes in path and languages by doubling; wrap path in quotes
        // because paths may contain commas.
        let path_esc = record.path.replace('"', "\"\"");
        let lang_esc = record.languages.replace('"', "\"\"");
        writeln!(
            writer,
            "\"{}\",\"{}\",{},{},{}",
            path_esc, lang_esc, record.at_s_count, record.utterance_count, record.file_bytes
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod output_tests {
    use super::*;
    use crate::commands::find::scanner::{TokenPatternCount, UtteranceLineCount};
    use std::path::PathBuf;
    use talkbank_model::LanguageCodes;
    use talkbank_model::model::LanguageCode;

    fn sample_scan(path: &str, languages: &[&str], at_s: u32) -> ChatFileScan {
        ChatFileScan {
            path: PathBuf::from(path),
            languages: LanguageCodes::new(
                languages.iter().map(|c| LanguageCode::new(*c)).collect(),
            ),
            utterance_count: UtteranceLineCount(12),
            token_count: TokenPatternCount(at_s),
            file_bytes: 4096,
        }
    }

    fn to_string(format: FindOutputFormat, scans: &[ChatFileScan]) -> String {
        let mut buf = Vec::new();
        write_results(&mut buf, format, scans).expect("write ok");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn paths_format_emits_one_path_per_line() {
        let scans = [
            sample_scan("a.cha", &["eng"], 0),
            sample_scan("b.cha", &["spa", "eng"], 3),
        ];
        let out = to_string(FindOutputFormat::Paths, &scans);
        assert_eq!(out, "a.cha\nb.cha\n");
    }

    #[test]
    fn jsonl_format_is_parseable() {
        let scans = [sample_scan("/tmp/x.cha", &["spa", "eng"], 7)];
        let out = to_string(FindOutputFormat::Jsonl, &scans);
        let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("valid json");
        assert_eq!(parsed["path"], "/tmp/x.cha");
        assert_eq!(parsed["languages"], "spa,eng");
        assert_eq!(parsed["at_s_count"], 7);
        assert_eq!(parsed["utterance_count"], 12);
    }

    #[test]
    fn csv_format_has_stable_header_and_quotes_path() {
        let scans = [sample_scan("/tmp/with,comma.cha", &["eng"], 0)];
        let out = to_string(FindOutputFormat::Csv, &scans);
        let mut lines = out.lines();
        assert_eq!(
            lines.next(),
            Some("path,languages,at_s_count,utterance_count,file_bytes")
        );
        assert_eq!(
            lines.next(),
            Some("\"/tmp/with,comma.cha\",\"eng\",0,12,4096")
        );
    }
}
