//! Type-state markers for validation status
//!
//! These phantom types enable compile-time enforcement of validation requirements.
//! A `ChatFile<NotValidated>` cannot be serialized to JSON, while a `ChatFile<Validated>`
//! has been through validation and can be safely exported.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

/// Type-state marker trait for validation status
///
/// This trait is sealed and can only be implemented by `Validated` and `NotValidated`.
pub trait ValidationState: sealed::Sealed {}

/// Marker indicating a ChatFile has been validated
///
/// Only `ChatFile<Validated>` can be serialized to JSON or exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validated;

/// Marker indicating a ChatFile has NOT been validated
///
/// `ChatFile<NotValidated>` is the result of parsing. It must go through
/// validation before it can be serialized to JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotValidated;

impl ValidationState for Validated {}
impl ValidationState for NotValidated {}

mod sealed {
    /// Defines behavior for Sealed.
    pub trait Sealed {}
    impl Sealed for super::Validated {}
    impl Sealed for super::NotValidated {}
}
