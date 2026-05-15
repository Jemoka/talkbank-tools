//! Item-level `%mor` morphology representation.
//!
//! A `%mor` item models one alignable main-tier slot plus any post-clitic
//! expansions that become additional `%gra` chunk positions.
//!
//! CHAT reference anchor:
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)

use super::super::WriteChat;
use super::analysis::PosCategory;
use super::word::MorWord;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use talkbank_derive::{SemanticEq, SpanShift};

/// Single morphological item (aligns with one main tier word).
///
/// A `Mor` item represents the morphological analysis for one word in the main tier.
/// Each item consists of a main MOR word and optional post-clitics from
/// multi-word token (MWT) expansions.
///
/// # Structure
///
/// The structure is: `main[~post-clitic]*`
/// - **Main word**: Required UD-style morphological analysis (e.g., `noun|dog-Plur`)
/// - **Post-clitics**: Marked with `~` (e.g., `~aux|be-Fin-Ind-Pres-S3`)
///
/// # CHAT Format Examples
///
/// Simple word:
/// ```text
/// *CHI: the dog runs .
/// %mor: det|the-Def-Art noun|dog verb|run-Fin-Ind-Pres-S3 .
/// ```
///
/// Word with post-clitic (contraction "it's" = "it is"):
/// ```text
/// *CHI: it's red .
/// %mor: pron|it~aux|be-Fin-Ind-Pres-S3 adj|red-S1 .
/// ```
///
/// # References
///
/// - [CHAT Manual: Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Mor {
    /// Main morphological word
    pub main: MorWord,

    /// Post-clitics (e.g., `~aux|be-Fin-Ind-Pres-S3`)
    ///
    /// Uses SmallVec with inline capacity of 2 - most words have 0-2 post-clitics
    #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
    #[schemars(with = "Vec<MorWord>")]
    pub post_clitics: SmallVec<[MorWord; 2]>,
}

impl Mor {
    /// Create a new morphological item with the given main word.
    ///
    /// Post-clitics start empty and can be appended later with
    /// [`Self::with_post_clitic`] or [`Self::with_post_clitics`].
    pub fn new(main: MorWord) -> Self {
        Self {
            main,
            post_clitics: SmallVec::new(),
        }
    }

    /// Append a post-clitic (marked with `~`).
    ///
    /// Order is significant because `%gra` chunk indexing follows serialized
    /// chunk order from left to right.
    pub fn with_post_clitic(mut self, clitic: MorWord) -> Self {
        self.post_clitics.push(clitic);
        self
    }

    /// Replace all post-clitics.
    ///
    /// This is useful when upstream logic already computed the full clitic list
    /// and wants one assignment instead of repeated pushes.
    pub fn with_post_clitics(mut self, clitics: impl Into<SmallVec<[MorWord; 2]>>) -> Self {
        self.post_clitics = clitics.into();
        self
    }

    /// Replace only the main word's POS category, preserving its lemma,
    /// morphological features, and all post-clitics. Used by consumers that
    /// need to override the POS decision (e.g., the `$POS`-hint post-pass)
    /// without touching the rest of the analysis.
    pub fn override_main_pos(&mut self, pos: impl Into<PosCategory>) {
        self.main.pos = pos.into();
    }

    /// Count total chunks contributed by this item for `%gra` alignment.
    ///
    /// Each main word counts as one chunk, and each post-clitic adds one more.
    /// This mirrors how `%gra` indices address clitic-expanded `%mor` content.
    pub fn count_chunks(&self) -> usize {
        1 + self.post_clitics.len()
    }

    /// Serializes one `%mor` item as `main[~post_clitic]*`.
    ///
    /// The method writes directly into the provided formatter so callers can
    /// stream large tiers without per-item allocations.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.main.write_chat(w)?;

        for clitic in &self.post_clitics {
            w.write_char('~')?;
            clitic.write_chat(w)?;
        }

        Ok(())
    }
}

impl WriteChat for Mor {
    /// Serializes the item as `main[~post_clitic]*`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        Mor::write_chat(self, w)
    }
}
