//! Language-resolution metadata for utterances.
//!
//! This layer records resolved language provenance for alignable words and
//! preserves where each resolution came from (`@Languages`, `[- code]`, word
//! markers, or unresolved fallback). It is consumed by validation, analytics,
//! and serialization paths that need language context without re-running parsing.
//!
//! Resolution precedence follows CHAT semantics:
//! - file default from `@Languages`
//! - utterance-scoped overrides (`[- code]`)
//! - explicit word markers (`@s`, `@s:code`, `@s:eng+spa`, `@s:eng&spa`)
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker>

mod metadata;
mod source;
#[cfg(test)]
mod tests;
mod word_info;

pub use metadata::{LanguageMetadata, WordLanguageInfos};
pub use source::LanguageSource;
pub use word_info::{WordLanguageInfo, WordLanguages};
