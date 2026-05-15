//! Domain selector for alignment counting and matching helpers.
//!
//! Each dependent tier applies slightly different alignment eligibility rules
//! over the same main-tier content. This enum makes those policy branches
//! explicit so helper APIs can stay deterministic and auditable.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

/// Alignment domain determines which main-tier elements participate in unit checks.
///
/// The same utterance content can produce different alignment-unit counts for
/// different tiers. For example, `%pho` may count pauses that `%mor` skips.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TierDomain {
    /// Morphological analysis alignment (`%mor`).
    ///
    /// Uses morpheme-oriented counting rules and skips content that has no
    /// morphological interpretation (for example, retraced material).
    Mor,
    /// Phonological alignment (`%pho`).
    ///
    /// Uses pronunciation-oriented counting rules, which are intentionally more
    /// permissive for produced speech content.
    Pho,
    /// Sign/speech-act alignment (`%sin`).
    ///
    /// Follows `%sin` grouping semantics while still mapping back to the same
    /// main-tier index domain used by other aligners.
    Sin,
    /// Word-timing alignment (`%wor`).
    ///
    /// Primarily follows `%pho`-like word counting but keeps `%wor`-specific
    /// exclusions (such as timestamp-shaped tokens).
    Wor,
}
