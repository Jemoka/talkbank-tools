//! Test module for validation in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::Path;
use talkbank_model::Line;
use talkbank_parser::TreeSitterParser;

use crate::stats::AlignmentStats;

/// Enum variants for ValidationError.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Missing filename for path {path}")]
    MissingFileName { path: String },
    #[error("Failed to read {path}: {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to parse {path}: {errors:?}")]
    ParseError {
        path: String,
        errors: talkbank_model::ParseErrors,
    },
}

/// Validates file.
pub fn validate_file(
    parser: &TreeSitterParser,
    path: &Path,
    stats: &mut AlignmentStats,
) -> Result<(), ValidationError> {
    let file_name = path.file_name().ok_or(ValidationError::MissingFileName {
        path: path.display().to_string(),
    })?;
    let filename = file_name.to_string_lossy();

    let content = fs::read_to_string(path).map_err(|source| ValidationError::ReadError {
        path: path.display().to_string(),
        source,
    })?;

    let chat_file =
        parser
            .parse_chat_file(&content)
            .map_err(|errors| ValidationError::ParseError {
                path: path.display().to_string(),
                errors,
            })?;

    stats.note_file();
    let mut file_has_errors = false;

    for line in &chat_file.lines {
        if let Line::Utterance(utterance) = line {
            stats.note_utterance();

            if !utterance.alignments_valid() {
                stats.note_utterance_with_errors();

                if !file_has_errors {
                    stats.note_file_with_errors();
                    file_has_errors = true;
                    eprintln!("\n{} has alignment errors:", filename);
                }

                let errors = utterance.collect_alignment_errors();
                stats.note_alignment_errors(errors.len());

                eprintln!(
                    "  Utterance ({}): {} errors",
                    utterance.main.speaker,
                    errors.len()
                );
                for error in errors {
                    eprintln!("    - {}", error);
                }
            }
        }
    }

    Ok(())
}
