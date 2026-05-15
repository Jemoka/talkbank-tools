//! Speaker extraction for `talkbank/getSpeakers`.

use serde::Serialize;
use talkbank_model::model::{Header, Line};

use crate::backend::state::Backend;

use super::shared::get_document_and_chat_file;
use crate::backend::execute_commands::DocumentUriRequest;

/// One speaker entry returned to the VS Code extension.
#[derive(Serialize)]
struct SpeakerInfo {
    /// CHAT speaker code.
    code: String,
    /// Participant display name.
    name: String,
    /// Participant role label.
    role: String,
}

/// Handle `talkbank/getSpeakers`.
pub(crate) fn handle_get_speakers(
    backend: &Backend,
    request: &DocumentUriRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (_, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let speakers = extract_speakers(&chat_file);
    serde_json::to_value(&speakers).map_err(Into::into)
}

/// Extract declared speakers from the `@Participants` header if present.
fn extract_speakers(chat_file: &talkbank_model::model::ChatFile) -> Vec<SpeakerInfo> {
    for line in &chat_file.lines {
        if let Line::Header { header, .. } = line
            && let Header::Participants { entries } = header.as_ref()
        {
            return entries
                .iter()
                .map(|p| SpeakerInfo {
                    code: p.speaker_code.as_str().to_string(),
                    name: p
                        .name
                        .as_ref()
                        .map_or(String::new(), |n| n.as_str().to_string()),
                    role: p.role.as_str().to_string(),
                })
                .collect();
        }
    }
    vec![]
}
