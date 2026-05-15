//! %pho and %mod tier parsers
//!
//! Parses phonological tier content into domain models using tree-sitter CST navigation.
//!
//! # Format
//!
//! Both tiers use the same simple format:
//! ```text
//! %pho:    wʌn tu θɹi
//! %mod:    wʌn tu θri
//! ```
//!
//! Tokens are separated by whitespace and align 1-1 with main tier words.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

mod cst;
mod groups;
mod unparsed;

#[cfg(test)]
mod tests;

pub use cst::{parse_mod_tier, parse_pho_tier};
pub use unparsed::parse_mod_tier_from_unparsed;
