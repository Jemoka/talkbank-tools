//! %gra tier parser
//!
//! Parses grammatical relation tiers (%gra) into the domain model using tree-sitter CST navigation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

mod relation;
mod tier;

#[cfg(test)]
mod tests;

pub use tier::parse_gra_tier;
