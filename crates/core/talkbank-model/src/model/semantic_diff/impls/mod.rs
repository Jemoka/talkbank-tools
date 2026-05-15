//! SemanticDiff trait implementations for standard types.
//!
//! These impls support structural equality checks for parsed CHAT models.
//! Container impls manage path-aware traversal and shape differences, while
//! scalar impls define leaf-level value comparison policy (including metadata
//! exclusions such as source spans).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod container;
mod scalar;
