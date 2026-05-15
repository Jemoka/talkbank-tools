//! Strict (error-returning) chat-file parse entry points.
//!
//! This module provides parser methods that return `ParseResult` and therefore
//! fail the call when any error-level diagnostics are produced.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::parse_lines_with_old_tree;
use super::normalize::{headers_enable_ca_mode, normalize_ca_omissions};
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, ParseErrors, ParseResult,
    Severity, SourceLocation,
};
use crate::model::{ChatFile, Header, Line};
use crate::parser::TreeSitterParser;
use crate::parser::participants::build_participants_from_lines;
use talkbank_model::LineMap;
use talkbank_model::ParseOutcome;
use tracing::{debug, info, warn};
use tree_sitter::Tree;

/// Extract header values from parsed lines for post-parse feature checks.
fn collect_headers(lines: &[Line]) -> Vec<Header> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Header { header, .. } => Some(header.as_ref().clone()),
            _ => None,
        })
        .collect()
}

impl TreeSitterParser {
    /// Parse a CHAT file to a raw tree-sitter CST, optionally reusing a previous
    /// parse tree for incremental reparsing.
    ///
    /// Unlike the `parse_chat_file*` family, this method does **not** build a
    /// `ChatFile` model from the CST. It returns the bare `tree_sitter::Tree`,
    /// which callers (primarily the LSP) can use for lightweight structural queries
    /// or selective model rebuilds without paying the cost of full AST conversion.
    ///
    /// # Parameters
    ///
    /// - `input`: The full CHAT file source text.
    /// - `old_tree`: An optional previously-parsed tree. When provided, tree-sitter
    ///   reuses unchanged portions of the CST, making re-parsing after small edits
    ///   significantly faster.
    ///
    /// # Returns
    ///
    /// The `tree_sitter::Tree` representing the concrete syntax tree for the input.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` with a `ParseFailed` error code if tree-sitter's
    /// internal parser returns `None` (e.g., due to a timeout or cancellation).
    #[tracing::instrument(skip(self, input, old_tree), fields(input_size = input.len(), has_old_tree = old_tree.is_some()))]
    pub fn parse_tree_incremental(
        &self,
        input: &str,
        old_tree: Option<&Tree>,
    ) -> ParseResult<Tree> {
        debug!(
            "Parsing CHAT file to CST ({} bytes, old_tree: {})",
            input.len(),
            old_tree.is_some()
        );

        match self.parser.borrow_mut().parse(input, old_tree) {
            Some(tree) => Ok(tree),
            None => {
                let error = ParseError::new(
                    ErrorCode::ParseFailed,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), input),
                    "Tree-sitter parse failed for chat file",
                );
                Err(ParseErrors::from(vec![error]))
            }
        }
    }

    /// Parse a CHAT file in strict mode.
    ///
    /// Delegates to the streaming parse path, then upgrades any
    /// error-severity diagnostics into a returned `Err`.
    ///
    /// Callers that need best-effort recovery should use
    /// `parse_chat_file_streaming()` instead.
    #[tracing::instrument(skip(self, input), fields(input_size = input.len()))]
    pub fn parse_chat_file(&self, input: &str) -> ParseResult<ChatFile> {
        let errors = ErrorCollector::new();
        let outcome = self.parse_chat_file_fragment(input, 0, &errors);

        let error_vec = errors.into_vec();
        match outcome {
            ParseOutcome::Parsed(chat_file) => {
                let has_actual_errors = error_vec
                    .iter()
                    .any(|e| matches!(e.severity, Severity::Error));
                if has_actual_errors {
                    Err(ParseErrors::from(error_vec))
                } else {
                    Ok(chat_file)
                }
            }
            ParseOutcome::Rejected => {
                if error_vec.is_empty() {
                    Err(ParseErrors::from(vec![ParseError::new(
                        ErrorCode::ParseFailed,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), input),
                        "Parser returned no result and emitted no diagnostics",
                    )]))
                } else {
                    Err(ParseErrors::from(error_vec))
                }
            }
        }
    }

    /// Parse a CHAT file in strict mode with optional incremental reuse.
    ///
    /// When an old tree is provided, tree-sitter can reuse unchanged portions
    /// of the parse tree, making re-parsing after small edits significantly faster.
    ///
    /// Returns `(ParseResult<ChatFile>, Option<Tree>)` where the Tree can be
    /// cached and passed to future calls for incremental parsing.
    #[tracing::instrument(skip(self, input, old_tree), fields(input_size = input.len(), has_old_tree = old_tree.is_some()))]
    pub fn parse_chat_file_incremental(
        &self,
        input: &str,
        old_tree: Option<&Tree>,
    ) -> (ParseResult<ChatFile>, Option<Tree>) {
        debug!(
            "Parsing CHAT file incrementally ({} bytes, old_tree: {})",
            input.len(),
            old_tree.is_some()
        );

        let errors = ErrorCollector::new();
        let (mut lines, new_tree) = parse_lines_with_old_tree(self, input, old_tree, &errors);

        // Build participant map from headers
        let all_headers = collect_headers(&lines);

        let (participants, participant_errors) = build_participants_from_lines(&lines);
        for error in participant_errors {
            errors.report(error);
        }

        let ca_mode = headers_enable_ca_mode(&all_headers);
        if ca_mode {
            normalize_ca_omissions(&mut lines);
        }

        let error_vec = errors.into_vec();
        let has_actual_errors = error_vec
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));

        let result = if has_actual_errors {
            warn!("Parse failed with {} actual errors", error_vec.len());
            Err(ParseErrors::from(error_vec))
        } else {
            if !error_vec.is_empty() {
                warn!("Parse completed with {} warnings", error_vec.len());
            } else {
                info!(
                    "Parse completed successfully: {} lines, {} participants",
                    lines.len(),
                    participants.len()
                );
            }
            Ok(ChatFile::with_line_map(
                lines,
                participants,
                LineMap::new(input),
            ))
        };

        (result, new_tree)
    }

    /// Parse with incremental tree reuse while streaming diagnostics.
    ///
    /// Like `parse_chat_file_streaming` but accepts an `old_tree` for tree-sitter
    /// incremental reuse. Always returns a `(ChatFile, Option<Tree>)` — errors are
    /// streamed to the sink rather than causing an `Err` return.
    ///
    /// This is the preferred method for LSP full-fallback parsing: it produces a
    /// ChatFile even when errors exist, enabling features (hover, completion) and
    /// preserving a baseline for incremental diffing on the next keystroke.
    #[tracing::instrument(skip(self, input, old_tree, errors), fields(input_size = input.len(), has_old_tree = old_tree.is_some()))]
    pub fn parse_chat_file_streaming_incremental(
        &self,
        input: &str,
        old_tree: Option<&Tree>,
        errors: &impl ErrorSink,
    ) -> (ChatFile, Option<Tree>) {
        debug!(
            "Parsing CHAT file streaming-incremental ({} bytes, old_tree: {})",
            input.len(),
            old_tree.is_some()
        );

        let (mut lines, new_tree) = parse_lines_with_old_tree(self, input, old_tree, errors);

        let all_headers = collect_headers(&lines);

        let (participants, participant_errors) = build_participants_from_lines(&lines);
        for err in participant_errors {
            errors.report(err);
        }

        let ca_mode = headers_enable_ca_mode(&all_headers);
        if ca_mode {
            normalize_ca_omissions(&mut lines);
        }

        info!(
            "Streaming-incremental parse completed: {} lines, {} participants",
            lines.len(),
            participants.len()
        );

        (
            ChatFile::with_line_map(lines, participants, LineMap::new(input)),
            new_tree,
        )
    }
}
