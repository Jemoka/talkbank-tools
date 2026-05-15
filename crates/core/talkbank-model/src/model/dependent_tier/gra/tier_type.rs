//! Tier discriminator for `%gra`.
//!
//! The explicit tier-kind enum keeps formatting and generic tier dispatch
//! uniform even when a family currently exposes a single concrete tier tag.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use super::super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Type of grammatical relations tier.
///
/// This enum currently has one variant because CHAT defines `%gra` as a single
/// standardized dependency tier. Keeping it explicit still helps generic code
/// share tier-prefix logic with other dependent-tier families.
///
/// # Alignment
///
/// The %gra tier aligns with morphological **chunks** in the %mor tier
/// (including clitics).
///
/// # Universal Dependencies Format
///
/// ```text
/// word_index|head_index|relation_type
/// ```
///
/// # CHAT Format Example
///
/// ```text
/// *CHI: I eat cookies .
/// %mor: pro:sub|I v|eat n|cookie-PL .
/// %gra: 1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
/// ```
///
/// # References
///
/// - [Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
/// - [Universal Dependencies](https://universaldependencies.org/)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum GraTierType {
    /// Standard grammatical relations tier (%gra).
    ///
    /// Provides dependency syntax aligned to `%mor` chunks.
    ///
    /// See: [Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
    Gra,
}

impl WriteChat for GraTierType {
    /// Writes the `%gra` tier prefix used in CHAT serialization.
    ///
    /// Keeping this formatter on the enum keeps prefix logic co-located with
    /// tier-kind definitions.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            GraTierType::Gra => w.write_str("%gra"),
        }
    }
}
