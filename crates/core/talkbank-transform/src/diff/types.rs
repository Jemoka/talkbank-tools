//! Types for utterance-level diff between two versions of a CHAT file.
//!
//! The core type is [`UtteranceDelta`], which classifies what changed for
//! each utterance between a "before" and "after" version of a file.

use talkbank_model::UtteranceIdx;

/// Classification of what changed for one utterance between before/after.
///
/// Used by the incremental processing engine to determine the minimal set of
/// utterances that need reprocessing after a user edits a CHAT file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UtteranceDelta {
    /// Utterance is identical in both files — preserve all dependent tiers.
    Unchanged {
        /// Index in the "before" file.
        before_idx: UtteranceIdx,
        /// Index in the "after" file.
        after_idx: UtteranceIdx,
    },

    /// Main tier words changed. Timing may or may not have changed.
    ///
    /// Requires reprocessing of morphotag, utseg, and potentially FA
    /// (if the utterance's FA group is affected).
    WordsChanged {
        /// Index in the "before" file.
        before_idx: UtteranceIdx,
        /// Index in the "after" file.
        after_idx: UtteranceIdx,
        /// Whether the utterance-level bullet timing also changed.
        timing_changed: bool,
    },

    /// Only the utterance-level bullet timing changed; words are identical.
    ///
    /// Preserves %mor/%gra but requires FA re-alignment for the affected group.
    TimingOnly {
        /// Index in the "before" file.
        before_idx: UtteranceIdx,
        /// Index in the "after" file.
        after_idx: UtteranceIdx,
    },

    /// Only the speaker code changed; words and timing are identical.
    ///
    /// No NLP reprocessing needed — speaker is metadata.
    SpeakerChanged {
        /// Index in the "before" file.
        before_idx: UtteranceIdx,
        /// Index in the "after" file.
        after_idx: UtteranceIdx,
    },

    /// New utterance in "after" not present in "before".
    ///
    /// Must be processed from scratch.
    Inserted {
        /// Index in the "after" file.
        after_idx: UtteranceIdx,
    },

    /// Utterance in "before" removed from "after".
    ///
    /// Already absent from the output; may affect FA group boundaries.
    Deleted {
        /// Index in the "before" file.
        before_idx: UtteranceIdx,
    },
}

impl UtteranceDelta {
    /// Returns `true` if this delta requires NLP reprocessing (morphotag/utseg).
    pub fn needs_nlp_reprocessing(&self) -> bool {
        matches!(self, Self::WordsChanged { .. } | Self::Inserted { .. })
    }

    /// Returns `true` if this delta affects timing/FA alignment.
    pub fn affects_timing(&self) -> bool {
        matches!(
            self,
            Self::WordsChanged {
                timing_changed: true,
                ..
            } | Self::TimingOnly { .. }
                | Self::Inserted { .. }
                | Self::Deleted { .. }
        )
    }

    /// Returns the "before" utterance index, if any.
    pub fn before_idx(&self) -> Option<UtteranceIdx> {
        match self {
            Self::Unchanged { before_idx, .. }
            | Self::WordsChanged { before_idx, .. }
            | Self::TimingOnly { before_idx, .. }
            | Self::SpeakerChanged { before_idx, .. }
            | Self::Deleted { before_idx } => Some(*before_idx),
            Self::Inserted { .. } => None,
        }
    }

    /// Returns the "after" utterance index, if any.
    pub fn after_idx(&self) -> Option<UtteranceIdx> {
        match self {
            Self::Unchanged { after_idx, .. }
            | Self::WordsChanged { after_idx, .. }
            | Self::TimingOnly { after_idx, .. }
            | Self::SpeakerChanged { after_idx, .. }
            | Self::Inserted { after_idx } => Some(*after_idx),
            Self::Deleted { .. } => None,
        }
    }
}

/// Summary statistics for a diff result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    /// Number of unchanged utterances.
    pub unchanged: usize,
    /// Number of utterances with changed words.
    pub words_changed: usize,
    /// Number of utterances with only timing changes.
    pub timing_only: usize,
    /// Number of utterances with only speaker changes.
    pub speaker_changed: usize,
    /// Number of inserted utterances.
    pub inserted: usize,
    /// Number of deleted utterances.
    pub deleted: usize,
}

impl DiffSummary {
    /// Compute summary from a list of deltas.
    pub fn from_deltas(deltas: &[UtteranceDelta]) -> Self {
        let mut summary = Self {
            unchanged: 0,
            words_changed: 0,
            timing_only: 0,
            speaker_changed: 0,
            inserted: 0,
            deleted: 0,
        };
        for delta in deltas {
            match delta {
                UtteranceDelta::Unchanged { .. } => summary.unchanged += 1,
                UtteranceDelta::WordsChanged { .. } => summary.words_changed += 1,
                UtteranceDelta::TimingOnly { .. } => summary.timing_only += 1,
                UtteranceDelta::SpeakerChanged { .. } => summary.speaker_changed += 1,
                UtteranceDelta::Inserted { .. } => summary.inserted += 1,
                UtteranceDelta::Deleted { .. } => summary.deleted += 1,
            }
        }
        summary
    }

    /// Total number of utterances that need NLP reprocessing.
    pub fn needs_reprocessing(&self) -> usize {
        self.words_changed + self.inserted
    }

    /// Total number of deltas.
    pub fn total(&self) -> usize {
        self.unchanged
            + self.words_changed
            + self.timing_only
            + self.speaker_changed
            + self.inserted
            + self.deleted
    }
}
