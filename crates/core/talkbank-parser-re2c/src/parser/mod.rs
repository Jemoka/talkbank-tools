//! CHAT parser — re2c lexer + chumsky parser combinators.
//!
//! Entry points in `entry_points.rs`. Chumsky parsers in `main_tier.rs`,
//! `dependent_tiers.rs`, `headers.rs`. File-level parser in `file.rs`.

pub mod classify;
pub mod dependent_tiers;
pub mod entry_points;
pub mod file;
pub mod headers;
pub mod main_tier;
pub mod word_body;

// Re-export all public entry points so existing code can use `parser::parse_*`.
pub use entry_points::*;

use crate::lexer::Lexer;
use crate::token::Token;

/// Lex input text into a leaked token slice for chumsky parsers.
///
/// NUL-pads and leaks the input string, then collects lexer output into
/// a leaked slice. Both are leaked so `&'a [Token<'a>]` has a stable
/// lifetime for chumsky's `Input` trait.
pub(crate) fn lex_to_tokens<'a>(input: &str, start_condition: usize) -> &'a [Token<'a>] {
    let (tokens, _) = lex_to_tokens_and_source(input, start_condition);
    tokens
}

/// Lex input and return both the token slice and the leaked source string.
///
/// The leaked source (minus trailing NUL) can be reused as `ChatFile.source`,
/// avoiding a second `Box::leak` in entry points.
pub(crate) fn lex_to_tokens_and_source<'a>(
    input: &str,
    start_condition: usize,
) -> (&'a [Token<'a>], &'a str) {
    let mut padded = input.to_string();
    padded.push('\0');
    let padded: &'a str = Box::leak(padded.into_boxed_str());
    let lexer = Lexer::new(padded, start_condition);
    let tokens: Vec<Token<'a>> = lexer.map(|(tok, _span)| tok).collect();
    let token_slice = Box::leak(tokens.into_boxed_slice());
    // Source is the padded string minus the NUL sentinel
    let source = &padded[..padded.len() - 1];
    (token_slice, source)
}
