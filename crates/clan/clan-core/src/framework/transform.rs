//! Transform command infrastructure for file-modifying CLAN commands.
//!
//! Unlike analysis commands which read files and produce statistics,
//! transform commands take a [`ChatFile`] as input and produce a modified
//! [`ChatFile`] as output. Examples: FLO (simplified main line), LOWCASE
//! (lowercase words), CHSTRING (string replacement), DATES (age computation).
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for the
//! original transform command semantics.
//!
//! # Transform pipeline
//!
//! The [`run_transform()`] function implements the standard pipeline:
//!
//! 1. Read and parse input file (no validation needed for transforms)
//! 2. Call [`TransformCommand::transform()`] on the parsed [`ChatFile`]
//! 3. Serialize the modified file back to CHAT format
//! 4. Write to stdout (default) or to the specified output file
//!
//! Some transforms (DATACLEAN, LINES) use custom run functions
//! because they operate on serialized text rather than the AST.

use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::Path;

use talkbank_model::ParseValidateOptions;
use talkbank_model::{ChatFile, WriteChat};

/// Errors that can occur during file transformation.
#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    /// I/O error (reading input, writing output)
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Parse error (input file is not valid CHAT)
    #[error("Parse error: {0}")]
    Parse(String),

    /// Transform-specific error (command logic failure)
    #[error("Transform error: {0}")]
    Transform(String),
}

/// Trait that all file-transforming CLAN commands implement.
///
/// Transform commands operate on a mutable [`ChatFile`] reference,
/// modifying it in place. [`ChatFileLines`](talkbank_model::ChatFileLines)
/// implements `DerefMut<Target = Vec<Line>>`, enabling efficient in-place
/// mutation of the line sequence.
pub trait TransformCommand {
    /// Command-specific configuration parsed from CLI args.
    type Config;

    /// Apply the transformation to the parsed CHAT file.
    ///
    /// The command may modify any aspect of the file: add/remove lines,
    /// modify utterance content, strip dependent tiers, etc.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError>;
}

/// Run a transform command on an input file.
///
/// Pipeline: read → parse → transform → serialize → write.
/// Output goes to stdout by default, or to the specified output file.
pub fn run_transform(
    command: &impl TransformCommand,
    input: &Path,
    output: Option<&Path>,
) -> Result<(), TransformError> {
    // Read input file
    let content = fs::read_to_string(input).map_err(TransformError::Io)?;

    // Parse (no validation needed for transforms)
    let mut chat_file =
        talkbank_transform::parse_and_validate(&content, ParseValidateOptions::default())
            .map_err(|e| TransformError::Parse(e.to_string()))?;

    // Apply transform
    command.transform(&mut chat_file)?;

    // Serialize back to CHAT format
    let output_str = chat_file.to_chat_string();

    // Write output
    if let Some(output_path) = output {
        fs::write(output_path, output_str).map_err(TransformError::Io)?;
    } else {
        io::stdout()
            .write_all(output_str.as_bytes())
            .map_err(TransformError::Io)?;
    }

    Ok(())
}
