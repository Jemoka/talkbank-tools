//! Item-level types for `%pho` and `%mod` dependent tiers.
//!
//! CHAT reference anchors:
//! - [Phonology tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
//! - [Model phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
//! - [IPA usage](https://talkbank.org/0info/manuals/CHAT.html#IPA)

use super::{PhoWord, WriteChat};
use crate::ErrorSink;
use crate::validation::{Validate, ValidationContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Phonological tier content item
///
/// Represents a single item in a %pho or %mod tier, which can be:
/// - A simple phonological word (e.g., `həˈloʊ`)
/// - A compound word (e.g., `foo+bar`)
/// - A phonological group containing multiple words (e.g., `‹a b c›`)
///
/// # Item Types
///
/// **Word**: Single phonological transcription aligning with one main tier word
/// ```text
/// *CHI: hello .
/// %pho: həˈloʊ .
/// ```
///
/// **Group**: Multiple phonological words grouped together using `‹...›` brackets.
/// Used when multiple phonological units correspond to a single orthographic word
/// or when capturing detailed prosodic boundaries.
/// ```text
/// *CHI: wanna .
/// %pho: ‹wɑ nə› .
/// ```
///
/// # Alignment
///
/// Each PhoItem aligns with one alignable unit in the main tier:
/// - Words (including annotated words) → PhoItem::Word or PhoItem::Group
/// - Terminators → PhoItem::Word (typically ".")
/// - Retraces, events, pauses → NOT aligned (no corresponding PhoItem)
///
/// # CHAT Format Examples
///
/// Simple word:
/// ```text
/// həˈloʊ
/// ```
///
/// Phonological group:
/// ```text
/// ‹wɑ nə›
/// ```
///
/// Multiple items in tier:
/// ```text
/// %pho: aɪ wɑnt ‹θri kʊ kiz› .
/// ```
///
/// # References
///
/// - [Phonology Tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
/// - [Model Phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
/// - [IPA Usage](https://talkbank.org/0info/manuals/CHAT.html#IPA)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(untagged)]
pub enum PhoItem {
    /// Phonological group `‹...›` containing multiple words
    /// Note: Must come FIRST in untagged enum to match arrays before strings
    Group(PhoGroupWords),

    /// Simple phonological word or token
    Word(PhoWord),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
/// A group of phonological words within a phonological item.
///
/// # Reference
///
/// - [Phonology tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
pub struct PhoGroupWords(pub Vec<PhoWord>);

impl PhoGroupWords {
    /// Creates a new group of phonological words.
    ///
    /// Group order is preserved exactly because serialization and alignment
    /// depend on token position.
    pub fn new(words: Vec<PhoWord>) -> Self {
        Self(words)
    }

    /// Returns `true` if this group has no words.
    ///
    /// Empty groups are allowed at construction time, with semantic checks
    /// deferred to higher-level tier validation.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for PhoGroupWords {
    type Target = Vec<PhoWord>;

    /// Borrows the underlying phonological-word vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PhoGroupWords {
    /// Mutably borrows the underlying phonological-word vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<PhoWord>> for PhoGroupWords {
    /// Wraps grouped words without copying.
    ///
    /// This keeps parser-to-model conversion cheap for large corpora.
    fn from(words: Vec<PhoWord>) -> Self {
        Self(words)
    }
}

impl Validate for PhoGroupWords {
    /// Group-level semantic checks are enforced during tier/alignment validation.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

impl WriteChat for PhoItem {
    /// Serializes one `%pho` item (plain word or `‹...›` grouped words).
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            PhoItem::Word(word) => word.write_chat(w),
            PhoItem::Group(words) => {
                w.write_char('\u{2039}')?; // ‹
                for (i, word) in words.iter().enumerate() {
                    if i > 0 {
                        w.write_char(' ')?;
                    }
                    word.write_chat(w)?;
                }
                w.write_char('\u{203A}') // ›
            }
        }
    }
}
