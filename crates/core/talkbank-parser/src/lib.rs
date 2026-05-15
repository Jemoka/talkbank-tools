#![warn(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints —
// see `book/src/operations/panic-audit/talkbank-parser.md`.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! Tree-sitter parser for TalkBank CHAT.
//!
//! Create a [`TreeSitterParser`] once, then reuse it for all parsing in that
//! scope. The parser handle owns an internal tree-sitter buffer that is reused
//! across calls — creating a new parser per call wastes that allocation.
//!
//! **Do not create a parser per file or per word.** Create one at the top of
//! your entry point (CLI main, server request handler, test function) and pass
//! `&TreeSitterParser` to everything that needs parsing.
//!
//! # Example
//!
//! ```rust
//! use talkbank_parser::TreeSitterParser;
//!
//! let parser = TreeSitterParser::new().expect("grammar loads");
//!
//! // Reuse the same parser for multiple files:
//! let file1 = parser.parse_chat_file("@UTF8\n@Begin\n*CHI:\thello .\n@End\n")
//!     .expect("valid CHAT");
//! let file2 = parser.parse_chat_file("@UTF8\n@Begin\n*MOT:\thi .\n@End\n")
//!     .expect("valid CHAT");
//! ```
//!
//! # Thread Safety
//!
//! `TreeSitterParser` uses `RefCell` internally and is `!Send + !Sync`.
//! For multi-threaded work, create one parser per thread.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod error {
    pub use talkbank_model::*;
}

pub(crate) mod model {
    pub use talkbank_model::model::*;
}

#[cfg(test)]
pub(crate) mod validation {
    pub use talkbank_model::validation::*;
}

/// Node type string constants from tree-sitter-talkbank grammar.
pub mod node_types;

/// Token parsing for coarsened grammar tokens (language codes, annotations, etc.).
pub mod tokens;

/// Public API modules (tier parsing).
pub mod api;
/// Internal parser implementation modules.
pub(crate) mod parser;

/// Main parser type and initialization error.
pub use parser::{ParserInitError, TreeSitterParser};
pub use talkbank_model::FragmentSemanticContext;

/// Convenience re-exports for dependent-tier parsing APIs.
pub use api::{dependent_tier::parse_dependent_tier, tiers};
