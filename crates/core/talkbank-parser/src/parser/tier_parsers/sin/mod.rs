//! %sin tier parser (gesture/sign annotation)
//!
//! Parses gesture/sign tier content using tree-sitter CST navigation.
//!
//! # Format
//!
//! ```text
//! %sin:    g:toy:dpoint 0 〔g:book:hold g:book:point〕
//! ```
//!
//! Tokens are separated by whitespace and align 1-1 with main tier words.
//! Common token patterns:
//! - `0` for no gesture
//! - Gesture codes like `g:object:type`
//! - Groups using `〔...〕` brackets
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GesturePosition_Tier>

mod groups;
mod parse;

#[cfg(test)]
mod tests;

pub use parse::parse_sin_tier;
