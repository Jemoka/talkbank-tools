//! Word category prefixes (`0`, `&~`, `&-`, `&+`) for non-canonical lexical tokens.
//!
//! CHAT reference anchors:
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)

use crate::model::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Prefix category for non-canonical lexical tokens.
///
/// Categories encode omission/filler/nonword/fragment forms that change lexical
/// interpretation before morphological analysis.
///
/// # CHAT Format Examples
///
/// ```text
/// 0is               Omitted word (0)
/// 0det the          Omitted determiner
/// &~gaga            Nonword/babbling (&~)
/// &-uh              Filler (&-)
/// &-um              Filler
/// &+fr              Phonological fragment (&+)
/// &+w               Fragment starting with 'w'
/// ```
///
/// # Important Distinction
///
/// - `0` alone = Action (see [`crate::model::Action`])
/// - `0word` = Omitted word (this category)
///
/// # References
///
/// - [Omitted Words](https://talkbank.org/0info/manuals/CHAT.html#Omitted_Words)
/// - [Filler Code](https://talkbank.org/0info/manuals/CHAT.html#Filler_Code)
/// - [Fragments](https://talkbank.org/0info/manuals/CHAT.html#Fragments)
/// - [Nonwords](https://talkbank.org/0info/manuals/CHAT.html#Nonwords)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(rename_all = "lowercase")]
pub enum WordCategory {
    /// `0` - Omitted word (e.g., `0is`, `0det`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Omitted_Words>
    Omission,
    /// `(word)` - CA-style omission (standalone shortening)
    ///
    /// In CA mode (`@Options: CA`), `(word)` represents an omitted or uncertain word,
    /// semantically equivalent to `0word` in standard CHAT format.
    ///
    /// # Serialization
    ///
    /// Unlike `Omission` which serializes with a `0` prefix, `CAOmission` serializes
    /// as a parenthesized word `(word)` at the word level.
    ///
    /// # Validation
    ///
    /// In non-CA mode, `(word)` alone (without following text) is a validation error.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CA.html>
    #[serde(rename = "ca_omission")]
    CAOmission,
    /// `&~` - Nonword/babbling (e.g., `&~gaga`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Nonwords>
    Nonword,
    /// `&-` - Filler (e.g., `&-uh`, `&-um`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Filler_Code>
    Filler,
    /// `&+` - Phonological fragment (e.g., `&+fr`, `&+w`)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Fragments>
    PhonologicalFragment,
}

impl WordCategory {
    /// Returns canonical CHAT prefix text for this category.
    ///
    /// Note: `CAOmission` returns empty string because the parentheses are handled
    /// at the word level, not as a prefix. This method only covers prefix-bearing
    /// categories and is intentionally serialization-focused.
    pub fn to_chat_prefix(&self) -> &'static str {
        match self {
            WordCategory::Omission => "0",
            WordCategory::CAOmission => "", // No prefix - handled as shortening content
            WordCategory::Nonword => "&~",
            WordCategory::Filler => "&-",
            WordCategory::PhonologicalFragment => "&+",
        }
    }

    /// Return `true` for omission categories in either CHAT style.
    ///
    /// This unifies standard omission (`0word`) and CA-style parenthesized
    /// omission (`(word)`) for validators that care about omission semantics.
    pub fn is_omission(&self) -> bool {
        matches!(self, WordCategory::Omission | WordCategory::CAOmission)
    }
}

impl WriteChat for WordCategory {
    /// Writes the category prefix that precedes the lexical word body.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.to_chat_prefix())
    }
}
