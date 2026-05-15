//! Main tier content types - the building blocks of utterances.
//!
//! This module defines all content types that can appear on the main tier between
//! the speaker code and terminator. Main tier content includes words, events, pauses,
//! groups, and various annotation markers.
//!
//! # Content Categories
//!
//! - **Words**: Basic lexical items, optionally with scoped annotations or replacements
//! - **Groups**: Bracketed content for retracing, phonology, sign language, quotations
//! - **Events**: Non-speech sounds and actions (e.g., `&=laughs`, `0`)
//! - **Pauses**: Timed or untimed pauses (e.g., `(.)`, `(2.5)`)
//! - **Annotations**: Freecodes, scoped symbols, error codes
//! - **Markers**: Overlap points, separators, long features, nonvocal scopes
//!
//! # CHAT Format Examples
//!
//! ```text
//! *CHI: I want cookie .                    Words
//! *CHI: I want [* m] cookie .              Word with error annotation
//! *CHI: I want [: need] that .             Word replacement
//! *CHI: <I want> [/] I need cookie .       Group with retracing
//! *CHI: the dog &=barks !                  Event
//! *CHI: um (.) yeah .                      Pause
//! *CHI: "hello there" !                    Quotation
//! *CHI: [^ transcriber note] .             Freecode
//! ```
//!
//! # References
//!
//! - [Main Tier](https://talkbank.org/0info/manuals/CHAT.html#Main_Tier)
//! - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
//! - [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
//! - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)

mod types;
mod write;

#[cfg(test)]
mod tests;

pub use types::UtteranceContent;
