//! Helper functions for cross-utterance validation
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::model::{Linker, Utterance};

/// Helper: Check if utterance has quoted linker (+")
pub(super) fn has_quoted_linker(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|l| matches!(l, Linker::QuotationFollows))
}

/// Helper: Check if utterance has self-completion linker (+,)
/// Note: Currently DISABLED (2025-12-24) - see cross_utterance/mod.rs for rationale
#[allow(dead_code)]
pub(super) fn has_self_completion_linker(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|l| matches!(l, Linker::SelfCompletion))
}

/// Helper: Check if utterance has other-completion linker (++)
///
/// Other-completion (++) means a different speaker is finishing or continuing
/// another speaker's incomplete thought.
pub(super) fn has_other_completion_linker(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|l| matches!(l, Linker::OtherCompletion))
}
