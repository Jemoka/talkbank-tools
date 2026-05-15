// Test code is exempt from this crate's `deny`-level panic lints —
// see `book/src/operations/panic-audit/talkbank-model.md`.
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
//! Core TalkBank CHAT model plus validation/alignment APIs.
//!
//! `model` contains the strongly-typed AST/data structures, while `validation` and `alignment`
//! provide semantic checks and cross-tier consistency logic used by higher-level tools.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Examples
//!
//! ```
//! use talkbank_model::ChatFile;
//!
//! let file = ChatFile::new(vec![]);
//! assert_eq!(file.utterances().count(), 0);
//! ```

#![warn(missing_docs)]

// Self-alias so that proc macros generating `talkbank_model::SpanShift` resolve
// within this crate itself.
extern crate self as talkbank_model;

pub mod alignment;
pub mod chars;
pub mod errors;
pub mod generated;
pub mod indices;
pub mod model;
pub mod parser_api;
pub mod pipeline;
pub mod text_types;
pub mod validation;

pub use alignment::*;
pub use errors::*;
pub use indices::{UtteranceIdx, WordIdx};
pub use model::*;
pub use parser_api::*;
pub use pipeline::*;
pub use text_types::{ChatCleanedText, ChatRawText};
pub use validation::{Validate, ValidationContext, resolve_word_language};
