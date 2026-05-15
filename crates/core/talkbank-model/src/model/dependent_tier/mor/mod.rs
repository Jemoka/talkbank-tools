//! Morphological analysis tier (`%mor`) representation.
//!
//! This module groups `%mor` atom types, word/item structures, and tier-level
//! containers used by parsing, alignment, and serialization paths.
//! It is the canonical source of `%mor` domain types reused across parser API,
//! model validation, and `%gra` alignment code.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
/// Part-of-speech categories, stems, and features.
pub mod analysis;
/// Chunk sequence primitive (main / post-clitic / terminator) used for `%gra` alignment.
pub mod chunk;
/// Morphological item (`Mor`).
pub mod item;
/// Morphological tier (`MorTier`) container and validation.
pub mod tier;
/// Word-level morphological representations.
pub mod word;

#[cfg(test)]
mod tests;

pub use analysis::{MorFeature, MorStem, PosCategory, clan_to_ud_upos};
pub use chunk::{MorChunk, MorChunkKind};
pub use item::Mor;
pub use tier::{MorTier, MorTierType};
pub use word::MorWord;
