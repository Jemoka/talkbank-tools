//! CHAT document operations for the LSP.
//!
//! Provides execute-command handlers that replace ad-hoc string/regex parsing
//! in the TypeScript extension with proper model-based operations:
//!
//! - `talkbank/getSpeakers` — extract declared speaker codes from `@Participants`
//! - `talkbank/filterDocument` — filter a document to selected speakers
//! - `talkbank/getUtterances` — list utterances with speaker, line, and coded status
//! - `talkbank/formatBulletLine` — construct a timing bullet and new utterance line
//! - `talkbank/scopedFind` — semantically scoped search across tiers and speakers

use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;

use super::execute_commands::ExecuteCommandRequest;
use super::state::Backend;

mod filter_document;
mod format_bullet;
mod line_index;
mod scoped_find;
mod shared;
mod speakers;
#[cfg(test)]
mod tests;
mod utterances;

pub(crate) use filter_document::handle_filter_document;
pub(crate) use format_bullet::handle_format_bullet_line;
pub(crate) use scoped_find::handle_scoped_find;
pub(crate) use speakers::handle_get_speakers;
pub(crate) use utterances::handle_get_utterances;

/// Feature-oriented execute-command service for CHAT document operations.
pub(crate) struct ChatOpsCommandService;

impl ChatOpsCommandService {
    /// Dispatch one chat-ops-family execute-command request.
    pub(crate) fn dispatch(
        &self,
        backend: &Backend,
        request: ExecuteCommandRequest,
    ) -> LspResult<Option<Value>> {
        // Routing invariant: `ExecuteCommandRoutingService::dispatch`
        // partitions variants by `request.family()` and only forwards
        // `ExecuteCommandFamily::ChatOps` variants here. Follow-up:
        // typed sub-enum per family — see
        // `book/src/operations/panic-audit/chatter-lsp.md`.
        #[allow(clippy::unreachable)]
        match request {
            ExecuteCommandRequest::GetSpeakers(request) => {
                command_response(handle_get_speakers(backend, &request), "Speaker error")
            }
            ExecuteCommandRequest::FilterDocument(request) => {
                command_response(handle_filter_document(backend, &request), "Filter error")
            }
            ExecuteCommandRequest::GetUtterances(request) => {
                command_response(handle_get_utterances(backend, &request), "Utterance error")
            }
            ExecuteCommandRequest::FormatBulletLine(request) => {
                command_response(handle_format_bullet_line(&request), "Format error")
            }
            ExecuteCommandRequest::ScopedFind(request) => {
                command_response(handle_scoped_find(backend, &request), "Search error")
            }
            _ => unreachable!("chat-ops service received unsupported execute-command request"),
        }
    }
}

fn command_response(
    result: Result<Value, crate::backend::LspBackendError>,
    prefix: &str,
) -> LspResult<Option<Value>> {
    match result {
        Ok(json) => Ok(Some(json)),
        // Stringify at the LSP wire boundary; the JSON-RPC response
        // carries the `Display`-formatted message so the TS client
        // sees the same user-facing text the untyped path produced.
        Err(error) => Ok(Some(Value::String(format!("{prefix}: {error}")))),
    }
}
