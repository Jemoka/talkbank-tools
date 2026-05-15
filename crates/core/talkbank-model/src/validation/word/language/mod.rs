//! Word-language resolution helpers used by word validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker>
//!
//! This module is split so callers can reuse a single entrypoint
//! (`resolve_word_language`) while keeping digit-policy and marker-parsing logic
//! isolated in focused submodules.

mod digits;
mod helpers;
mod resolve;

#[cfg(test)]
mod tests;

pub(crate) use digits::check_word_digits_multi;
pub use resolve::{LanguageResolution, LanguageResolutionOutcome, resolve_word_language};
