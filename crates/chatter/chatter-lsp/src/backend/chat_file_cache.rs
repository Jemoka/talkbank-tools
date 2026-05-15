//! Single source of truth for "load a parsed `ChatFile` for a URI".
//!
//! All feature handlers that need a parsed document route through
//! [`load_chat_file`]; those that also need the document text use
//! [`load_document_and_chat_file`]. Adding a second copy of this
//! cache-then-reparse shape is banned — extend the helpers here
//! instead. The chat_ops submodule re-exports the pair-variant under
//! the name `get_document_and_chat_file` for historical uniformity.

use std::sync::Arc;

use talkbank_model::ParseErrors;
use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::Url;

use crate::backend::LspBackendError;
use crate::backend::documents;
use crate::backend::state::{Backend, ParseState};

/// Return a parsed [`ChatFile`] for `uri`, reparsing `doc` on cache miss.
///
/// Emits a `tracing::debug!` when a cache hit is being served from a
/// stale baseline so operators can observe KIB-013 occurrences without
/// touching each feature handler (see [`Backend::parse_state`]).
pub(crate) fn load_chat_file(
    backend: &Backend,
    uri: &Url,
    doc: &str,
) -> Result<Arc<ChatFile>, LspBackendError> {
    if let Some(cached) = backend.chat_files.get(uri) {
        if backend.parse_state(uri) == ParseState::StaleBaseline {
            tracing::debug!(
                uri = %uri,
                "serving feature request from stale %mor baseline (KIB-013)",
            );
        }
        return Ok(Arc::clone(cached.value()));
    }

    match backend
        .language_services
        .with_parser(|parser| parser.parse_chat_file(doc))
    {
        Ok(Ok(chat_file)) => Ok(Arc::new(chat_file)),
        Ok(Err(errors)) => Err(parse_failure_from(&errors)),
        Err(init_error) => Err(LspBackendError::LanguageServicesUnavailable(init_error)),
    }
}

/// Load both the current document text and its parsed [`ChatFile`].
///
/// [`LspBackendError::DocumentNotFound`] fires when no cached text
/// exists for the URI (most commonly, the client never opened the
/// file). [`load_chat_file`] supplies the other failure variants.
pub(crate) fn load_document_and_chat_file(
    backend: &Backend,
    uri: &Url,
) -> Result<(String, Arc<ChatFile>), LspBackendError> {
    let text =
        documents::get_document_text(backend, uri).ok_or(LspBackendError::DocumentNotFound)?;
    let chat_file = load_chat_file(backend, uri, &text)?;
    Ok((text, chat_file))
}

/// Convert the parser's collected [`ParseErrors`] into
/// [`LspBackendError::ParseFailure`] (the empty-diagnostics branch is
/// the `count == 0` case of the same variant).
///
/// Kept private: every call that needs a [`ChatFile`] should route
/// through [`load_chat_file`] rather than recreate the error-mapping
/// shape. New code that needs a different classification should add
/// a variant to [`LspBackendError`], not a second copy of this
/// function.
fn parse_failure_from(errors: &ParseErrors) -> LspBackendError {
    LspBackendError::ParseFailure {
        count: errors.errors.len(),
        first_message: errors.errors.first().map(|e| e.message.clone()),
    }
}
