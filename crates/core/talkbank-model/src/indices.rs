//! Domain-specific index newtypes used by higher-level CHAT processing passes.
//!
//! These are intentionally simple wrappers over `usize` so callers do not mix
//! utterance positions with per-utterance word positions.

/// Index of an utterance among utterances in one CHAT file, 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UtteranceIdx(pub usize);

impl UtteranceIdx {
    /// Returns the wrapped raw index value.
    pub fn raw(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for UtteranceIdx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Index of a word within one extracted utterance domain, 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WordIdx(pub usize);

impl WordIdx {
    /// Returns the wrapped raw index value.
    pub fn raw(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for WordIdx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
