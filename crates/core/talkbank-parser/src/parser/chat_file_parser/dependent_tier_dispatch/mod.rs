//! Dependent tier dispatch
//!
//! Routes dependent tier nodes to the appropriate tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>

mod helpers;
mod parse;
mod parsed;
mod raw;
mod unparsed;
mod user_defined;

pub(crate) use parse::parse_and_attach_dependent_tier;
