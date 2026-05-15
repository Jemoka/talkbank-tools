//! Semantic-diff report storage and rendering helpers.
//!
//! `SemanticDiffReport` serves as both collector and renderer for structured
//! differences. It is designed for assertion failures, roundtrip debugging, and
//! CLI output where path-aware mismatch context is more useful than `==` failure.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::fmt;
use std::fmt::Write as _;

use super::context::SemanticDiffContext;
use super::path::SemanticPath;
use super::source_utils::{extract_line_by_number, extract_line_context, span_snippet};
use super::types::{DEFAULT_MAX_DIFFS, SemanticDiffKind, SemanticDifference};
use crate::Span;

/// Collect and render semantic differences between two model values.
///
/// The report keeps insertion order so the first discovered mismatch remains
/// deterministic across runs, which is important for snapshot-based tests.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct SemanticDiffReport {
    differences: Vec<SemanticDifference>,
    truncated: bool,
    max_diffs: usize,
}

impl SemanticDiffReport {
    /// Creates a new [`SemanticDiffReport`] that will collect at most `max_diffs` differences.
    ///
    /// Truncation is intentional: it prevents runaway diff output on deeply
    /// divergent structures while still surfacing representative failures.
    pub fn new(max_diffs: usize) -> Self {
        Self {
            differences: Vec::new(),
            truncated: false,
            max_diffs,
        }
    }

    /// Returns `true` if no differences were recorded.
    ///
    /// This is the fastest way for callers to treat the report as a pass/fail
    /// signal before requesting any formatted output.
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }

    /// Returns `true` if the report was truncated because `max_diffs` was reached.
    ///
    /// A truncated report is still valid, but consumers should avoid assuming it
    /// enumerates every mismatch.
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Returns the collected differences.
    ///
    /// The slice is ordered by discovery order during semantic traversal.
    pub fn differences(&self) -> &[SemanticDifference] {
        &self.differences
    }

    /// Record one difference unless the report is already truncated.
    ///
    /// Once `max_diffs` is reached, subsequent pushes only mark truncation and
    /// intentionally drop additional entries.
    pub fn push(
        &mut self,
        path: &SemanticPath,
        kind: SemanticDiffKind,
        left: impl Into<String>,
        right: impl Into<String>,
        span: Option<Span>,
    ) {
        if self.differences.len() >= self.max_diffs {
            self.truncated = true;
            return;
        }

        self.differences.push(SemanticDifference {
            path: path.to_string(),
            kind,
            left: left.into(),
            right: right.into(),
            span,
        });
    }

    /// Records a difference using the span from the given [`SemanticDiffContext`].
    ///
    /// This helper keeps call sites concise and ensures span provenance follows
    /// the current traversal context consistently.
    pub fn push_with_context(
        &mut self,
        path: &SemanticPath,
        kind: SemanticDiffKind,
        left: impl Into<String>,
        right: impl Into<String>,
        ctx: &SemanticDiffContext,
    ) {
        self.push(path, kind, left, right, ctx.current_span());
    }

    /// Returns a one-line summary of the first difference, or a "no differences" message.
    ///
    /// The summary is intended for quick logs and assertion failures where
    /// full reports would be too noisy.
    pub fn short_summary(&self) -> String {
        if self.differences.is_empty() {
            return "no semantic differences detected".to_string();
        }
        let first = &self.differences[0];
        format!(
            "first diff at {} ({} vs {})",
            first.path, first.left, first.right
        )
    }

    /// Returns a one-line summary of the first difference with source location info.
    ///
    /// When span data is available, this adds byte offsets and a short snippet
    /// to make debugging parser regressions faster.
    pub fn short_summary_with_source(&self, source: &str) -> String {
        if self.differences.is_empty() {
            return "no semantic differences detected".to_string();
        }
        let first = &self.differences[0];
        let mut summary = format!(
            "first diff at {} ({} vs {})",
            first.path, first.left, first.right
        );
        if let Some(span) = first.span {
            summary.push_str(&format!(" at bytes {}..{}", span.start, span.end));
            if let Some(snippet) = span_snippet(source, span) {
                summary.push_str(&format!(" ({}: {})", snippet.location, snippet.line));
            }
        }
        summary
    }

    /// Renders all differences as a multi-line plain text report.
    ///
    /// This format is optimized for terminal output and CI logs where a compact
    /// but human-readable diff overview is needed.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Semantic Diff Report\n");
        out.push_str(&format!(
            "Differences: {}{}\n",
            self.differences.len(),
            if self.truncated { " (truncated)" } else { "" }
        ));

        if let Some(first) = self.differences.first() {
            out.push_str(&format!(
                "First diff: {} [{}]
  left:  {}
  right: {}\n",
                first.path,
                first.kind.as_str(),
                first.left,
                first.right
            ));
            if let Some(span) = first.span {
                out.push_str(&format!("  span:  {}..{}\n", span.start, span.end));
            }
        }

        out.push_str(
            "
Differences (first ",
        );
        out.push_str(&self.max_diffs.to_string());
        out.push_str("):\n");

        for (idx, diff) in self.differences.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} [{}]
   left:  {}
   right: {}\n",
                idx + 1,
                diff.path,
                diff.kind.as_str(),
                diff.left,
                diff.right
            ));
            if let Some(span) = diff.span {
                out.push_str(&format!("   span:  {}..{}\n", span.start, span.end));
            }
        }

        out
    }

    /// Renders all differences with source span snippets and caret markers.
    ///
    /// Use this when investigating concrete transcript offsets; it trades output
    /// size for direct source context around each mismatch.
    pub fn render_with_source(&self, source: &str) -> String {
        let mut out = String::new();
        out.push_str("Semantic Diff Report (with spans)\n");
        out.push_str(&format!(
            "Differences: {}{}\n",
            self.differences.len(),
            if self.truncated { " (truncated)" } else { "" }
        ));

        for (idx, diff) in self.differences.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} [{}]
   left:  {}
   right: {}\n",
                idx + 1,
                diff.path,
                diff.kind.as_str(),
                diff.left,
                diff.right
            ));
            if let Some(span) = diff.span {
                out.push_str(&format!("   span: {}..{}\n", span.start, span.end));
                if let Some(snippet) = span_snippet(source, span) {
                    out.push_str(&format!("   at:   {}\n", snippet.location));
                    out.push_str(&format!("   text: {}\n", snippet.line));
                    out.push_str(&format!("         {}\n", snippet.caret));
                }
            }
        }

        out
    }

    /// Render a comparison showing both original and serialized CHAT.
    ///
    /// This specialized renderer is designed for roundtrip debugging where
    /// differences need to be mapped back to original/serialized line pairs.
    pub fn render_comparison(
        &self,
        filename: &str,
        original_source: &str,
        serialized_source: &str,
    ) -> String {
        if self.differences.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        for diff in &self.differences {
            // Build message
            let message = format!(
                "{} at `{}`",
                diff.kind.as_str().replace('_', " "),
                diff.path
            );
            let _ = writeln!(output, "DIFF in {}: {}", filename, message);

            // Add explanation note
            let note = match diff.kind {
                SemanticDiffKind::LengthMismatch => {
                    "array/vector length changed - overlap markers may have been grouped differently"
                }
                SemanticDiffKind::ValueMismatch => "value changed during roundtrip",
                SemanticDiffKind::MissingKey => "key present in original, missing after roundtrip",
                SemanticDiffKind::ExtraKey => "new key appeared after roundtrip",
                SemanticDiffKind::VariantMismatch => {
                    "enum variant changed (e.g., Word → OverlapGroup)"
                }
            };
            let _ = writeln!(output, "  Note: {}", note);

            if let Some(span) = diff.span
                && let Some(ctx) = extract_line_context(original_source, span)
            {
                let _ = writeln!(
                    output,
                    "  Original (line {}): {}",
                    ctx.line_num, ctx.line_content
                );
                if let Some(snippet) = span_snippet(original_source, span) {
                    let _ = writeln!(output, "  {}", snippet.caret);
                }
                if let Some(ser_line) = extract_line_by_number(serialized_source, ctx.line_num) {
                    let _ = writeln!(output, "  Serialized: {}", ser_line);
                }
            }

            let _ = writeln!(output, "  Model: {} → {}", diff.left, diff.right);
            let _ = writeln!(output);
        }

        if self.truncated {
            let mut output = output;
            let _ = writeln!(
                output,
                "... stopping at first difference (subsequent diffs likely off-by-one)"
            );
            return output;
        }

        output
    }

    /// Render a short summary for the first difference (for inline display).
    pub fn render_comparison_short(
        &self,
        _filename: &str,
        original_source: &str,
        serialized_source: &str,
    ) -> String {
        use std::fmt::Write;

        if self.differences.is_empty() {
            return "no semantic differences detected".to_string();
        }

        let diff = &self.differences[0];
        let mut out = String::new();

        // Header line
        writeln!(
            out,
            "{} at `{}`",
            diff.kind.as_str().replace('_', " ").to_uppercase(),
            diff.path
        )
        .ok();

        // Show original context
        if let Some(span) = diff.span
            && let Some(ctx) = extract_line_context(original_source, span)
        {
            writeln!(
                out,
                "  Original (line {}): {}",
                ctx.line_num, ctx.line_content
            )
            .ok();

            // Show serialized line at same position
            if let Some(ser_line) = extract_line_by_number(serialized_source, ctx.line_num) {
                writeln!(out, "  Serialized:         {}", ser_line).ok();
            }
        }

        // Show model values
        writeln!(out, "  Model: {} → {}", diff.left, diff.right).ok();

        out
    }

    /// Render differences as a hierarchical tree visualization.
    ///
    /// Shows the path to each difference in tree form with:
    /// - Indentation showing hierarchy
    /// - Symbols indicating difference type (✗ value, ⊘ missing, ↔ type, etc.)
    /// - Left vs. right values
    /// - Source span if available
    ///
    /// # Arguments
    /// * `max_depth` - Optional limit on tree depth (None = unlimited)
    /// * `show_spans` - Whether to include byte offset information
    /// * `compact` - Collapse unchanged intermediate nodes
    ///
    /// # Returns
    /// Multi-line string with tree-formatted differences
    ///
    /// # Example Output
    /// ```text
    /// Semantic Differences (2 found):
    ///
    /// ChatFile
    /// ├─ lines[7]
    /// │  └─ utterance
    /// │     └─ main
    /// │        └─ content[1]
    /// │           └─ word
    /// │              └─ ✗ cleaned_text [bytes 191..198]
    /// │                   TreeSitter: "aubg"
    /// │                   Direct:     "au^bg"
    /// ```
    pub fn render_tree_diff(
        &self,
        max_depth: Option<usize>,
        show_spans: bool,
        mode: super::RenderMode,
    ) -> String {
        use crate::model::semantic_diff::tree_renderer::{TreeNode, TreeRenderer};

        if self.differences.is_empty() {
            return "No semantic differences detected.\n".to_string();
        }

        let mut out = String::new();
        out.push_str(&format!(
            "Semantic Differences ({} found):\n\n",
            self.differences.len()
        ));

        // Build tree from paths
        let tree = TreeNode::from_differences(&self.differences);

        // Render tree
        let renderer = TreeRenderer::new(max_depth, show_spans, mode);
        out.push_str(&renderer.render(&tree));

        if self.truncated {
            out.push_str("\n... (truncated, more differences exist)\n");
        }

        out
    }
}

impl Default for SemanticDiffReport {
    /// Uses the crate default diff cap (`DEFAULT_MAX_DIFFS`).
    fn default() -> Self {
        Self::new(DEFAULT_MAX_DIFFS)
    }
}

impl fmt::Display for SemanticDiffReport {
    /// Renders the summary-oriented diff text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}
