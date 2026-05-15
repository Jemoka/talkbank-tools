//! Parameter structs and enums that replace long function parameter lists
//! and eliminate boolean blindness.
//!
//! These types group related parameters that are always passed together,
//! reducing function signatures from 10–15 parameters to 3–5.

use std::collections::BTreeSet;
use std::path::Path;

use crate::api::{DurationMs, LanguageCode3};
use crate::chat_ops::CacheTaskName;
use crate::chat_ops::fa::{AudioIdentity, FaEngineType, FaTimingMode};
use crate::chat_ops::morphosyntax_ops::{MultilingualPolicy, MwtDict, TokenizationMode};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Boolean blindness elimination
// ---------------------------------------------------------------------------

/// Cache lookup policy for audio-task NLP processing (FA, UTR ASR,
/// media analysis). Text-NLP tasks bypass the cache entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Use the cache normally (check for hits, store new results).
    UseCache,
    /// Skip cache lookups (always recompute; still stores results for future use).
    SkipCache,
}

impl CachePolicy {
    /// Returns `true` when cache lookups should be skipped.
    pub fn should_skip(&self) -> bool {
        matches!(self, Self::SkipCache)
    }
}

impl From<bool> for CachePolicy {
    /// Converts from the CLI `override_media_cache` flag — `true` means
    /// skip cache.
    fn from(override_media_cache: bool) -> Self {
        if override_media_cache {
            Self::SkipCache
        } else {
            Self::UseCache
        }
    }
}

/// Whether to generate the `%wor` (word-level timing) dependent tier.
///
/// Replaces the `write_wor: bool` parameter in FA processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "bool", from = "bool")]
#[derive(Default)]
pub enum WorTierPolicy {
    /// Generate `%wor` tier with word-level timing bullets.
    Include,
    /// Omit `%wor` tier from output.
    #[default]
    Omit,
}

impl WorTierPolicy {
    /// Returns `true` when the `%wor` tier should be generated.
    pub fn should_write(&self) -> bool {
        matches!(self, Self::Include)
    }
}

impl From<bool> for WorTierPolicy {
    /// Converts from legacy `write_wor: bool` — `true` means include.
    fn from(write_wor: bool) -> Self {
        if write_wor { Self::Include } else { Self::Omit }
    }
}

impl From<WorTierPolicy> for bool {
    fn from(policy: WorTierPolicy) -> Self {
        policy.should_write()
    }
}

/// Whether abbreviation merging should run before writing output.
///
/// Replaces the `merge_abbrev: bool` option-family flag with an explicit policy
/// that still serializes to the existing boolean wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "bool", from = "bool")]
#[derive(Default)]
pub enum MergeAbbrevPolicy {
    /// Merge abbreviated forms into their expanded spellings.
    Merge,
    /// Preserve abbreviated forms as written.
    #[default]
    Keep,
}

impl MergeAbbrevPolicy {
    /// Returns `true` when abbreviation merging should run.
    pub fn should_merge(&self) -> bool {
        matches!(self, Self::Merge)
    }
}

impl From<bool> for MergeAbbrevPolicy {
    /// Converts from legacy `merge_abbrev: bool` — `true` means merge.
    fn from(merge_abbrev: bool) -> Self {
        if merge_abbrev {
            Self::Merge
        } else {
            Self::Keep
        }
    }
}

impl From<MergeAbbrevPolicy> for bool {
    fn from(policy: MergeAbbrevPolicy) -> Self {
        policy.should_merge()
    }
}

/// Operator opt-in to the legacy Stanza constituency-parser fallback
/// for utterance segmentation when no language-specific TalkBank BERT
/// utseg model is configured for the requested language.
///
/// The default is [`Refuse`], which mirrors the
/// `WhisperHubModelNotFoundError` pattern: silent substitution of one
/// model for another is the foot-gun this enum exists to prevent.
/// Operators who want the previous behavior pass
/// `--utseg-fallback-stanza` on the CLI; this is surfaced on every
/// utseg-invoking subcommand (transcribe, transcribe-s, utseg).
///
/// Serialized as a plain JSON `bool` (`#[serde(into = "bool", from
/// = "bool")]`) so persisted job documents that predate this field
/// (`#[serde(default)]` on the surrounding option structs) load as
/// `Refuse` without a schema migration.
///
/// [`Refuse`]: UtsegFallbackPolicy::Refuse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "bool", from = "bool")]
#[derive(Default)]
pub enum UtsegFallbackPolicy {
    /// Refuse to segment when no language-specific BERT model is
    /// configured; raise a typed error pointing the user at
    /// `--utseg-fallback-stanza` and at the resolver-entry workflow.
    #[default]
    Refuse,
    /// Use Stanza constituency parsing as the segmenter for languages
    /// without a BERT model. Quality varies by Stanza language pack.
    AllowStanza,
}

impl UtsegFallbackPolicy {
    /// Returns `true` when the Stanza fallback should be permitted.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::AllowStanza)
    }
}

impl From<bool> for UtsegFallbackPolicy {
    /// Converts from the CLI `--utseg-fallback-stanza` flag — `true`
    /// means the operator has opted in to the Stanza substitution.
    fn from(allow: bool) -> Self {
        if allow {
            Self::AllowStanza
        } else {
            Self::Refuse
        }
    }
}

impl From<UtsegFallbackPolicy> for bool {
    fn from(policy: UtsegFallbackPolicy) -> Self {
        policy.is_allowed()
    }
}

// ---------------------------------------------------------------------------
// Fine-grained cache overrides
// ---------------------------------------------------------------------------

/// Typed cache override policy supporting per-task granularity.
///
/// Replaces the binary `override_media_cache: bool` with a richer model
/// that avoids boolean blindness and enables experiment-grade control.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CacheOverrides {
    /// Use cache normally for all tasks (default).
    #[default]
    None,
    /// Skip cache for all tasks (`--override-media-cache`).
    All,
    /// Skip cache only for listed tasks (`--override-media-cache-tasks`).
    Tasks(BTreeSet<CacheTaskName>),
}

impl CacheOverrides {
    /// Resolve the effective cache policy for a specific task.
    pub fn policy_for(&self, task: CacheTaskName) -> CachePolicy {
        match self {
            Self::None => CachePolicy::UseCache,
            Self::All => CachePolicy::SkipCache,
            Self::Tasks(set) => {
                if set.contains(&task) {
                    CachePolicy::SkipCache
                } else {
                    CachePolicy::UseCache
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter grouping structs
// ---------------------------------------------------------------------------

/// Morphosyntax-specific processing parameters.
///
/// Groups the 5 morphosyntax-specific parameters that are always passed
/// together through `process_morphosyntax`, `process_morphosyntax_batch`,
/// and `process_morphosyntax_incremental`.
pub struct MorphosyntaxParams<'a> {
    /// ISO 639-3 language code (e.g., `"eng"`, `"spa"`).
    pub lang: &'a LanguageCode3,
    /// How to handle Stanza retokenization during injection.
    pub tokenization_mode: TokenizationMode,
    /// How to handle non-primary languages in multilingual files.
    pub multilingual_policy: MultilingualPolicy,
    /// Multi-word token lexicon for retokenization overrides.
    pub mwt: &'a MwtDict,
    /// [Experimental] Route @s words to secondary language Stanza models
    /// instead of blanking to L2|xxx.
    pub l2_morphotag: bool,
    /// [Experimental] After morphotag, apply transcriber `$POS` hints:
    /// for each main-tier word carrying a `$POS` suffix, override the
    /// Stanza-assigned `%mor` POS with the CLAN→UD-mapped POS when
    /// they disagree. Lemma and features from Stanza are preserved.
    /// Default `true`; opt out via `--no-pos-hints`.
    pub respect_pos_hints: bool,
}

/// Audio file context for forced alignment and transcription.
///
/// Groups the audio-related parameters that are always passed together
/// to FA processing functions.
pub struct AudioContext<'a> {
    /// Path to the audio file on disk.
    pub audio_path: &'a Path,
    /// Content-based identity for cache keying (hash of audio content).
    pub audio_identity: &'a AudioIdentity,
    /// Total duration of the audio file in milliseconds, if known.
    pub total_audio_ms: Option<DurationMs>,
}

/// Forced alignment processing parameters.
///
/// Groups the 5 FA-specific parameters that are always passed together
/// through `process_fa` and `process_fa_incremental`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaParams {
    /// How to handle pause timing (`Continuous` vs `WithPauses`).
    pub timing_mode: FaTimingMode,
    /// Maximum FA group duration in milliseconds.
    pub max_group_ms: DurationMs,
    /// Which FA engine to use (`WhisperFa` or `Wave2Vec`).
    pub engine: FaEngineType,
    /// Cache lookup policy.
    pub cache_policy: CachePolicy,
    /// Whether to generate `%wor` tier.
    pub wor_tier: WorTierPolicy,
    /// Whether to apply post-FA bullet repair.
    pub bullet_repair: bool,
    /// Review tier verbosity.
    pub review_level: crate::chat_ops::fa::ReviewLevel,
}

#[cfg(test)]
mod tests {
    use super::{CacheOverrides, CachePolicy, MergeAbbrevPolicy, WorTierPolicy};

    #[test]
    fn wor_tier_policy_uses_bool_wire_format() {
        assert_eq!(
            serde_json::to_string(&WorTierPolicy::Include).unwrap(),
            "true"
        );
        assert_eq!(
            serde_json::to_string(&WorTierPolicy::Omit).unwrap(),
            "false"
        );
        assert_eq!(
            serde_json::from_str::<WorTierPolicy>("true").unwrap(),
            WorTierPolicy::Include
        );
        assert_eq!(
            serde_json::from_str::<WorTierPolicy>("false").unwrap(),
            WorTierPolicy::Omit
        );
    }

    #[test]
    fn cache_overrides_none_uses_cache_for_all() {
        let overrides = CacheOverrides::None;
        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::UtrAsr),
            CachePolicy::UseCache,
        );
        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::ForcedAlignment),
            CachePolicy::UseCache,
        );
    }

    #[test]
    fn cache_overrides_all_skips_cache_for_all() {
        let overrides = CacheOverrides::All;
        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::ForcedAlignment),
            CachePolicy::SkipCache,
        );
        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::UtrAsr),
            CachePolicy::SkipCache,
        );
    }

    #[test]
    fn cache_overrides_tasks_selective() {
        use std::collections::BTreeSet;
        let mut tasks = BTreeSet::new();
        tasks.insert(crate::chat_ops::CacheTaskName::UtrAsr);
        let overrides = CacheOverrides::Tasks(tasks);

        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::UtrAsr),
            CachePolicy::SkipCache,
        );
        assert_eq!(
            overrides.policy_for(crate::chat_ops::CacheTaskName::ForcedAlignment),
            CachePolicy::UseCache,
        );
    }

    #[test]
    fn cache_overrides_default_is_none() {
        assert_eq!(CacheOverrides::default(), CacheOverrides::None);
    }

    #[test]
    fn merge_abbrev_policy_uses_bool_wire_format() {
        assert_eq!(
            serde_json::to_string(&MergeAbbrevPolicy::Merge).unwrap(),
            "true"
        );
        assert_eq!(
            serde_json::to_string(&MergeAbbrevPolicy::Keep).unwrap(),
            "false"
        );
        assert_eq!(
            serde_json::from_str::<MergeAbbrevPolicy>("true").unwrap(),
            MergeAbbrevPolicy::Merge
        );
        assert_eq!(
            serde_json::from_str::<MergeAbbrevPolicy>("false").unwrap(),
            MergeAbbrevPolicy::Keep
        );
    }
}
