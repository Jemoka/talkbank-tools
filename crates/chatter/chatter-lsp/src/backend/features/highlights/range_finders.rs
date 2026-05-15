//! CST traversal to locate tier-item LSP ranges for highlights.
//!
//! Each `find_*_range` function walks the tree-sitter CST to find the N-th child
//! of a specific node kind (e.g. the 3rd `MOR_CONTENT` inside `MOR_CONTENTS`)
//! and converts its byte span to an LSP `Range`. These are used by
//! [`super::tier_handlers`] to compute highlight regions.

use talkbank_parser::node_types::{GRA_RELATION, MOR_CONTENT, PHO_GROUP, SIN_GROUP};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use talkbank_model::Span;
use talkbank_model::model::UtteranceContent;

use crate::backend::utils;

/// Finds content range.
pub(super) fn find_content_range(content: &UtteranceContent, doc: &str) -> Option<Range> {
    let span = content_span(content)?;
    Some(range_from_span(doc, span))
}

/// Finds mor item range.
pub(super) fn find_mor_item_range(
    tree: &Tree,
    doc: &str,
    tier_span: Span,
    target_idx: usize,
) -> Option<Range> {
    find_nth_node_range(tree, doc, tier_span, MOR_CONTENT, target_idx)
}

/// Finds pho item range.
pub(super) fn find_pho_item_range(
    tree: &Tree,
    doc: &str,
    tier_span: Span,
    target_idx: usize,
) -> Option<Range> {
    find_nth_node_range(tree, doc, tier_span, PHO_GROUP, target_idx)
}

/// Finds sin item range.
pub(super) fn find_sin_item_range(
    tree: &Tree,
    doc: &str,
    tier_span: Span,
    target_idx: usize,
) -> Option<Range> {
    find_nth_node_range(tree, doc, tier_span, SIN_GROUP, target_idx)
}

/// Finds gra item range.
pub(super) fn find_gra_item_range(
    tree: &Tree,
    doc: &str,
    tier_span: Span,
    target_idx: usize,
) -> Option<Range> {
    find_nth_node_range(tree, doc, tier_span, GRA_RELATION, target_idx)
}

/// Finds mor item index at offset.
pub(super) fn find_mor_item_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    offset: u32,
) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, MOR_CONTENT, offset)
}

/// Finds pho item index at offset.
pub(super) fn find_pho_item_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    offset: u32,
) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, PHO_GROUP, offset)
}

/// Finds sin item index at offset.
pub(super) fn find_sin_item_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    offset: u32,
) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, SIN_GROUP, offset)
}

/// Finds gra item index at offset.
pub(super) fn find_gra_item_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    offset: u32,
) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, GRA_RELATION, offset)
}

/// Convert a model span into an LSP range.
fn range_from_span(doc: &str, span: Span) -> Range {
    Range {
        start: utils::offset_to_position(doc, span.start),
        end: utils::offset_to_position(doc, span.end),
    }
}

/// Convert a tree-sitter node into an LSP range.
fn range_from_node(doc: &str, node: Node) -> Range {
    Range {
        start: utils::offset_to_position(doc, node.start_byte() as u32),
        end: utils::offset_to_position(doc, node.end_byte() as u32),
    }
}

/// Return `true` when a node lies completely inside a span.
fn node_inside_span(node: Node, span: Span) -> bool {
    let start = node.start_byte() as u32;
    let end = node.end_byte() as u32;
    start >= span.start && end <= span.end
}

/// Return `true` when a node intersects a span.
fn node_overlaps_span(node: Node, span: Span) -> bool {
    let start = node.start_byte() as u32;
    let end = node.end_byte() as u32;
    end >= span.start && start <= span.end
}

/// Finds nth node range.
fn find_nth_node_range(
    tree: &Tree,
    doc: &str,
    tier_span: Span,
    kind: &str,
    target_idx: usize,
) -> Option<Range> {
    let node = find_nth_node_in_span(tree, tier_span, kind, target_idx)?;
    Some(range_from_node(doc, node))
}

/// Finds nth node in span.
fn find_nth_node_in_span<'a>(
    tree: &'a Tree,
    tier_span: Span,
    kind: &str,
    target_idx: usize,
) -> Option<Node<'a>> {
    let root = tree.root_node();
    let mut stack = vec![root];
    let mut current_idx = 0usize;

    while let Some(node) = stack.pop() {
        if !node_overlaps_span(node, tier_span) {
            continue;
        }

        if !node.is_missing() && node.kind() == kind && node_inside_span(node, tier_span) {
            if current_idx == target_idx {
                return Some(node);
            }
            current_idx += 1;
        }

        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }

    None
}

/// Finds node index at offset.
fn find_node_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    kind: &str,
    offset: u32,
) -> Option<usize> {
    let root = tree.root_node();
    let mut stack = vec![root];
    let mut current_idx = 0usize;

    while let Some(node) = stack.pop() {
        if !node_overlaps_span(node, tier_span) {
            continue;
        }

        if !node.is_missing() && node.kind() == kind && node_inside_span(node, tier_span) {
            let start = node.start_byte() as u32;
            let end = node.end_byte() as u32;
            if offset >= start && offset <= end {
                return Some(current_idx);
            }
            current_idx += 1;
        }

        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }

    None
}

/// Return the source span for a main-tier content item.
fn content_span(content: &UtteranceContent) -> Option<Span> {
    match content {
        UtteranceContent::Word(word) => Some(word.span),
        UtteranceContent::AnnotatedWord(annotated) => Some(annotated.span),
        UtteranceContent::ReplacedWord(replaced) => Some(replaced.span),
        UtteranceContent::Group(group) => Some(group.span),
        UtteranceContent::AnnotatedGroup(annotated) => Some(annotated.span),
        UtteranceContent::PhoGroup(_) => None,
        UtteranceContent::SinGroup(_) => None,
        UtteranceContent::Quotation(_) => None,
        _ => None,
    }
}
