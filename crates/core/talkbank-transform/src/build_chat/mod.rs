//! Build a CHAT file from a structured transcript description.
//!
//! This module constructs a [`ChatFile`] AST from structured input — either
//! a JSON transcript description (for PyO3 bridge compatibility) or typed
//! Rust structs (for the Rust server's transcribe orchestrator).
//!
//! The implementation is split by responsibility so contributors can find the
//! schema bridge, parser setup, header synthesis, and utterance assembly logic
//! without paging through the full end-to-end pipeline in one file.
//!
//! # Two entry points
//!
//! - [`build_chat`] — takes a typed [`TranscriptDescription`] struct
//! - [`build_chat_from_json`] — deserializes JSON into `TranscriptDescription`,
//!   then calls `build_chat`. Used by the PyO3 bridge to delegate here.
//!
//! # Convenience
//!
//! - [`transcript_from_asr_utterances`] — converts post-processed ASR
//!   utterances into a `TranscriptDescription` for CHAT assembly.

mod bridge;
mod headers;
mod parser;
mod schema;
#[cfg(test)]
mod tests;
mod utterances;

use talkbank_model::model::{
    ChatFile, Header, IDHeader, Line, Participant, ParticipantEntry, ParticipantName,
    ParticipantRole, SpeakerCode,
};

pub use bridge::{TranscriptBuildError, build_chat_from_json, transcript_from_asr_utterances};
pub use schema::{ParticipantDesc, TranscriptDescription, UtteranceDesc, WordDesc};
pub use utterances::tag_marker_separator;

use headers::build_header_lines;
use parser::BuildChatContext;
use utterances::build_utterance_lines;

/// Build a CHAT file from a typed transcript description.
pub fn build_chat(desc: &TranscriptDescription) -> Result<ChatFile, String> {
    if desc.participants.is_empty() {
        return Err("At least one participant is required".to_string());
    }

    let context = BuildChatContext::new(desc)?;
    let mut lines = build_header_lines(desc, context.langs());
    lines.extend(build_utterance_lines(
        desc,
        context.parser(),
        context.langs(),
        context.primary_lang(),
    )?);
    lines.push(Line::header(Header::End));

    // `ChatFile::new` intentionally leaves its derived participant map empty;
    // it is meant for parser intermediates before @Participants/@ID
    // reconciliation. Programmatically assembled CHAT must carry the same
    // metadata as parsed CHAT, because stage validation and downstream
    // transforms consult this map rather than re-scanning header lines.
    let lang_code = context.langs().first().map(String::as_str).unwrap_or("eng");
    let participants = desc
        .participants
        .iter()
        .map(|participant| {
            let code = SpeakerCode::new(participant.id.as_str());
            let role = ParticipantRole::new(participant.role.as_str());
            let entry = ParticipantEntry {
                speaker_code: code.clone(),
                name: participant.name.as_ref().map(ParticipantName::new),
                role: role.clone(),
            };
            let corpus = if participant.corpus.is_empty() {
                "corpus_name"
            } else {
                participant.corpus.as_str()
            };
            let id = IDHeader::new(lang_code, code.clone(), role).with_corpus(corpus);
            (code, Participant::new(entry, id))
        })
        .collect();
    Ok(ChatFile::with_participants(lines, participants))
}
