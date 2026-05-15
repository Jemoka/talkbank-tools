//! Dependency graph generation for %mor and %gra tiers.
//!
//! Generates DOT (Graphviz) format graphs showing grammatical relations
//! between morphological chunks, similar to the legacy Perl gra-cgi tool.
//!
//! # Layout
//!
//! - **LR (left-to-right)**: Words flow left to right, dependency arcs curve above/below
//! - **Invisible ordering edges**: Maintain word sequence without visual clutter
//! - **Colored dependency edges**: Labeled with relation type (SUBJ, OBJ, DET, etc.)
//! - **ROOT node**: Invisible node at position 0 for root relations
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::backend::{LspBackendError, ParseState, TierName};
use serde::{Deserialize, Serialize};
use talkbank_model::alignment::GraAlignment;
use talkbank_model::model::{GraTier, MorTier, Utterance};

mod builder;
mod edges;
mod error;
mod labels;

pub use error::GraphEdgeError;

#[cfg(test)]
mod tests;

/// Response shape for `talkbank/showDependencyGraph`.
///
/// Using a discriminated union keeps the TS extension from having to guess
/// whether a bare string is DOT syntax or a human-readable reason. The Graphviz
/// renderer choked on plain-text messages ("No %mor tier found") under the old
/// `Value::String`-based protocol; `kind` makes the two cases unambiguous at
/// the boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DependencyGraphResponse {
    /// The utterance has the data needed to render: `source` is Graphviz DOT.
    Dot {
        /// DOT source ready for a Graphviz renderer.
        source: String,
    },
    /// No graph can be rendered. `reason` is a user-facing explanation
    /// (e.g., "No %mor tier found", "No %gra alignment computed").
    Unavailable {
        /// Why the graph is not available for this utterance.
        reason: String,
    },
}

/// Produce a typed dependency-graph response for a single utterance.
///
/// Delegates to [`generate_dot_graph`] for the actual DOT construction and
/// lifts the typed `Result<String, LspBackendError>` into the discriminated
/// [`DependencyGraphResponse`]. Callers — the LSP command handler and unit
/// tests alike — see the same typed verdict; the `reason` field on the
/// `Unavailable` variant carries the error's `Display` output, so the
/// user-facing wire format is unchanged from the pre-typed version.
pub fn build_dependency_graph_response(
    utterance: &Utterance,
    parse_state: ParseState,
) -> DependencyGraphResponse {
    match generate_dot_graph(utterance, parse_state) {
        Ok(source) => DependencyGraphResponse::Dot { source },
        Err(err) => DependencyGraphResponse::Unavailable {
            reason: err.to_string(),
        },
    }
}

/// Generate a DOT format dependency graph for an utterance's %mor and %gra tiers.
///
/// # Returns
///
/// DOT format string suitable for rendering with Graphviz, or error message.
///
/// # Format
///
/// The generated graph:
/// - Shows all morphological chunks (words, clitics, terminators) as nodes
/// - Connects chunks with dependency edges from %gra tier
/// - Uses invisible edges to enforce left-to-right word order
/// - Layouts with `rankdir=LR` for left-to-right presentation
///
/// # Example Output
///
/// ```text
/// digraph utterance {
///   rankdir=LR;
///   node [shape=box, style=filled, fillcolor=white, fontname=Arial];
///   edge [fontname=Arial, fontsize=10];
///
///   0 [label="ROOT", shape=point, style=invis];
///   1 [label="the\n1"];
///   2 [label="cat\n2"];
///   3 [label="sat\n3"];
///
///   1 -> 2 [style=invis];
///   2 -> 3 [style=invis];
///
///   1 -> 2 [label="DET"];
///   2 -> 3 [label="SUBJ"];
///   3 -> 0 [label="ROOT"];
/// }
/// ```
pub fn generate_dot_graph(
    utterance: &Utterance,
    parse_state: ParseState,
) -> Result<String, LspBackendError> {
    let mor_tier = require_mor_tier(utterance)?;
    let gra_tier = require_gra_tier(utterance)?;
    let gra_alignment = require_gra_alignment(utterance)?;

    let node_labels = labels::collect_node_labels(mor_tier);
    builder::render_graph(&node_labels, gra_tier, gra_alignment, parse_state)
}

/// Require a `%mor` tier or return a typed [`LspBackendError::MissingTier`].
fn require_mor_tier(utterance: &Utterance) -> Result<&MorTier, LspBackendError> {
    utterance.mor_tier().ok_or(LspBackendError::MissingTier {
        tier: TierName::Mor,
    })
}

/// Require a `%gra` tier or return a typed [`LspBackendError::MissingTier`].
fn require_gra_tier(utterance: &Utterance) -> Result<&GraTier, LspBackendError> {
    utterance.gra_tier().ok_or(LspBackendError::MissingTier {
        tier: TierName::Gra,
    })
}

/// Require `%gra` alignment metadata before dependency graph rendering.
fn require_gra_alignment(utterance: &Utterance) -> Result<&GraAlignment, LspBackendError> {
    let alignment_metadata = utterance
        .alignments
        .as_ref()
        .ok_or(LspBackendError::AlignmentMetadataMissing)?;

    alignment_metadata
        .gra
        .as_ref()
        .ok_or(LspBackendError::TierAlignmentMissing {
            tier: TierName::Gra,
        })
}
