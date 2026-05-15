//! Core utterance data model (`main` tier + dependent tiers + runtime metadata).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::model::dependent_tier::DependentTier;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use talkbank_derive::{SemanticEq, SpanShift};

use crate::ParseError;
use crate::{
    AlignmentSet, Header, MainTier, ParseHealthState, UtteranceLanguage, UtteranceLanguageMetadata,
};

/// A complete utterance with one main tier and zero or more dependent tiers.
///
/// In CHAT transcripts, each utterance is a speaker turn anchored by a required
/// main line (`*SPEAKER:`). Dependent tiers attach analysis layers (`%mor`,
/// `%gra`, `%pho`, `%wor`, text commentary tiers, etc.) in source order.
///
/// # Structure
///
/// - **Main tier**: required, stores speaker code + utterance content.
/// - **Dependent tiers**: optional, preserves original per-utterance tier order.
/// - **Runtime metadata**: alignment/parse-health/language state derived during validation.
///
/// # Tier Types
///
/// - **%mor**: Morphological analysis (parts of speech, lemmas, affixes)
/// - **%gra**: Grammatical relations (dependency structure for %mor)
/// - **%pho**: Phonological transcription (actual pronunciation)
/// - **%mod**: target/model phonological form
/// - **%sin**: Gesture/sign annotations
/// - **%com**: Comments about the utterance
/// - **%exp**: Explanations or expansions
/// - **%act**: Actions performed during utterance
/// - **%cod**: Coding categories for analysis
///
/// # CHAT Manual Reference
///
/// - [Main Line](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
/// - [Morphology (%mor)](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
/// - [Grammar (%gra)](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
///
/// # Example
///
/// ```
/// use talkbank_model::model::{Utterance, MainTier, Terminator};
/// use talkbank_model::Span;
///
/// let utterance = Utterance::new(
///     MainTier::new(
///         "CHI",
///         vec![/* words */],
///         Terminator::Period { span: Span::DUMMY },
///     )
/// );
/// // Dependent tiers can be added via builder methods
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Utterance {
    /// Headers that precede this utterance.
    ///
    /// Some headers (`@Comment`, `@Bg`, `@Eg`, `@G`) can appear between turns.
    /// These are attached here to preserve exact source ordering during read/write.
    ///
    /// Uses `SmallVec<[Header; 2]>` because most utterances carry at most two
    /// interstitial headers.
    ///
    /// See: [Comment Header](https://talkbank.org/0info/manuals/CHAT.html#Comment_Header)
    #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
    #[schemars(with = "Vec<Header>")]
    pub preceding_headers: SmallVec<[Header; 2]>,

    /// Main tier containing the speaker's transcribed words.
    ///
    /// Format: `*SPEAKER: word1 word2 terminator`.
    /// This field is required for every utterance.
    ///
    /// See: [Main Line Structure](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
    pub main: MainTier,

    /// Dependent tiers providing linguistic annotations and auxiliary content.
    ///
    /// Most utterances have 0-3 dependent tiers (e.g., `%mor`, `%gra`, `%pho`).
    /// Keeping tiers in one ordered collection avoids hard-coding one slot per
    /// tier kind and preserves unusual-but-valid tier layouts.
    ///
    /// Uses `SmallVec<[DependentTier; 3]>` to avoid heap allocation for common
    /// short tier lists.
    ///
    /// Common tier types:
    /// - **%mor**: Morphological analysis
    /// - **%gra**: Grammatical relations
    /// - **%pho**: Phonological transcription
    /// - **%sin**: Gesture annotations
    /// - **%com**: Comments
    /// - plus additional standard and user-defined `%x*` tiers
    ///
    /// See: [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
    #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
    #[schemars(with = "Vec<DependentTier>")]
    pub dependent_tiers: SmallVec<[DependentTier; 3]>,

    /// Alignment metadata (computed during validation).
    ///
    /// Contains the legacy aggregate alignment view between main and dependent tiers.
    /// `None` if alignment has not been computed yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub alignments: Option<AlignmentSet>,

    /// Embedded-alignment diagnostics collected during alignment computation.
    ///
    /// This is the source of truth for alignment-related validation reporting
    /// in core parser/validation pipelines. It decouples validation from the
    /// legacy `AlignmentSet` shape retained for transitional consumers.
    #[serde(skip, default)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub alignment_diagnostics: Vec<ParseError>,

    /// Parse provenance for alignment gating (runtime metadata, not serialized).
    ///
    /// `Unknown` means this utterance did not come through a parser-backed path
    /// that established whether recovery was needed, so alignment code must
    /// not silently assume the content is parse-clean.
    #[serde(skip, default)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub parse_health: ParseHealthState,

    /// Explicit utterance-level language state.
    ///
    /// This avoids ambiguous `Option` semantics:
    /// - `Uncomputed`: metadata pipeline has not run yet
    /// - `Unresolved`: metadata computed but baseline language could not be resolved
    /// - resolved variants carry both language code and provenance
    ///
    /// Word-level language markers (`@s`, `@s:code`) remain in `language_metadata`.
    #[serde(default, skip_serializing_if = "UtteranceLanguage::is_uncomputed")]
    #[semantic_eq(skip)]
    pub utterance_language: UtteranceLanguage,

    /// Language metadata state (computed during validation).
    ///
    /// `Uncomputed` means the language metadata pipeline has not run.
    /// `Computed` carries the full per-word language metadata.
    #[serde(
        default,
        skip_serializing_if = "UtteranceLanguageMetadata::is_uncomputed"
    )]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub language_metadata: UtteranceLanguageMetadata,
}
