//! Normalize command - convert CHAT to canonical format.
//!
//! This command builds a canonical CHAT representation using `talkbank-transform`,
//! optionally runs validation/alignment, and writes the normalized transcript.
//! It is used by tooling that needs normalized output for downstream processing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::fs;
use std::path::PathBuf;
use tracing::{Level, debug, info, span, warn};

use crate::output::print_errors;

/// Normalize a CHAT file to the canonical format defined by the CHAT File Format and Header sections.
///
/// The command re-parses the transcript, optionally re-applies validation/alignment, and emits
/// a canonical form where lines, headers, and formatting match the transformed output described
/// in the manual. This is the same normalized representation other commands (e.g., `clean`, `validate`)
/// work from when checking for manual compliance.
pub fn normalize_chat(
    input: &PathBuf,
    output: Option<&PathBuf>,
    validate: bool,
    skip_alignment: bool,
) {
    let _span = span!(Level::INFO, "normalize_chat", input = %input.display()).entered();
    info!("Normalizing CHAT file to canonical format");

    // Read CHAT file
    let content = {
        let _span = span!(Level::DEBUG, "read_file").entered();
        match fs::read_to_string(input) {
            Ok(c) => {
                debug!("Read {} bytes from file", c.len());
                c
            }
            Err(e) => {
                warn!("Failed to read file: {}", e);
                eprintln!("Error reading file {:?}: {}", input, e);
                std::process::exit(1);
            }
        }
    };

    // Build pipeline options that control validation/alignment, matching the manual’s discussion of `%wor` alignment costs.
    let mut options = talkbank_model::ParseValidateOptions::default();
    if validate {
        if skip_alignment {
            options = options.with_validation();
        } else {
            options = options.with_alignment();
        }
    }

    // Use pipeline function to parse, validate, and normalize
    let canonical_chat = {
        let _span = span!(Level::DEBUG, "pipeline").entered();
        match talkbank_transform::normalize_chat(&content, options) {
            Ok(chat_str) => {
                debug!("Pipeline successful, {} bytes", chat_str.len());
                if validate {
                    info!("✓ Validation passed");
                    eprintln!("✓ Validation passed");
                }
                chat_str
            }
            Err(e) => {
                match e {
                    talkbank_transform::PipelineError::Validation(errors) => {
                        warn!("Validation found {} errors", errors.len());
                        eprintln!("✗ Validation errors found:");
                        print_errors(input, &content, &errors);
                    }
                    _ => {
                        warn!("Pipeline error: {}", e);
                        eprintln!("Error: {}", e);
                    }
                }
                std::process::exit(1);
            }
        }
    };

    // Write or print canonical CHAT
    if let Some(output_path) = output {
        let _span = span!(Level::DEBUG, "write_output").entered();
        if let Err(e) = fs::write(output_path, canonical_chat) {
            warn!("Failed to write output: {}", e);
            eprintln!("Error writing CHAT to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        info!(
            "Normalized {} to {}",
            input.display(),
            output_path.display()
        );
        eprintln!(
            "✓ Normalized {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        print!("{}", canonical_chat);
    }
}
