//! Mapping from CST terminator node kinds to `model::Terminator`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#BreakForCoding>
//! - <https://talkbank.org/0info/manuals/CHAT.html#BrokenQuestion_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>

use crate::model::Terminator;
use crate::node_types::*;
use talkbank_model::Span;

/// Map terminator node kind to Terminator enum.
///
/// Returns None for unknown terminator kinds (e.g., ERROR nodes from malformed input).
pub(crate) fn terminator_from_node_kind(kind: &str, span: Span) -> Option<Terminator> {
    match kind {
        PERIOD => Some(Terminator::Period { span }),
        QUESTION => Some(Terminator::Question { span }),
        EXCLAMATION => Some(Terminator::Exclamation { span }),
        TRAILING_OFF => Some(Terminator::TrailingOff { span }),
        INTERRUPTION => Some(Terminator::Interruption { span }),
        SELF_INTERRUPTION => Some(Terminator::SelfInterruption { span }),
        INTERRUPTED_QUESTION => Some(Terminator::InterruptedQuestion { span }),
        BROKEN_QUESTION => Some(Terminator::BrokenQuestion { span }),
        QUOTED_NEW_LINE => Some(Terminator::QuotedNewLine { span }),
        QUOTED_PERIOD_SIMPLE => Some(Terminator::QuotedPeriodSimple { span }),
        SELF_INTERRUPTED_QUESTION => Some(Terminator::SelfInterruptedQuestion { span }),
        TRAILING_OFF_QUESTION => Some(Terminator::TrailingOffQuestion { span }),
        BREAK_FOR_CODING => Some(Terminator::BreakForCoding { span }),
        _ => None,
    }
}
