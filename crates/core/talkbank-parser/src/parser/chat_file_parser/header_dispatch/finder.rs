//! Locate the Nth header node in a parsed document tree.
//!
//! Used by single-line header APIs that parse inside a synthetic wrapper.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Begin_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#End_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#UTF8_Header>

use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::node_types::*;
use crate::parser::tree_parsing::parser_helpers::{is_header, is_pre_begin_header};
use tree_sitter::Node;

/// Find header node in tree-sitter CST by index.
///
/// Walks direct children of the document root and counts header nodes.
/// Conditions are mutually exclusive to prevent double-counting:
/// 1. Pre-begin header wrappers/subtypes (continue after handling)
/// 2. LINE nodes (search inside for headers)
/// 3. HEADER wrapper nodes (unwrap to inner child)
/// 4. Direct concrete header nodes (utf8_header, begin_header, end_header, etc.)
pub(super) fn find_header_node_in_tree(root: Node, index: usize) -> ParseResult<Node> {
    let mut found_count = 0;

    for i in 0..root.child_count() {
        if let Some(child) = root.child(i as u32) {
            let kind = child.kind();

            // 1. Pre-begin header wrappers/subtypes
            if kind == PRE_BEGIN_HEADER || is_pre_begin_header(kind) {
                if is_pre_begin_header(kind) && kind != PRE_BEGIN_HEADER {
                    // Concrete pre_begin_header subtype: count directly
                    if found_count == index {
                        return Ok(child);
                    }
                    found_count += 1;
                } else {
                    // PRE_BEGIN_HEADER wrapper: look inside for header children
                    for j in 0..child.child_count() {
                        if let Some(grandchild) = child.child(j as u32)
                            && is_header(grandchild.kind())
                        {
                            if found_count == index {
                                return Ok(grandchild);
                            }
                            found_count += 1;
                        }
                    }
                }
                continue;
            }

            // 2. LINE nodes: search inside for headers
            if kind == LINE {
                for j in 0..child.child_count() {
                    if let Some(grandchild) = child.child(j as u32) {
                        let gc_kind = grandchild.kind();
                        if gc_kind == HEADER {
                            // HEADER wrapper inside LINE: unwrap
                            if let Some(inner) = grandchild.child(0u32) {
                                if found_count == index {
                                    return Ok(inner);
                                }
                                found_count += 1;
                            }
                        } else if is_header(gc_kind) {
                            // Concrete header inside LINE
                            if found_count == index {
                                return Ok(grandchild);
                            }
                            found_count += 1;
                        }
                    }
                }
                continue;
            }

            // 3. HEADER wrapper node: unwrap to inner child
            if kind == HEADER {
                if let Some(inner) = child.child(0u32) {
                    if found_count == index {
                        return Ok(inner);
                    }
                    found_count += 1;
                }
                continue;
            }

            // 4. Direct concrete header node (utf8_header, begin_header,
            //    end_header, or any other is_header type that appears as a
            //    direct child of the document root)
            if is_header(kind) {
                if found_count == index {
                    return Ok(child);
                }
                found_count += 1;
            }
        }
    }

    let mut errors = ParseErrors::new();
    errors.push(
        ParseError::new(
            ErrorCode::TierValidationError,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new("", 0..0, ""),
            format!(
                "Tier validation error: header at index {} not found in CST (only {} headers present)",
                index, found_count
            ),
        )
        .with_suggestion("Check that all header lines are well-formed and appear before utterances"),
    );
    Err(errors)
}
