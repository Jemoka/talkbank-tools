//! Untranscribed-material markers (`xxx`, `yyy`, `www`) for word tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Untranscribed_Material>
//! - <https://talkbank.org/0info/manuals/CHAT.html#UntranscribedMaterial_Code>

use crate::model::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Classification for untranscribed-word marker tokens.
///
/// Each variant maps directly to one canonical CHAT marker (`xxx`, `yyy`, `www`).
///
/// # CHAT Format Examples
///
/// ```text
/// xxx               Unintelligible speech
/// yyy               Requires phonetic transcription
/// www               Deliberately untranscribed
/// ```
///
/// # Usage Context
///
/// These markers appear as the word text itself when speech cannot be
/// transcribed using standard orthography:
///
/// ```text
/// *CHI: I want xxx .           Child says something unintelligible
/// *MOT: did you say yyy ?      Requires phonetic analysis
/// *CHI: www and then we left   Deliberately not transcribed
/// ```
///
/// # References
///
/// - [Untranscribed Material](https://talkbank.org/0info/manuals/CHAT.html#Untranscribed_Material)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(rename_all = "lowercase")]
pub enum UntranscribedStatus {
    /// Unintelligible speech (`xxx`).
    Unintelligible,

    /// Requires phonetic transcription (`yyy`).
    Phonetic,

    /// Deliberately untranscribed (`www`).
    Untranscribed,
}

impl WriteChat for UntranscribedStatus {
    /// Writes canonical CHAT marker text (`xxx`, `yyy`, or `www`).
    ///
    /// Serialization is intentionally lossless and normalization-free because
    /// downstream tooling relies on the exact marker token.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            UntranscribedStatus::Unintelligible => w.write_str("xxx"),
            UntranscribedStatus::Phonetic => w.write_str("yyy"),
            UntranscribedStatus::Untranscribed => w.write_str("www"),
        }
    }
}
