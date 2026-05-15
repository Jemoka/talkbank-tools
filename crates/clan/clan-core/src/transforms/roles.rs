//! ROLES — Reassign speaker roles in a CHAT file.
//!
//! Renames speaker codes throughout a CHAT file: in `@Participants`,
//! `@ID` headers, and all main-tier speaker prefixes. This is used
//! to standardize speaker codes across a corpus (e.g., renaming
//! `EXP` to `INV`, or `Child` to `CHI`).
//!
//! # CLAN Equivalence
//!
//! | CLAN command | Rust equivalent |
//! |---|---|
//! | `roles +d"OLD>NEW" file.cha` | `chatter clan roles --rename "OLD=NEW" file.cha` |
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Speaker codes are renamed in all structural locations (participants,
//!   ID headers, utterance speaker fields) via AST manipulation.

use talkbank_model::{ChatFile, Header, Line};

use crate::framework::{TransformCommand, TransformError};

/// Configuration for the ROLES transform.
#[derive(Debug, Clone, Default)]
pub struct RolesConfig {
    /// Mapping of old speaker code → new speaker code.
    pub renames: Vec<(String, String)>,
}

/// ROLES transform: rename speaker codes.
pub struct RolesCommand {
    /// Configuration with rename mappings.
    pub config: RolesConfig,
}

impl TransformCommand for RolesCommand {
    type Config = RolesConfig;

    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for (old, new) in &self.config.renames {
            rename_speaker(file, old, new);
        }
        Ok(())
    }
}

/// Rename all occurrences of a speaker code throughout the file.
fn rename_speaker(file: &mut ChatFile, old: &str, new: &str) {
    for line in file.lines.iter_mut() {
        match line {
            Line::Header { header, .. } => {
                rename_in_header(header, old, new);
            }
            Line::Utterance(utt) => {
                if utt.main.speaker.as_str() == old {
                    utt.main.speaker = new.into();
                }
            }
        }
    }
}

/// Rename speaker code within header lines (@Participants, @ID).
fn rename_in_header(header: &mut Header, old: &str, new: &str) {
    match header {
        Header::Participants { entries } => {
            for p in entries.iter_mut() {
                if p.speaker_code.as_str() == old {
                    p.speaker_code = new.into();
                }
            }
        }
        Header::ID(id) if id.speaker.as_str() == old => {
            id.speaker = new.into();
        }
        _ => {}
    }
}
