//! Load, parse, and pre-validate input for alignment visualization.
//!
//! Reads the file, parses via [`TreeSitterParser`], and runs
//! [`validate_with_alignment`](talkbank_model::ChatFile::validate_with_alignment)
//! to populate the `AlignmentSet` on each utterance. The resulting
//! [`AlignmentContext`] bundles the parsed `ChatFile`, source text, and any
//! validation errors so the renderer can display aligned tiers alongside
//! diagnostics.

use std::fs;
use std::path::PathBuf;

use talkbank_model::ChatFile;
use talkbank_model::{ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;

/// Parsed transcript, original text, and validation diagnostics for rendering.
pub(super) struct AlignmentContext {
    pub content: String,
    pub chat_file: ChatFile,
    pub validation_errors: Vec<ParseError>,
}

/// Read, parse, and validate a transcript before showing tier alignments.
///
/// This function mirrors the CLI’s validation path (including `%mor/%gra/%pho` alignment) so the rendered
/// output is grounded in the structured CHAT rules described in the manual. It writes the original text,
/// the parsed `ChatFile`, and any validation errors into an `AlignmentContext` so the caller can highlight
/// misalignments exactly where the main-tier content differs from the dependent tiers.
pub(super) fn load_alignment_context(input: &PathBuf) -> Result<AlignmentContext, String> {
    // Read file
    let content =
        fs::read_to_string(input).map_err(|e| format!("Error reading file {:?}: {}", input, e))?;

    // Parse file
    let parser = TreeSitterParser::new().map_err(|e| format!("Error creating parser: {}", e))?;

    let mut chat_file = parser
        .parse_chat_file(&content)
        .map_err(|e| format!("Error parsing file {:?}: {}", input, e))?;

    // Compute alignments for all utterances and report validation issues
    let errors = ErrorCollector::new();
    let filename = input.to_str();
    chat_file.validate_with_alignment(&errors, filename);
    let validation_errors = errors.into_vec();

    Ok(AlignmentContext {
        content,
        chat_file,
        validation_errors,
    })
}
