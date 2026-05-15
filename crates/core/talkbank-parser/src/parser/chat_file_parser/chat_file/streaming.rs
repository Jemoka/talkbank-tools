//! Streaming CHAT parse entry points.
//!
//! These APIs keep the same recovery behavior as strict parsing but route diagnostics through an
//! `ErrorSink` so callers can surface errors incrementally.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::parse_lines;
use super::normalize::{headers_enable_ca_mode, normalize_ca_omissions};
use crate::error::ErrorSink;
use crate::model::{ChatFile, Header, Line};
use crate::parser::TreeSitterParser;
use crate::parser::participants::build_participants_from_lines;
use talkbank_model::LineMap;
use tracing::{debug, info};

impl TreeSitterParser {
    /// Parse a complete CHAT transcript file with streaming error output.
    ///
    /// Errors are reported to the `errors` sink as they're discovered, enabling:
    /// - Early cancellation when user has seen enough errors
    /// - Real-time error display in GUI applications
    /// - Memory-efficient processing of large files
    ///
    /// Unlike `parse_chat_file()`, this method always returns a ChatFile (with error recovery),
    /// and streams errors via the sink instead of returning them.
    #[tracing::instrument(skip(self, input, errors), fields(input_size = input.len()))]
    pub fn parse_chat_file_streaming(&self, input: &str, errors: &impl ErrorSink) -> ChatFile {
        debug!("Parsing CHAT file ({} bytes) with streaming", input.len());

        let mut lines = parse_lines(self, input, errors);

        // Build participant map from headers
        let all_headers: Vec<Header> = lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { header, .. } => Some(header.as_ref().clone()),
                _ => None,
            })
            .collect();

        let (participants, participant_errors) = build_participants_from_lines(&lines);

        let ca_mode = headers_enable_ca_mode(&all_headers);
        if ca_mode {
            normalize_ca_omissions(&mut lines);
        }

        // Stream participant errors
        for err in participant_errors {
            errors.report(err);
        }

        info!(
            "Streaming parse completed: {} lines, {} participants",
            lines.len(),
            participants.len()
        );

        ChatFile::with_line_map(lines, participants, LineMap::new(input))
    }
}
