//! CHAT text serialization for full `ChatFile` values.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! The writer assumes `self.lines` is already normalized, so headers carry
//! canonical label metadata and utterance writers are responsible for trailing
//! newline placement.

use super::ChatFile;
use crate::validation::ValidationState;
use crate::{Line, WriteChat};

impl<S: ValidationState> WriteChat for ChatFile<S> {
    /// Serializes file contents in stored line order.
    ///
    /// Preserves header/utterance interleaving exactly as stored in `self.lines`.
    /// This guarantees deterministic roundtrip output for files that contain
    /// interstitial headers between utterances. Header lines are newline-
    /// terminated here, while utterance writers own their own trailing-line
    /// behavior to match CHAT multi-line tier formatting.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for line in &self.lines {
            match line {
                Line::Header { header, .. } => {
                    header.write_chat(w)?;
                    w.write_char('\n')?;
                }
                Line::Utterance(u) => {
                    // Utterances already include required trailing newlines.
                    u.write_chat(w)?;
                }
            }
        }
        Ok(())
    }
}

impl<S: ValidationState> std::fmt::Display for ChatFile<S> {
    /// Formats this file using canonical CHAT serialization.
    ///
    /// Intended for diagnostics/debugging; use [`WriteChat`] for buffer-oriented flows.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
