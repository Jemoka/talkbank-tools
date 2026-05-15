//! `%mor` word representation (`POS|lemma[-feature]*`) used in the morphological tier.
//!
//! CHAT reference anchors:
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::super::WriteChat;
use super::super::analysis::{MorFeature, MorStem, PosCategory};

/// POS string for the `L2|xxx` placeholder. BA2-equivalent fallback
/// when secondary-language morphology is unavailable.
const L2_PLACEHOLDER_POS: &str = "L2";
/// Lemma string for the `L2|xxx` placeholder. Pairs with
/// [`L2_PLACEHOLDER_POS`].
const L2_PLACEHOLDER_LEMMA: &str = "xxx";

/// Single morphological word in UD format.
///
/// A `MorWord` represents the complete morphological analysis of a single word,
/// consisting of a required POS tag and lemma, and optional feature chains.
///
/// # Structure
///
/// The format is: `POS|lemma[-Feature]*`
/// - **POS**: UD-style part-of-speech tag (e.g., `noun`, `verb`, `pron`, `det`)
/// - **Lemma**: Word lemma/base form (required)
/// - **Features**: UD morphological feature values separated by `-` (e.g., `-Plur`, `-Fin-Ind-Pres-S3`)
///
/// # CHAT Format Examples
///
/// Simple noun:
/// ```text
/// noun|dog
/// ```
///
/// Plural noun with features:
/// ```text
/// noun|dog-Plur
/// ```
///
/// Verb with multiple features:
/// ```text
/// verb|make-Part-Pres-S
/// ```
///
/// Auxiliary with complex features:
/// ```text
/// aux|be-Fin-Ind-Pres-S3
/// ```
///
/// # References
///
/// - [CHAT Manual: Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MorWord {
    /// UD-style part-of-speech tag (e.g., `noun`, `verb`, `pron`, `det`)
    pub pos: PosCategory,

    /// Word lemma/base form (e.g., `dog`, `be`, `I`)
    pub lemma: MorStem,

    /// Morphological feature values (e.g., `Plur`, `Fin`, `Ind`, `Pres`, `S3`)
    ///
    /// Uses SmallVec with inline capacity of 4 - most words have 0-4 features
    #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
    #[schemars(with = "Vec<MorFeature>")]
    pub features: SmallVec<[MorFeature; 4]>,
}

impl MorWord {
    /// Create a new morphological word with the given POS and lemma.
    ///
    /// Features start empty and can be layered in with builder helpers.
    /// Validation of lexical quality runs later at `%mor` tier validation time.
    pub fn new(pos: impl Into<PosCategory>, lemma: impl Into<MorStem>) -> Self {
        Self {
            pos: pos.into(),
            lemma: lemma.into(),
            features: SmallVec::new(),
        }
    }

    /// Append a morphological feature (e.g., `Plur`, `Past`).
    ///
    /// Feature order is preserved because serialization emits exactly the stored
    /// sequence after `POS|lemma`.
    pub fn with_feature(mut self, feature: impl Into<MorFeature>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Replace all features.
    ///
    /// This is useful when callers already parsed a complete feature vector and
    /// want one assignment instead of repeated `with_feature` chaining.
    pub fn with_features(mut self, features: impl Into<SmallVec<[MorFeature; 4]>>) -> Self {
        self.features = features.into();
        self
    }

    /// Build the canonical `L2|xxx` placeholder used when secondary
    /// language morphology cannot be analyzed (or is rejected by the
    /// L2 splice's invariant guard) — BA2-equivalent fallback shape.
    pub fn l2_placeholder() -> Self {
        Self::new(
            PosCategory::new(L2_PLACEHOLDER_POS),
            MorStem::new(L2_PLACEHOLDER_LEMMA),
        )
    }

    /// Reset this word to the `L2|xxx` placeholder in place. Used by
    /// the synthesis path and the L2 splice rollback path to demote a
    /// word to BA2-equivalent fallback morphology without reallocating
    /// the surrounding structure.
    pub fn reset_to_l2_placeholder(&mut self) {
        self.pos = PosCategory::new(L2_PLACEHOLDER_POS);
        self.lemma = MorStem::new(L2_PLACEHOLDER_LEMMA);
        self.features.clear();
    }

    /// Serializes one `%mor` word as `POS|lemma[-Feature]*`.
    ///
    /// The method writes directly into the provided formatter so callers can
    /// stream full tiers without per-token allocations.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Write pos|lemma
        w.write_str(&self.pos)?;
        w.write_char('|')?;
        w.write_str(&self.lemma)?;
        // Write features
        for feature in &self.features {
            w.write_char('-')?;
            feature.write_chat(w)?;
        }
        Ok(())
    }
}

impl WriteChat for MorWord {
    /// Serializes this `%mor` token as `POS|lemma[-Feature]*`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        MorWord::write_chat(self, w)
    }
}
