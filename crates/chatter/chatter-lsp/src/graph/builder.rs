//! DOT graph builder for `%mor`/`%gra` dependency visualisation.
//!
//! Assembles a Graphviz DOT string from the `%mor` chunks (nodes) and `%gra`
//! relations (edges) of a single utterance. The graph uses invisible ordering
//! edges to maintain left-to-right word order, with coloured dependency arcs
//! showing head→dependent relations. The VS Code extension renders this via
//! the `talkbank/showDependencyGraph` command.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::alignment::GraAlignment;
use talkbank_model::model::GraTier;

use super::edges;
use super::labels::NodeLabel;
use crate::backend::{LspBackendError, ParseState};

/// Assemble a complete DOT graph from labeled nodes and `%gra` edges.
///
/// When `parse_state` is [`ParseState::StaleBaseline`], a muted
/// monospace `stale baseline` label is emitted at the top-left of the
/// graph so viewers can tell the dependency structure was computed
/// against the last successful parse rather than the current document
/// text (KIB-013).
pub(super) fn render_graph(
    node_labels: &[NodeLabel],
    gra_tier: &GraTier,
    gra_alignment: &GraAlignment,
    parse_state: ParseState,
) -> Result<String, LspBackendError> {
    let mut dot = String::new();

    append_header(&mut dot, parse_state);
    append_root_node(&mut dot);
    append_nodes(&mut dot, node_labels);
    edges::append_ordering_edges(&mut dot, node_labels);
    edges::append_dependency_edges(&mut dot, node_labels, gra_tier, gra_alignment)?;

    dot.push_str("}\n");
    Ok(dot)
}

/// Write graph-level DOT attributes and defaults.
fn append_header(dot: &mut String, parse_state: ParseState) {
    dot.push_str("digraph utterance {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  charset=\"UTF-8\";\n");
    dot.push_str("  fontname=\"DejaVu Sans, Noto Sans CJK SC, SimSun, Arial Unicode MS\";\n");
    if parse_state == ParseState::StaleBaseline {
        // Muted top-left label marks this as meta-information about
        // the rendering, not a dependency relation. Courier +
        // fontcolor=#888 keep it subordinate to the actual graph.
        dot.push_str("  label=\"stale baseline\";\n");
        dot.push_str("  labelloc=\"t\";\n");
        dot.push_str("  labeljust=\"l\";\n");
        dot.push_str("  fontcolor=\"#888888\";\n");
        dot.push_str("  fontname=\"Courier\";\n");
        dot.push_str("  fontsize=\"10\";\n");
    }
    dot.push_str("  node [shape=box, style=filled, fillcolor=white, fontname=\"DejaVu Sans, Noto Sans CJK SC, SimSun, Arial Unicode MS\", fontsize=11];\n");
    dot.push_str("  edge [fontname=\"DejaVu Sans, Noto Sans CJK SC, SimSun, Arial Unicode MS\", fontsize=10, color=\"#333333\"];\n\n");
}

/// Add an invisible ROOT anchor node for head index `0` relations.
fn append_root_node(dot: &mut String) {
    dot.push_str("  0 [label=\"ROOT\", shape=point, style=invis, width=0.1, height=0.1];\n\n");
}

/// Emit one DOT node per morphology chunk label.
fn append_nodes(dot: &mut String, node_labels: &[NodeLabel]) {
    for node in node_labels {
        dot.push_str(&format!(
            "  {} [label=\"{}\\n{}\"];\n",
            node.node_id, node.label, node.node_id
        ));
    }
    dot.push('\n');
}
