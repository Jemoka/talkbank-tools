//! Phonological transcription tiers (%pho, %mod) for CHAT transcripts.
//!
//! Phonological tiers provide detailed pronunciation information using phonetic
//! notation (typically IPA or UNIBET). They capture how words are actually
//! pronounced versus their standard forms.
//!
//! # Tier Types
//!
//! - **%pho**: Actual pronunciation - what the speaker said
//! - **%mod**: Model/target pronunciation - standard or intended form
//! - **%upho**: User-defined pronunciation system
//!
//! # Format
//!
//! Phonological tiers use whitespace-separated tokens that align 1-to-1 with
//! alignable content in the main tier (words, excluding retraces and events).
//!
//! ```text
//! *CHI: I want three cookies .
//! %pho: aɪ wɑnt fwi kʊkiz .
//! %mod: aɪ wɑnt θri kʊkiz .
//! ```
//!
//! In this example, the child says "fwi" for "three" (θri).
//!
//! # Common Use Cases
//!
//! - **Phonological development**: Track child's pronunciation errors and progress
//! - **Clinical assessment**: Document speech sound disorders
//! - **Cross-linguistic research**: Capture L2 learner pronunciations
//! - **Dialectal variation**: Record non-standard pronunciations
//!
//! # Parsing Strategy
//!
//! We deliberately parse **only** enough word/group-level structure in %pho/%mod
//! to enable alignment with the main tier. The full IPA phoneme content is stored
//! as opaque strings in [`PhoWord`], not decomposed into individual segments or
//! features. Phon handles the deep phonological analysis; we avoid duplicating
//! that work.
//!
//! The related Phon extension tiers (%modsyl, %phosyl, %phoaln) follow the same
//! strategy — see the [`super::phon`] module.
//!
//! # CHAT Manual Reference
//!
//! - [Phonology Tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
//! - [Model Tier](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
//!
//! # Examples
//!
//! ```text
//! *CHI: the dog is wed .
//! %pho: ðə dɔg ɪz wɛd .
//! %mod: ðə dɔg ɪz rɛd .
//! ```
//! Child substitutes /w/ for /r/ in "red".

use super::WriteChat;

mod item;
mod tier;
mod tier_type;
mod word;

#[cfg(test)]
mod tests;

pub use item::{PhoGroupWords, PhoItem};
pub use tier::PhoTier;
pub use tier_type::PhoTierType;
pub use word::PhoWord;
