//! Scoped annotation types for CHAT transcripts.
//!
//! Scoped annotations provide additional information about words, phrases, or utterances.
//! They are enclosed in square brackets `[marker text]` and appear within or after
//! the content they annotate.
//!
//! This module re-exports both data types and formatter logic so callers can
//! treat scoped annotations as first-class typed nodes.
//!
//! # Common Annotations
//!
//! - **Errors**: `[*]` or `[* code]` - Mark speech errors or corrections needed
//! - **Explanations**: `[= text]` - Clarify unclear or ambiguous utterances
//! - **Retracing**: `[/]`, `[//]`, `[///]` - Mark repeated or self-corrected words
//! - **Overlaps**: `[<]`, `[>]` - Mark simultaneous speech by multiple speakers
//!
//! # CHAT Manual Reference
//!
//! - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
//! - [Explanation Scope](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope)
//! - [Retracing and Repetition](https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition)
//! - [Overlap Precedes Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
//! - [Overlap Follows Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
//!
//! # Examples
//!
//! ```text
//! *CHI: I want [/] I want cookie .     # Partial retracing
//! *CHI: he goed [* error] there .      # Error marking
//! *CHI: xxx [= probably said ball] .   # Explanation
//! *CHI: look [<] there .                # Overlap begin
//! *MOT: what [>] ?                     # Overlap end
//! ```

mod types;
mod write;

pub use types::{
    ContentAnnotation, OverlapMarkerIndex, ScopedAlternative, ScopedError, ScopedExplanation,
    ScopedOverlapBegin, ScopedOverlapEnd, ScopedParalinguistic, ScopedPercentComment,
    ScopedUnknown,
};
