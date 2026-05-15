//! Derived metadata pipelines attached to `Utterance`.
//!
//! This module hosts computations that augment parsed utterances with runtime
//! metadata used by validation, diagnostics, and downstream tooling.
//! Input is the parsed utterance (`main` + dependent tiers); output is stored
//! on runtime metadata fields in [`crate::model::Utterance`].
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>

mod alignment;
mod language;
#[cfg(test)]
mod tests;
