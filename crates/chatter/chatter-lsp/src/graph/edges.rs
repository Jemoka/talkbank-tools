//! DOT edge emitters — ordering constraints and dependency arcs.
//!
//! Two kinds of edges: invisible `ordering` edges maintain left-to-right word
//! order in the graph layout, and coloured `dependency` edges represent `%gra`
//! grammatical relations (SUBJ, OBJ, ROOT, etc.) between `%mor` chunks.

use talkbank_model::alignment::{GraAlignment, GraHeadRef, GraIndex};
use talkbank_model::model::GraTier;

use super::error::GraphEdgeError;
use super::labels::NodeLabel;

/// Add invisible left-to-right ordering edges between adjacent chunks.
pub(super) fn append_ordering_edges(dot: &mut String, node_labels: &[NodeLabel]) {
    for i in 0..node_labels.len() {
        let curr = node_labels[i].node_id;
        if i + 1 < node_labels.len() {
            let next = node_labels[i + 1].node_id;
            dot.push_str(&format!("  {} -> {} [style=invis];\n", curr, next));
        }
    }
    dot.push('\n');
}

/// Add labeled dependency arcs from `%gra` relations using alignment metadata.
pub(super) fn append_dependency_edges(
    dot: &mut String,
    node_labels: &[NodeLabel],
    gra_tier: &GraTier,
    gra_alignment: &GraAlignment,
) -> Result<(), GraphEdgeError> {
    for (rel_idx, rel) in gra_tier.relations().iter().enumerate() {
        let relation = &rel.relation;

        let from_node = match gra_alignment
            .pairs
            .iter()
            .find(|p| p.gra_index == Some(GraIndex::new(rel_idx)))
        {
            Some(pair) => match pair.mor_chunk_index {
                Some(mor_chunk_idx) => {
                    let chunk_index = mor_chunk_idx.as_usize();
                    node_id_for_chunk(node_labels, chunk_index)
                        .ok_or(GraphEdgeError::InvalidChunkIndex { chunk_index })?
                }
                None => continue,
            },
            None => {
                return Err(GraphEdgeError::MissingAlignmentPair { rel_index: rel_idx });
            }
        };

        // Head resolution goes through `head_ref()` so the ROOT sentinel
        // (`head == 0` in CHAT) is handled explicitly as its own variant
        // rather than via an inline magic-number comparison. DOT node id
        // 0 is reserved for the virtual ROOT node the graph builder emits.
        let to_node = match rel.head_ref() {
            GraHeadRef::Root => 0,
            GraHeadRef::Word(head_idx) => {
                let chunk_index = head_idx.to_chunk_index().as_usize();
                node_id_for_chunk(node_labels, chunk_index).ok_or(
                    GraphEdgeError::InvalidHeadIndex {
                        head: head_idx.as_usize(),
                        chunk_index,
                    },
                )?
            }
        };

        let color = relation_color(relation);

        dot.push_str(&format!(
            "  {} -> {} [label=\"{}\", color=\"{}\", fontcolor=\"{}\", constraint=false];\n",
            from_node, to_node, relation, color, color
        ));
    }

    Ok(())
}

/// Resolve the DOT node id corresponding to an aligned morphology chunk index.
fn node_id_for_chunk(node_labels: &[NodeLabel], chunk_idx: usize) -> Option<usize> {
    node_labels
        .iter()
        .find(|node| node.chunk_idx == chunk_idx)
        .map(|node| node.node_id)
}

/// Pick a stable color for a `%gra` relation label.
fn relation_color(relation: &str) -> &'static str {
    match relation {
        "SUBJ" => "#4a90e2",
        "OBJ" | "OBJ2" => "#e24a4a",
        "ROOT" => "#50c878",
        "JCT" => "#f5a623",
        "MOD" => "#9b59b6",
        "DET" => "#3498db",
        "QUANT" => "#16a085",
        "AUX" => "#e67e22",
        "COMP" => "#c0392b",
        "COORD" => "#27ae60",
        _ => "#95a5a6",
    }
}
