//! Supertype matcher for utterance terminator node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#BreakForCoding>

/// Check if a node kind is a `terminator` subtype
///
/// **Subtypes:** period, question, exclamation, interruption, etc.
pub fn is_terminator(kind: &str) -> bool {
    matches!(
        kind,
        "terminator" |  // Keep for backwards compatibility
        "break_for_coding" |
        "broken_question" |
        "exclamation" |
        "interrupted_question" |
        "interruption" |
        "period" |
        "question" |
        "quoted_new_line" |
        "quoted_period_simple" |
        "self_interrupted_question" |
        "self_interruption" |
        "trailing_off" |
        "trailing_off_question"
    )
}
