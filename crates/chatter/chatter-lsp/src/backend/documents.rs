//! Document-lifecycle handlers (`didOpen`/`didChange`/`didSave`/`didClose`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::sync::atomic::Ordering;
use std::time::Duration;

use tower_lsp::lsp_types::*;

use super::diagnostics;
use super::state::Backend;

/// Debounce delay for validation after document changes (milliseconds)
const DEBOUNCE_MS: u64 = 250;

/// Return cached document text for a URI.
pub(super) fn get_document_text(backend: &Backend, uri: &Url) -> Option<String> {
    backend
        .documents
        .get(uri)
        .map(|entry| entry.value().clone())
}

/// Process `textDocument/didOpen` and trigger immediate validation.
pub(super) async fn handle_did_open(backend: &Backend, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let text = params.text_document.text;
    backend.documents.insert(uri.clone(), text.clone());
    // Immediate validation on open - user expects to see errors right away
    diagnostics::validate_and_publish(
        diagnostics::ValidationResources {
            client: &backend.client,
            language_services: &backend.language_services,
            parse_trees: &backend.parse_trees,
            chat_files: &backend.chat_files,
            parse_clean: &backend.parse_clean,
            validation_cache: &backend.validation_cache,
            last_diagnostics: &backend.last_diagnostics,
        },
        uri,
        &text,
        None,
    )
    .await;
}

/// Process `textDocument/didChange` and schedule debounced validation.
pub(super) async fn handle_did_change(backend: &Backend, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let old_text = backend
        .documents
        .get(&uri)
        .map(|entry| entry.value().clone());

    // Apply content changes to get new text (supports both incremental and full replacement)
    let mut text = old_text.clone().unwrap_or_default();
    for change in params.content_changes {
        if let Some(range) = change.range {
            let start = super::utils::position_to_offset(&text, range.start);
            let end = super::utils::position_to_offset(&text, range.end);
            text.replace_range(start..end, &change.text);
        } else {
            // Full replacement fallback (client sends no range)
            text = change.text;
        }
    }

    backend.documents.insert(uri.clone(), text);

    // Generate unique validation ID and store it
    let validation_id = backend.validation_counter.fetch_add(1, Ordering::SeqCst);
    backend
        .pending_validations
        .insert(uri.clone(), validation_id);

    // Clone backend for the spawned task
    let backend = backend.clone();
    let uri_clone = uri.clone();

    // Spawn debounced validation task
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;

        // Only validate if this is still the latest change for this document
        if let Some(current_id) = backend.pending_validations.get(&uri_clone)
            && *current_id == validation_id
            && let Some(text) = backend.documents.get(&uri_clone)
        {
            diagnostics::validate_and_publish(
                diagnostics::ValidationResources {
                    client: &backend.client,
                    language_services: &backend.language_services,
                    parse_trees: &backend.parse_trees,
                    chat_files: &backend.chat_files,
                    parse_clean: &backend.parse_clean,
                    validation_cache: &backend.validation_cache,
                    last_diagnostics: &backend.last_diagnostics,
                },
                uri_clone,
                &text,
                old_text.as_deref(),
            )
            .await;
        }
    });
}

/// Process `textDocument/didSave`, validating only when cache is missing.
pub(super) async fn handle_did_save(backend: &Backend, params: DidSaveTextDocumentParams) {
    // Skip if validation cache already exists — did_open or did_change already validated.
    // This avoids redundant full re-validation when saving right after editing.
    if backend
        .validation_cache
        .contains_key(&params.text_document.uri)
    {
        return;
    }
    if let Some(text) = get_document_text(backend, &params.text_document.uri) {
        diagnostics::validate_and_publish(
            diagnostics::ValidationResources {
                client: &backend.client,
                language_services: &backend.language_services,
                parse_trees: &backend.parse_trees,
                chat_files: &backend.chat_files,
                parse_clean: &backend.parse_clean,
                validation_cache: &backend.validation_cache,
                last_diagnostics: &backend.last_diagnostics,
            },
            params.text_document.uri,
            &text,
            None,
        )
        .await;
    }
}

/// Process `textDocument/didClose` and evict all per-document caches/state.
pub(super) async fn handle_did_close(backend: &Backend, params: DidCloseTextDocumentParams) {
    let uri = &params.text_document.uri;
    backend.documents.remove(uri);
    backend.pending_validations.remove(uri);
    backend.parse_trees.remove(uri);
    backend.chat_files.remove(uri);
    backend.parse_clean.remove(uri);
    backend.validation_cache.remove(uri);
    backend.last_diagnostics.remove(uri);
    backend
        .client
        .publish_diagnostics(params.text_document.uri, vec![], None)
        .await;
}
