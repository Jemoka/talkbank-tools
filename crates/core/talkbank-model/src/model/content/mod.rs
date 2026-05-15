//! Main-tier content model for CHAT speaker lines.
//!
//! This module defines every typed token that can appear on `*` tiers
//! (`*CHI:`, `*MOT:`, etc.), from lexical words to timing bullets and
//! discourse markers. These types form the canonical surface consumed by
//! validation, alignment, and serializer pipelines.
//!
//! ## Structural Types
//! - `MainTier` - The complete main tier
//! - `UtteranceContent` - Content between speaker and terminator
//!
//! ## Content Items
//! - `Pause` - Pauses and delays
//! - `Event` - Sound events &=
//! - `Action` - Actions &%
//! - `Group` - Grouped content <...>
//! - `Bracketed` - Bracketed content [...]
//!
//! ## Markers
//! - `Separator` - List separators (^)
//! - `Linker` - Inter-utterance linkers (+", etc.)
//! - `FreeCode` - Free codes on main tier
//! - `Overlap` - Overlap markers
//! - `Terminator` - Utterance terminators (.!? etc.)
//! - `Postcode` - Post-terminator codes
//! - `Bullet` - Time alignment bullets
//!
//! ## Word Types
//! Re-exported from `word/` submodule
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//! - <https://talkbank.org/0info/manuals/CHAT.html#LongEvent>
//! - <https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent>

pub mod word;

mod action;
mod bracketed;
mod bullet;
mod event;
mod freecode;
mod group;
mod linker;
mod long_feature;
mod main_tier;
mod nonvocal;
mod other_spoken;
mod overlap;
mod pause;
mod postcode;
mod retrace;
mod separator;
mod terminator;
mod tier_content;
mod utterance_content;

pub use action::*;
pub use bracketed::*;
pub use bullet::*;
pub use event::*;
pub use freecode::*;
pub use group::*;
pub use linker::*;
pub use long_feature::{LongFeatureBegin, LongFeatureEnd, LongFeatureLabel};
pub use main_tier::*;
pub use nonvocal::{NonvocalBegin, NonvocalEnd, NonvocalLabel, NonvocalSimple};
pub use other_spoken::*;
pub use overlap::*;
pub use pause::*;
pub use postcode::*;
pub use retrace::*;
pub use separator::*;
pub use terminator::*;
pub use tier_content::*;
pub use utterance_content::*;

// Re-export word types at content level
pub use word::{
    CADelimiter, CADelimiterType, CAElement, CAElementType, FormType, UnderlineMarker, Word,
    WordCategory, WordCliticBoundary, WordCompoundMarker, WordContent, WordContents,
    WordLanguageMarker, WordLengthening, WordShortening, WordStressMarker, WordStressMarkerType,
    WordSyllablePause, WordText, WordUnderlineBegin, WordUnderlineEnd,
};

// Re-export types needed by submodules via super::
pub(crate) use crate::model::WriteChat;
pub(crate) use crate::model::annotation::{Annotated, ReplacedWord};
pub(crate) use crate::model::header::{LanguageCode, SpeakerCode};
