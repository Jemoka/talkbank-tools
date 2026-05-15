//! Parser object construction and initialization errors.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use std::cell::RefCell;
use thiserror::Error;
use tracing::{debug, warn};
use tree_sitter::Parser;

/// Tree-sitter based CHAT parser.
///
/// Create one per entry point and reuse it for all parsing in that scope.
/// Pass `&TreeSitterParser` to functions that need parsing — do not create
/// a new parser per file or per word.
///
/// The parser is `!Send + !Sync` (uses `RefCell` internally). For
/// multi-threaded work, create one parser per thread.
pub struct TreeSitterParser {
    pub(crate) parser: RefCell<Parser>,
}

/// Errors that can occur when initializing the tree-sitter parser.
#[derive(Debug, Error)]
pub enum ParserInitError {
    /// Failed to load the tree-sitter-talkbank grammar.
    #[error("Failed to load tree-sitter-talkbank grammar: {0}")]
    SetLanguage(#[from] tree_sitter::LanguageError),
}

impl TreeSitterParser {
    /// Create a new `TreeSitterParser` by loading the tree-sitter-talkbank grammar.
    ///
    /// Initializes a `tree_sitter::Parser` and configures it with the
    /// `tree_sitter_talkbank` language definition. The resulting parser can then
    /// be used for all CHAT parsing operations (files, utterances, tiers, words).
    ///
    /// # Errors
    ///
    /// Returns [`ParserInitError::SetLanguage`] if the tree-sitter-talkbank grammar
    /// cannot be loaded (e.g., ABI version mismatch between the grammar and the
    /// tree-sitter runtime).
    /// **Prefer the free functions** ([`parse_chat_file`](crate::parse_chat_file),
    /// [`parse_chat_file_streaming`](crate::parse_chat_file_streaming),
    /// [`parse_word`](crate::parse_word)) which use a thread-local parser pool
    /// and avoid per-call allocation.
    ///
    /// Direct construction is appropriate when you need a long-lived parser
    /// instance (e.g., per-worker-thread in batch processing, LSP backend).
    /// It is NOT needed for one-off parsing — the free functions handle that.
    #[tracing::instrument(name = "TreeSitterParser::new")]
    pub fn new() -> Result<Self, ParserInitError> {
        debug!("Creating TreeSitterParser");
        let mut parser = Parser::new();

        // Load CHAT grammar
        let language = tree_sitter_talkbank::LANGUAGE.into();
        if let Err(err) = parser.set_language(&language) {
            warn!("Failed to load tree-sitter-talkbank grammar: {:?}", err);
            return Err(err.into());
        }

        debug!("TreeSitterParser created successfully");
        Ok(Self {
            parser: RefCell::new(parser),
        })
    }
}
