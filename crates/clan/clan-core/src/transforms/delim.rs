//! DELIM -- add missing utterance terminators.
//!
//! Reimplements CLAN's `delim` command, which ensures every main tier has a
//! terminator. Utterances missing a terminator (`.`, `?`, `!`) receive a
//! default period (`.`). This is typically used as a repair step for files
//! imported from external formats that lack CHAT punctuation conventions.
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Checks and sets the structured `Terminator` field on the AST instead of
//!   scanning line endings for punctuation characters.
//! - CLAN writes an empty `.cex` file when no changes are needed; this
//!   implementation always writes the full file (divergence accepted).

use talkbank_model::{ChatFile, Line, Terminator};

use crate::framework::{TransformCommand, TransformError};

/// DELIM transform: add missing terminators to main tiers.
pub struct DelimCommand;

impl TransformCommand for DelimCommand {
    type Config = ();

    /// Ensure each main tier has a terminator, defaulting to period when absent.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utterance) = line
                && utterance.main.content.terminator.is_none()
            {
                utterance.main.content.terminator = Some(Terminator::Period {
                    span: talkbank_model::Span::DUMMY,
                });
            }
        }

        Ok(())
    }
}
