//! XML conversion command (`chatter to-xml`).
//!
//! Exports one CHAT transcript to TalkBank XML using the Rust-side XML writer in
//! `talkbank-transform`. This is an export-only surface; XML ingest is not
//! implemented in the toolchain.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::fs;
use std::path::PathBuf;

use tracing::{Level, debug, info, span, warn};

use crate::output::print_errors;

/// Convert one CHAT file to TalkBank XML.
///
/// The command validates the CHAT file before export so downstream XML consumers
/// receive the same well-formed transcript structure that other `chatter`
/// conversion commands require. Alignment checks run by default and may be
/// disabled with `--skip-alignment`.
pub fn chat_to_xml(input: &PathBuf, output: Option<&PathBuf>, skip_alignment: bool) {
    let _span = span!(Level::INFO, "chat_to_xml", input = %input.display()).entered();
    info!("Converting CHAT to XML");

    let content = match fs::read_to_string(input) {
        Ok(c) => {
            debug!("Read {} bytes from file", c.len());
            c
        }
        Err(e) => {
            warn!("Failed to read file: {}", e);
            eprintln!("Error reading file {:?}: {}", input, e);
            std::process::exit(1);
        }
    };

    let mut options = talkbank_model::ParseValidateOptions::default().with_validation();
    if !skip_alignment {
        options = options.with_alignment();
    }

    let chat_file = match talkbank_transform::parse_and_validate(&content, options) {
        Ok(chat_file) => chat_file,
        Err(e) => {
            match e {
                talkbank_transform::PipelineError::Validation(errors) => {
                    eprintln!("✗ Validation errors found:");
                    print_errors(input, &content, &errors);
                }
                _ => eprintln!("Error: {}", e),
            }
            std::process::exit(1);
        }
    };

    let xml = match talkbank_transform::xml::write_chat_xml(&chat_file) {
        Ok(xml) => xml,
        Err(e) => {
            eprintln!("✗ XML export error: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(output_path) = output {
        if let Err(e) = fs::write(output_path, &xml) {
            eprintln!("Error writing XML to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        eprintln!(
            "✓ Converted {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        print!("{}", xml);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn chat_to_xml_writes_xml_file() {
        let dir = tempdir().expect("create tempdir");
        let input = dir.path().join("sample.cha");
        let output = dir.path().join("sample.xml");

        fs::write(
            &input,
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n",
        )
        .expect("write sample chat");

        chat_to_xml(&input, Some(&output), false);

        let xml = fs::read_to_string(&output).expect("read xml output");
        assert!(xml.contains("<CHAT"));
        assert!(xml.contains("<w>hello</w>"));
    }
}
