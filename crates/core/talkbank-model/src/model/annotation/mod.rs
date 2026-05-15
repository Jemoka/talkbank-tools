//! Annotation types (square bracket constructs)
//!
//! - `ContentAnnotation` - Annotations with scope markers `[*]`, `[!]`, etc.
//! - `Replacement` - Replacement text `[: word]`
//! - `ReplacedWord` - Word with its replacement
//! - `Annotated` - Base annotated item
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Repetition_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>

mod annotated;
mod replacement;
mod scoped;

pub use annotated::*;
pub use replacement::*;
pub use scoped::*;

// Re-export for submodules via super::
pub(crate) use crate::model::WriteChat;
pub(crate) use crate::model::content::Word;
