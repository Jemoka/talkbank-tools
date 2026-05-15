// `serde_json::to_string_pretty` / `to_value` is infallible on
// types that derive `Serialize` cleanly (no custom serializer can
// fail). All `CommandOutput` and `AnalysisResult` impls in this
// crate use derived `Serialize`.
#![allow(clippy::expect_used)]

//! Output formatting for analysis results.
//!
//! Provides the [`CommandOutput`] trait and supporting types for rendering
//! analysis results in multiple formats: human-readable text, structured JSON,
//! CSV for spreadsheet workflows, and CLAN-compatible output for character-level
//! matching with legacy CLAN binaries.
//!
//! Commands migrated to typed results define their own struct (e.g., `MluResult`,
//! `FreqResult`) implementing [`CommandOutput`]. The [`AnalysisResult`] container
//! serves as a bridge for commands not yet migrated to typed results.

use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;

/// Output format for analysis results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text (our own clean format)
    Text,
    /// Structured JSON for programmatic consumption
    Json,
    /// CSV for spreadsheet workflows
    Csv,
    /// CLAN-compatible output (character-for-character match with legacy CLAN)
    Clan,
}

/// Trait for typed command output that can be rendered in multiple formats.
///
/// Each command defines its own result struct (e.g., `MluResult`, `FreqResult`)
/// implementing this trait. The trait provides format-dispatched rendering via
/// [`render()`](CommandOutput::render), with per-format hooks that implementors
/// can override.
///
/// # Default implementations
///
/// - `render()` dispatches to the per-format method
/// - `render_clan()` falls back to `render_text()` for commands not yet CLAN-matched
/// - `render_csv()` returns an empty string by default
pub trait CommandOutput: Serialize + std::fmt::Debug {
    /// Render in the specified format.
    fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self.render_text(),
            OutputFormat::Json => {
                serde_json::to_string_pretty(self).expect("CommandOutput serialization cannot fail")
            }
            OutputFormat::Csv => self.render_csv(),
            OutputFormat::Clan => self.render_clan(),
        }
    }

    /// Serialize directly into JSON values for programmatic consumers.
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("CommandOutput serialization cannot fail")
    }

    /// Our clean text format (default rendering).
    fn render_text(&self) -> String;

    /// CLAN-compatible output (character-for-character match with legacy CLAN).
    ///
    /// Default: falls back to `render_text()` for commands not yet CLAN-matched.
    fn render_clan(&self) -> String {
        self.render_text()
    }

    /// CSV rendering. Default: empty string.
    fn render_csv(&self) -> String {
        String::new()
    }
}

/// A single row in a table section.
#[derive(Debug, Clone, Serialize)]
pub struct TableRow {
    /// Column values in order
    pub values: Vec<String>,
}

/// A section of analysis output.
///
/// Sections can contain key-value summaries (like "Total words: 42")
/// or tabular data (like frequency tables).
#[derive(Debug, Clone, Serialize)]
pub struct Section {
    /// Section heading (e.g., "Speaker: CHI", "Summary")
    pub heading: String,
    /// Key-value pairs (e.g., "Total words" => "42")
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub fields: IndexMap<String, String>,
    /// Column headers for tabular data
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// Table rows
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<TableRow>,
}

impl Section {
    /// Create a section with only key-value fields.
    pub fn with_fields(heading: impl Into<String>, fields: IndexMap<String, String>) -> Self {
        Self {
            heading: heading.into(),
            fields,
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Create a section with tabular data.
    pub fn with_table(
        heading: impl Into<String>,
        columns: Vec<String>,
        rows: Vec<TableRow>,
    ) -> Self {
        Self {
            heading: heading.into(),
            fields: IndexMap::new(),
            columns,
            rows,
        }
    }
}

/// Structured output from an analysis command.
///
/// Contains a command name, one or more sections of data, and can
/// be rendered in multiple output formats.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResult {
    /// Name of the command that produced this result (e.g., "freq", "mlu")
    pub command: String,
    /// Output sections (per-speaker, summary, etc.)
    pub sections: Vec<Section>,
}

impl AnalysisResult {
    /// Create a new result for the given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            sections: Vec::new(),
        }
    }

    /// Add a section to the result.
    pub fn add_section(&mut self, section: Section) {
        self.sections.push(section);
    }

    /// Render the result in the specified format.
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text | OutputFormat::Clan => self.render_text_impl(),
            OutputFormat::Json => self.render_json(),
            OutputFormat::Csv => self.render_csv_impl(),
        }
    }

    /// Render as human-readable text.
    fn render_text_impl(&self) -> String {
        let mut out = String::new();
        for (i, section) in self.sections.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            render_text_section(&mut out, section);
        }
        out
    }

    /// Render as JSON.
    fn render_json(&self) -> String {
        // Unwrap is safe: AnalysisResult contains only serializable types
        serde_json::to_string_pretty(self).expect("AnalysisResult serialization cannot fail")
    }

    /// Render as CSV.
    ///
    /// For sections with tables, emits header row + data rows.
    /// For sections with fields, emits key,value rows.
    fn render_csv_impl(&self) -> String {
        let mut out = String::new();
        for section in &self.sections {
            if !section.columns.is_empty() {
                out.push_str(&section.columns.join(","));
                out.push('\n');
                for row in &section.rows {
                    out.push_str(&row.values.join(","));
                    out.push('\n');
                }
            } else if !section.fields.is_empty() {
                for (key, value) in &section.fields {
                    out.push_str(&csv_escape(key));
                    out.push(',');
                    out.push_str(&csv_escape(value));
                    out.push('\n');
                }
            }
        }
        out
    }
}

/// Bridge implementation: `AnalysisResult` implements `CommandOutput` so that
/// commands still using the stringly-typed container work with the new trait.
impl CommandOutput for AnalysisResult {
    /// Render the bridge type through the shared text renderer.
    fn render_text(&self) -> String {
        self.render_text_impl()
    }

    /// Render the bridge type through the shared CSV renderer.
    fn render_csv(&self) -> String {
        self.render_csv_impl()
    }
}

/// Render a single section as human-readable text.
fn render_text_section(out: &mut String, section: &Section) {
    // Section heading
    fmt::write(out, format_args!("{}\n", section.heading)).ok();

    // Key-value fields
    for (key, value) in &section.fields {
        fmt::write(out, format_args!("  {key}: {value}\n")).ok();
    }

    // Table
    if !section.columns.is_empty() {
        // Calculate column widths for alignment
        let mut widths: Vec<usize> = section.columns.iter().map(|c| c.len()).collect();
        for row in &section.rows {
            for (i, val) in row.values.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(val.len());
                }
            }
        }

        // Header
        let header: Vec<String> = section
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| format!("{col:<width$}", width = widths[i]))
            .collect();
        fmt::write(out, format_args!("  {}\n", header.join("  "))).ok();

        // Separator
        let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        fmt::write(out, format_args!("  {}\n", sep.join("  "))).ok();

        // Rows
        for row in &section.rows {
            let cells: Vec<String> = row
                .values
                .iter()
                .enumerate()
                .map(|(i, val)| {
                    let width = widths.get(i).copied().unwrap_or(0);
                    format!("{val:<width$}")
                })
                .collect();
            fmt::write(out, format_args!("  {}\n", cells.join("  "))).ok();
        }
    }
}

/// Escape a value for CSV output.
///
/// Wraps in double quotes if the value contains a comma, newline, or double quote.
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('\n') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text rendering should print section headings and key/value fields.
    #[test]
    fn text_rendering_fields() {
        let mut result = AnalysisResult::new("test");
        let mut fields = IndexMap::new();
        fields.insert("Count".to_owned(), "42".to_owned());
        fields.insert("Mean".to_owned(), "3.5".to_owned());
        result.add_section(Section::with_fields("Summary", fields));

        let text = result.render(OutputFormat::Text);
        assert!(text.contains("Summary"));
        assert!(text.contains("Count: 42"));
        assert!(text.contains("Mean: 3.5"));
    }

    /// Text rendering should preserve table headers and row values.
    #[test]
    fn text_rendering_table() {
        let mut result = AnalysisResult::new("test");
        result.add_section(Section::with_table(
            "Words",
            vec!["Word".to_owned(), "Count".to_owned()],
            vec![
                TableRow {
                    values: vec!["hello".to_owned(), "5".to_owned()],
                },
                TableRow {
                    values: vec!["world".to_owned(), "3".to_owned()],
                },
            ],
        ));

        let text = result.render(OutputFormat::Text);
        assert!(text.contains("Word"));
        assert!(text.contains("Count"));
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
    }

    /// JSON rendering should be valid and include top-level command metadata.
    #[test]
    fn json_rendering_roundtrips() {
        let mut result = AnalysisResult::new("freq");
        let mut fields = IndexMap::new();
        fields.insert("Total".to_owned(), "10".to_owned());
        result.add_section(Section::with_fields("Summary", fields));

        let json = result.render(OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON should be valid");
        assert_eq!(parsed["command"], "freq");
    }

    /// Direct JSON-value conversion should match the rendered JSON structure.
    #[test]
    fn json_value_matches_rendered_json() {
        let mut result = AnalysisResult::new("freq");
        let mut fields = IndexMap::new();
        fields.insert("Total".to_owned(), "10".to_owned());
        result.add_section(Section::with_fields("Summary", fields));

        let rendered_json = result.render(OutputFormat::Json);
        let parsed_rendered: serde_json::Value =
            serde_json::from_str(&rendered_json).expect("rendered JSON should parse");

        assert_eq!(result.to_json_value(), parsed_rendered);
    }

    /// CSV escaping should quote commas and embedded quotes.
    #[test]
    fn csv_escaping() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
