//! Explicit state wrapper for utterance-level language metadata.
//!
//! CHAT reference anchors:
//! - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
//! - [Language switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::ValidationTagged;

use crate::LanguageMetadata;

/// Explicit state for utterance-level `LanguageMetadata`.
///
/// This avoids ambiguous `Option<LanguageMetadata>` semantics in the data model:
/// - `Uncomputed`: language metadata pipeline has not been run
/// - `Computed`: language metadata exists and is available for consumers
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ValidationTagged, Default,
)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UtteranceLanguageMetadata {
    /// Language metadata has not been computed for this utterance yet.
    #[validation_tag(warning)]
    #[default]
    Uncomputed,

    /// Language metadata has been computed and is available.
    Computed {
        /// The computed language metadata.
        metadata: LanguageMetadata,
    },
}

impl UtteranceLanguageMetadata {
    /// Wrap computed metadata in the explicit `Computed` state.
    ///
    /// This helper avoids direct enum construction at call sites and keeps
    /// state transitions explicit in language-resolution pipelines.
    pub fn computed(metadata: LanguageMetadata) -> Self {
        Self::Computed { metadata }
    }

    /// Returns `true` when language metadata has not been computed.
    ///
    /// Use this to distinguish "pipeline not run" from "pipeline run with empty results".
    pub fn is_uncomputed(&self) -> bool {
        matches!(self, Self::Uncomputed)
    }

    /// Returns computed metadata by shared reference, if available.
    ///
    /// This is the standard read path for validation and reporting logic.
    pub fn as_computed(&self) -> Option<&LanguageMetadata> {
        match self {
            Self::Computed { metadata } => Some(metadata),
            Self::Uncomputed => None,
        }
    }

    /// Returns computed metadata by mutable reference, if available.
    ///
    /// Mutation is useful for late enrichment passes that append derived fields.
    pub fn as_computed_mut(&mut self) -> Option<&mut LanguageMetadata> {
        match self {
            Self::Computed { metadata } => Some(metadata),
            Self::Uncomputed => None,
        }
    }

    /// Consume this state and return computed metadata, if available.
    ///
    /// Ownership transfer is useful when moving metadata into downstream artifacts.
    pub fn into_computed(self) -> Option<LanguageMetadata> {
        match self {
            Self::Computed { metadata } => Some(metadata),
            Self::Uncomputed => None,
        }
    }
}
