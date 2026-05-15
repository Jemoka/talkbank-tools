//! Utterance-linker tokens (`+<`, `++`, `+^`, ...).
//!
//! Linkers are modeled separately from terminators because they describe how a
//! turn connects to surrounding turns, not how it ends prosodically.
//!
//! # CHAT Format References
//!
//! - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
//! - [Lazy Overlap Linker](https://talkbank.org/0info/manuals/CHAT.html#LazyOverlap_Linker)

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Cross-utterance linker token.
///
/// Linkers appear at the start of an utterance to indicate its relationship
/// to the previous utterance(s).
///
/// # CHAT Format Examples
///
/// ```text
/// *MOT: are you ready ?
/// *CHI: +< yes .          Lazy overlap (started before previous finished)
/// *MOT: what do you want ?
/// *CHI: ++ cookie !       Quick uptake (no gap)
/// *MOT: she said +".      Quotation follows
/// *CHI: I'm hungry +".
/// ```
///
/// # References
///
/// - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Linker {
    /// `+<` lazy-overlap-precedes linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#LazyOverlap_Linker>
    LazyOverlapPrecedes,
    /// `++` other-completion linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
    /// Note: Serialized as "quick_uptake" for backward compatibility with existing JSON.
    #[serde(rename = "quick_uptake")] // Keep old serialization name for backward compatibility
    OtherCompletion,
    /// `+^` quick-uptake-overlap linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuickUptake_Linker>
    QuickUptakeOverlap,
    /// `+"` quotation-follows linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
    QuotationFollows,
    /// `+,` self-completion linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
    SelfCompletion,
    /// `+≋` TCU continuation linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_Continuation_Linker>
    TcuContinuation,
    /// `+≈` no-break TCU continuation linker.
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_NoBreak_Linker>
    NoBreakTcuContinuation,
}

impl WriteChat for Linker {
    /// Serializes the canonical CHAT token for this linker.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            Linker::LazyOverlapPrecedes => w.write_str("+<"),
            Linker::OtherCompletion => w.write_str("++"),
            Linker::QuickUptakeOverlap => w.write_str("+^"),
            Linker::QuotationFollows => w.write_str("+\""),
            Linker::SelfCompletion => w.write_str("+,"),
            Linker::TcuContinuation => w.write_str("+\u{224B}"),
            Linker::NoBreakTcuContinuation => w.write_str("+\u{2248}"),
        }
    }
}

impl std::fmt::Display for Linker {
    /// Formats this linker using its CHAT token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips the lazy-overlap-precedes linker (`+<`).
    #[test]
    fn linker_lazy_overlap_precedes_roundtrip() {
        let linker = Linker::LazyOverlapPrecedes;
        let output = linker.to_string();
        assert_eq!(output, "+<", "Linker +< roundtrip failed");
    }

    /// Round-trips the other-completion linker (`++`).
    #[test]
    fn linker_other_completion_roundtrip() {
        let linker = Linker::OtherCompletion;
        let output = linker.to_string();
        assert_eq!(
            output, "++",
            "Linker ++ (other completion) roundtrip failed"
        );
    }

    /// Round-trips the quick-uptake-overlap linker (`+^`).
    #[test]
    fn linker_quick_uptake_overlap_roundtrip() {
        let linker = Linker::QuickUptakeOverlap;
        let output = linker.to_string();
        assert_eq!(output, "+^", "Linker +^ roundtrip failed");
    }

    /// Round-trips the self-completion linker (`+,`).
    #[test]
    fn linker_self_completion_roundtrip() {
        let linker = Linker::SelfCompletion;
        let output = linker.to_string();
        assert_eq!(output, "+,", "Linker +, roundtrip failed");
    }

    /// Round-trips the quotation-follows linker (`+\"`).
    #[test]
    fn linker_quotation_follows_roundtrip() {
        let linker = Linker::QuotationFollows;
        let output = linker.to_string();
        assert_eq!(output, "+\"", "Linker +\" roundtrip failed");
    }
}
