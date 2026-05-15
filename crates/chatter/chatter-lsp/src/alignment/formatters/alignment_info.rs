//! Markdown table formatter for [`AlignmentHoverInfo`].
//!
//! Produces a compact two-column Markdown table (field | value) suitable for
//! LSP hover popups. Fields with empty values are omitted to keep the card
//! minimal.

use crate::alignment::types::AlignmentHoverInfo;

/// Format alignment hover info as a compact Markdown table for LSP hover display.
///
/// Layout:
/// ```text
/// **element** — *Element Type*
///
/// | tier    | annotation |
/// |---------|------------|
/// | ← main  | …          |
/// | %mor    | …          |
/// | %gra    | …          |
///
/// **Key**: value · **Key**: value
/// ```
///
/// The table is omitted when no tier alignments are present.
/// Details (key-value pairs) are inlined as bold labels separated by ` · `.
pub fn format_alignment_info(info: &AlignmentHoverInfo) -> String {
    let mut md = String::new();

    // Header line: bold element content + italic element type label.
    md.push_str(&format!(
        "**{}** — *{}*\n\n",
        info.element_content, info.element_type
    ));

    // Alignment table — only emitted when at least one tier has data.
    let rows: Vec<(&str, &str)> = [
        ("← main", info.aligned_to_main.as_deref()),
        ("%mor", info.aligned_to_mor.as_deref()),
        ("%gra", info.aligned_to_gra.as_deref()),
        ("%pho", info.aligned_to_pho.as_deref()),
        ("%mod", info.aligned_to_mod.as_deref()),
        ("%sin", info.aligned_to_sin.as_deref()),
    ]
    .iter()
    .filter_map(|(label, opt)| opt.map(|val| (*label, val)))
    .collect();

    if !rows.is_empty() {
        md.push_str("| tier | annotation |\n");
        md.push_str("|------|------------|\n");
        for (label, val) in &rows {
            md.push_str(&format!("| {} | {} |\n", label, val));
        }
        md.push('\n');
    }

    // Additional details as compact inline key · value pairs.
    if !info.details.is_empty() {
        let detail_parts: Vec<String> = info
            .details
            .iter()
            .map(|(k, v)| format!("**{}**: {}", k, v))
            .collect();
        md.push_str(&detail_parts.join(" · "));
        md.push('\n');
    }

    md
}
