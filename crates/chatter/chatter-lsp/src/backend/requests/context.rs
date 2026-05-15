//! Shared document, parse-tree, and `ChatFile` lookup helpers for request handlers.

use std::sync::Arc;

use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use crate::backend::LspBackendError;
use crate::backend::chat_file_cache;
use crate::backend::documents;
use crate::backend::state::Backend;

/// Return cached document text for one URI.
pub(super) fn document_text(backend: &Backend, uri: &Url) -> Option<String> {
    documents::get_document_text(backend, uri)
}

/// Return a parsed `ChatFile`, reparsing the document on cache miss.
///
/// Thin re-export of [`chat_file_cache::load_chat_file`] that keeps
/// the per-module name (`get_chat_file`) stable for existing callers
/// while the actual implementation lives in one shared place.
pub(super) fn get_chat_file(
    backend: &Backend,
    uri: &Url,
    doc: &str,
) -> Result<Arc<ChatFile>, LspBackendError> {
    chat_file_cache::load_chat_file(backend, uri, doc)
}

/// Return a parse tree, reparsing and caching it on cache miss.
pub(super) fn get_parse_tree(backend: &Backend, uri: &Url, doc: &str) -> Option<Tree> {
    if let Some(tree) = backend.parse_trees.get(uri) {
        return Some(tree.clone());
    }

    match backend
        .language_services
        .with_parser(|parser| parser.parse_tree_incremental(doc, None))
    {
        Ok(Ok(tree)) => {
            backend.parse_trees.insert(uri.clone(), tree.clone());
            Some(tree)
        }
        _ => None,
    }
}
