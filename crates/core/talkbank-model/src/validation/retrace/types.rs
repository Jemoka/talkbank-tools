//! Data structures for retrace validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::Span;

/// Classification of leaf content in utterance structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafKind {
    /// Real utterance content (word, event, pause, etc.)
    RealContent,
    /// Non-real content (annotations, freecodes, etc.)
    NonRealContent,
    /// Terminator (final punctuation)
    Terminator,
}

/// Record of a retrace marker found during collection.
#[derive(Clone, Copy, Debug)]
pub struct RetraceCheck {
    /// Index of the retrace among all retraces
    pub retrace_index: usize,
    /// Index in leaf stream after which content must appear
    pub after_leaf_index: usize,
}

/// Collected spans of all retrace markers in rendered output.
pub struct RenderedSpans {
    pub retrace_spans: Vec<Span>,
}
