//! Filesystem-oriented pipeline helpers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::fs;
use std::path::Path;

use talkbank_model::ChatFile;
use talkbank_model::ParseValidateOptions;

use super::error::PipelineError;
use super::parse::parse_and_validate;

/// Read a CHAT file from disk, then parse/validate using pipeline options.
///
/// # Arguments
///
/// * `path` - Path to CHAT file
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(ChatFile)` - Successfully parsed (and validated if requested)
/// * `Err(PipelineError)` - I/O, parse, or validation errors
pub fn parse_file_and_validate(
    path: &Path,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    let content = fs::read_to_string(path)?;
    parse_and_validate(&content, options)
}
