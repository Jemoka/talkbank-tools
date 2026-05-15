//! Tier discriminator for `%pho` vs `%mod`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use talkbank_derive::{SemanticEq, SpanShift};

/// Type of phonological tier.
///
/// Different tier types serve different purposes while using the same
/// phonetic transcription format.
///
/// # Tier Types
///
/// - **%pho**: Actual pronunciation - what was actually said
/// - **%mod**: Model/target pronunciation - what should have been said
///
/// # Use Cases
///
/// Use **%pho** alone when documenting pronunciation is sufficient:
/// ```text
/// *CHI: hello there .
/// %pho: həˈloʊ ðɛɹ .
/// ```
///
/// Use **%pho + %mod** together when documenting speech errors or developmental patterns:
/// ```text
/// *CHI: I want three cookies .
/// %pho: aɪ wɑnt fwi kʊkiz .
/// %mod: aɪ wɑnt θri kʊkiz .
/// ```
///
/// # CHAT Format Examples
///
/// Actual pronunciation only:
/// ```text
/// %pho: həˈloʊ wɜrld .
/// ```
///
/// With model/target pronunciation:
/// ```text
/// %pho: fwi kʊkiz .
/// %mod: θri kʊkiz .
/// ```
///
/// # References
///
/// - [Phonology Tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
/// - [Model Phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
/// - [IPA Usage](https://talkbank.org/0info/manuals/CHAT.html#IPA)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum PhoTierType {
    /// Actual pronunciation tier (%pho).
    ///
    /// Records what the speaker actually said, using phonetic notation
    /// (typically IPA or UNIBET).
    ///
    /// See: [Phonology](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
    Pho,

    /// Target/model pronunciation tier (%mod).
    ///
    /// Records the standard or intended pronunciation when the speaker's
    /// actual pronunciation (%pho) differs from the target form.
    ///
    /// See: [Model Phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
    Mod,
}

impl WriteChat for PhoTierType {
    /// Write tier type prefix to CHAT format
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            PhoTierType::Pho => w.write_str("%pho"),
            PhoTierType::Mod => w.write_str("%mod"),
        }
    }
}
