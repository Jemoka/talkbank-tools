//! Utterance-level validation checks shared across main/dependent tiers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//!
//! These checks are intentionally utterance-local. Cross-utterance sequencing
//! rules (for example quotation/linker continuity across turns) live in the
//! `validation::cross_utterance` module.

mod ca_delimiter;
mod comma;
mod overlap;
mod quotation;
mod tiers;
mod underline;

#[cfg(test)]
mod tests;

// Re-export check functions so callers can keep using `validation::utterance::*`.
#[allow(unused_imports)]
pub(crate) use ca_delimiter::CADelimiterRole;
#[allow(unused_imports)]
pub(crate) use ca_delimiter::analyze_ca_delimiter_roles;
pub(crate) use ca_delimiter::check_ca_delimiter_balance;
pub(crate) use comma::{check_comma_after_non_spoken, check_consecutive_commas};
#[cfg(test)]
pub(crate) use overlap::check_overlap_index_values;
pub(crate) use overlap::check_overlap_markers;
pub(crate) use quotation::check_quotation_balance;
pub(crate) use tiers::check_no_duplicate_dependent_tiers;
pub(crate) use underline::check_underline_balance;
