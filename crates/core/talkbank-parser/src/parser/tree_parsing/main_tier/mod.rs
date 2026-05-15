//! Main tier parsing modules
//!
//! This module contains all the logic for parsing CHAT main tiers including:
//! - Overall main tier structure (speaker, content, terminator)
//! - Utterance content (words, events, actions, pauses, groups)
//! - Annotations (replacements, scoped annotations)
//! - Word-level parsing (categories, form types, content)
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

// Submodules
pub mod annotations;
pub mod content;
pub mod structure;
pub mod word;

// Re-export public API
