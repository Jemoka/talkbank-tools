//! Shared test-fixture helpers for the LSP crate.
//!
//! Before this module existed, ~15 `#[cfg(test)] mod tests` blocks each
//! redefined their own `parse_chat` / `parse_tree` /
//! `parse_chat_with_alignments` helpers with identical bodies. That
//! pattern is banned going forward — add a helper here and call it from
//! the test module.
//!
//! Two distinct tree-sitter builders exist and both are still
//! needed: [`parse_tree`] spins up a fresh `tree_sitter::Parser`
//! directly (the common case, fine when the test does not care about
//! incremental edit semantics), and [`parse_tree_incremental`]
//! routes through [`TreeSitterParser`] the way the live backend does
//! (for tests that exercise incremental-parse behavior). They return
//! the same `Tree` shape but exercise different code paths.

use talkbank_model::model::{ChatFile, Line};
use talkbank_parser::TreeSitterParser;
use tree_sitter::Tree;

/// Parse a CHAT source string into a `ChatFile`, panicking on failure.
///
/// Callers get a sharp panic (`unwrap`) on the parser's `Result`
/// because a test that can't parse its own fixture has nothing useful
/// to say.
pub(crate) fn parse_chat(content: &str) -> ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    parser.parse_chat_file(content).unwrap()
}

/// [`parse_chat`] + compute per-utterance alignment metadata.
///
/// Default `parse_chat` leaves `utterance.alignments = None`. Tests
/// that need main↔mor / %mor↔%gra / main↔%pho / main↔%sin /
/// main↔`WorTimingSidecar` populated must call this variant so
/// `AlignmentSet` is built on each utterance.
pub(crate) fn parse_chat_with_alignments(content: &str) -> ChatFile {
    let mut chat_file = parse_chat(content);
    for line in &mut chat_file.lines {
        if let Line::Utterance(utterance) = line {
            utterance.compute_alignments_default();
        }
    }
    chat_file
}

/// Build a `tree_sitter::Tree` via a direct `tree_sitter::Parser`.
///
/// Use when the test only needs a parsed syntax tree (for handlers
/// that walk the CST) and does not care which path the backend
/// would have taken. For tests that specifically exercise
/// incremental-parse behavior, use [`parse_tree_incremental`].
pub(crate) fn parse_tree(input: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_talkbank::LANGUAGE;
    parser.set_language(&language.into()).unwrap();
    parser.parse(input, None).unwrap()
}

/// Build a `tree_sitter::Tree` through [`TreeSitterParser`] — the
/// same entry point the LSP backend uses.
///
/// Prefer this over [`parse_tree`] when the test exercises the
/// incremental-parse code path (e.g. `references`, `rename`).
pub(crate) fn parse_tree_incremental(input: &str) -> Tree {
    let parser = TreeSitterParser::new().unwrap();
    parser.parse_tree_incremental(input, None).unwrap()
}
