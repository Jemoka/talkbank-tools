//! Explicit utterance-level baseline language resolution state.
//!
//! CHAT reference anchors:
//! - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
//! - [Language switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

use crate::{LanguageCode, LanguageSource};

/// Utterance baseline language state used by downstream language metadata logic.
///
/// This avoids ambiguous `Option` semantics by separating:
/// - metadata not computed yet
/// - metadata computed but unresolved
/// - metadata computed and resolved from a known source
///
/// Word-level markers (`@s`, `@s:...`) are represented separately in
/// `LanguageMetadata` and can override this baseline per token.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
    ValidationTagged,
    Default,
)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UtteranceLanguage {
    /// Language metadata has not been computed for this utterance yet.
    #[validation_tag(warning)]
    #[default]
    Uncomputed,

    /// Metadata was computed, but no utterance baseline language could be resolved.
    #[validation_tag(error)]
    Unresolved,

    /// Baseline language resolved from the file default (`@Languages` primary).
    ResolvedDefault {
        /// The resolved language code.
        code: LanguageCode,
    },

    /// Baseline language resolved from utterance tier scope (`[- code]`).
    ResolvedTierScoped {
        /// The resolved language code.
        code: LanguageCode,
    },
}

impl UtteranceLanguage {
    /// Returns `true` when language baseline resolution has not run yet.
    ///
    /// This is distinct from `Unresolved`, which means resolution ran but found
    /// no usable baseline language.
    pub fn is_uncomputed(&self) -> bool {
        matches!(self, Self::Uncomputed)
    }

    /// Returns the resolved baseline language code, if any.
    ///
    /// Consumers that need provenance should pair this with [`Self::source`].
    pub fn code(&self) -> Option<&LanguageCode> {
        match self {
            Self::ResolvedDefault { code } | Self::ResolvedTierScoped { code } => Some(code),
            Self::Uncomputed | Self::Unresolved => None,
        }
    }

    /// Returns provenance for the resolved baseline language.
    ///
    /// Uncomputed and unresolved states both map to `LanguageSource::Unresolved`
    /// so downstream logic can treat them conservatively.
    pub fn source(&self) -> LanguageSource {
        match self {
            Self::ResolvedDefault { .. } => LanguageSource::Default,
            Self::ResolvedTierScoped { .. } => LanguageSource::TierScoped,
            Self::Uncomputed | Self::Unresolved => LanguageSource::Unresolved,
        }
    }
}
