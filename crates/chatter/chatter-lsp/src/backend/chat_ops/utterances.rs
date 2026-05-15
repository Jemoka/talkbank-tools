//! Utterance metadata queries for `talkbank/getUtterances`.

use serde::Serialize;
use talkbank_model::model::{DependentTier, Line};

use crate::backend::execute_commands::DocumentUriRequest;
use crate::backend::state::Backend;

use super::line_index::LineIndex;
use super::shared::get_document_and_chat_file;

/// Minimal utterance information returned to coder mode.
#[derive(Serialize)]
struct UtteranceInfo {
    /// Zero-based line number for the main tier.
    line: usize,
    /// Speaker code for the utterance.
    speaker: String,
    /// Whether the utterance already has a `%cod` tier.
    has_cod: bool,
}

/// Handle `talkbank/getUtterances`.
pub(crate) fn handle_get_utterances(
    backend: &Backend,
    request: &DocumentUriRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (text, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let line_index = LineIndex::new(&text);
    let utterances = extract_utterances(&chat_file, &line_index);
    serde_json::to_value(&utterances).map_err(Into::into)
}

/// Extract zero-based line, speaker, and `%cod` presence for each utterance.
fn extract_utterances(
    chat_file: &talkbank_model::model::ChatFile,
    line_index: &LineIndex,
) -> Vec<UtteranceInfo> {
    chat_file
        .lines
        .iter()
        .filter_map(|line| {
            if let Line::Utterance(utterance) = line {
                let main_line = if utterance.main.span.is_dummy() {
                    0
                } else {
                    line_index.byte_to_line(utterance.main.span.start as usize)
                };

                let has_cod = utterance
                    .dependent_tiers
                    .iter()
                    .any(|dt| matches!(dt, DependentTier::Cod(_)));

                Some(UtteranceInfo {
                    line: main_line,
                    speaker: utterance.main.speaker.as_str().to_string(),
                    has_cod,
                })
            } else {
                None
            }
        })
        .collect()
}
