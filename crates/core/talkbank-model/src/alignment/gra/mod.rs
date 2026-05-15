//! `%mor`-to-`%gra` alignment orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//!
//! This module aligns `%mor` chunk indices against `%gra` relation indices and
//! emits placeholder rows when counts diverge, allowing downstream diagnostics
//! to report precise mismatch positions.

mod align;
#[cfg(test)]
mod tests;
mod types;

pub use align::align_mor_to_gra;
pub use types::{GraAlignment, GraAlignmentPair};
