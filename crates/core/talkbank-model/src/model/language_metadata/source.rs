//! Provenance labels for resolved word-language assignments.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SpanShift, ValidationTagged};

/// Source of language resolution for a word.
///
/// CHAT supports multiple ways to specify language for words:
/// - **Default**: From `@Languages` header (primary language)
/// - **TierScoped**: From utterance-level `[- code]` marker
/// - **WordExplicit**: From word-level `@s:code` marker
/// - **WordShortcut**: From word-level `@s` shortcut (toggles between primary/secondary)
/// - **Unresolved**: No language could be determined (validation error)
///
/// Keeping provenance separate from `WordLanguages` lets downstream tools
/// distinguish "same resolved code, different source semantics" cases, which
/// matters for corpus QA and language-switching diagnostics.
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift, ValidationTagged,
)]
#[serde(rename_all = "snake_case")]
pub enum LanguageSource {
    /// Resolved from `@Languages` primary language.
    ///
    /// This is the baseline path when no utterance- or word-level override applies.
    Default,

    /// Resolved from utterance-scoped marker (`[- code]`).
    ///
    /// Applies to unmarked words while the scoped tier-language override is active.
    TierScoped,

    /// Resolved from explicit word marker (`@s:code`, `@s:eng+spa`, etc.).
    ///
    /// Used when the transcription names the word language(s) directly.
    WordExplicit,

    /// Resolved from `@s` shortcut toggling rule.
    ///
    /// In dual-language contexts this flips between primary and secondary language.
    WordShortcut,

    /// No language could be resolved.
    ///
    /// Indicates missing/ambiguous context rather than an implicit default language.
    #[validation_tag(error)]
    Unresolved,
}
