//! LSP request handlers — hover, completion, formatting, goto-definition, and more.
//!
//! The top-level `handle_*` functions here are the request-routing composition root.
//! They delegate request families into focused submodules:
//!
//! - `text_document` for the broader text/document request family
//! - `execute_command` for typed `workspace/executeCommand` routing
//! - single-purpose modules such as `semantic_tokens`, `symbols`, `formatting`,
//!   and `workspace` where one focused seam is already enough
//!
//! Request-family modules resolve document text and cached parse artifacts from
//! the [`Backend`] state, then delegate into the feature-specific logic under
//! [`super::features`].

mod alignment_sidecar;
mod context;
mod execute_command;
mod formatting;
mod goto_definition;
mod semantic_tokens;
mod symbols;
mod text_document;
mod workspace;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use super::state::Backend;

// Re-export tree-sitter utilities used by other modules.
pub(crate) use goto_definition::{find_ancestor_kind, walk_nodes};

pub(super) async fn handle_hover(backend: &Backend, params: HoverParams) -> Result<Option<Hover>> {
    text_document::handle_hover(backend, params).await
}

/// Handles code action.
pub(super) async fn handle_code_action(
    backend: &Backend,
    params: CodeActionParams,
) -> Result<Option<CodeActionResponse>> {
    text_document::handle_code_action(backend, params).await
}

/// Handles completion.
pub(super) async fn handle_completion(
    backend: &Backend,
    params: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    text_document::handle_completion(backend, params).await
}

/// Handles inlay hint.
pub(super) async fn handle_inlay_hint(
    backend: &Backend,
    params: InlayHintParams,
) -> Result<Option<Vec<InlayHint>>> {
    text_document::handle_inlay_hint(backend, params).await
}

/// Handles document highlight.
pub(super) async fn handle_document_highlight(
    backend: &Backend,
    params: DocumentHighlightParams,
) -> Result<Option<Vec<DocumentHighlight>>> {
    text_document::handle_document_highlight(backend, params).await
}

/// Handles semantic tokens full.
pub(super) async fn handle_semantic_tokens_full(
    backend: &Backend,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    semantic_tokens::handle_semantic_tokens_full(backend, params).await
}

/// Handles semantic tokens range.
pub(super) async fn handle_semantic_tokens_range(
    backend: &Backend,
    params: SemanticTokensRangeParams,
) -> Result<Option<SemanticTokensRangeResult>> {
    semantic_tokens::handle_semantic_tokens_range(backend, params).await
}

/// Handles execute command.
pub(super) async fn handle_execute_command(
    backend: &Backend,
    params: ExecuteCommandParams,
) -> Result<Option<Value>> {
    execute_command::handle_execute_command(backend, params).await
}

/// Handles document symbol.
pub(super) async fn handle_document_symbol(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> tower_lsp::jsonrpc::Result<Option<DocumentSymbolResponse>> {
    symbols::handle_document_symbol(backend, params).await
}

/// Handles folding range.
pub(super) async fn handle_folding_range(
    backend: &Backend,
    params: FoldingRangeParams,
) -> tower_lsp::jsonrpc::Result<Option<Vec<FoldingRange>>> {
    symbols::handle_folding_range(backend, params).await
}

/// Handles formatting.
pub(super) async fn handle_formatting(
    backend: &Backend,
    params: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    formatting::handle_formatting(backend, params).await
}

/// Handles find references.
pub(super) async fn handle_references(
    backend: &Backend,
    params: ReferenceParams,
) -> Result<Option<Vec<Location>>> {
    text_document::handle_references(backend, params).await
}

/// Handles code lens.
pub(super) async fn handle_code_lens(
    backend: &Backend,
    params: CodeLensParams,
) -> Result<Option<Vec<CodeLens>>> {
    symbols::handle_code_lens(backend, params).await
}

/// Handles prepare rename.
pub(super) async fn handle_prepare_rename(
    backend: &Backend,
    params: TextDocumentPositionParams,
) -> Result<Option<PrepareRenameResponse>> {
    text_document::handle_prepare_rename(backend, params).await
}

/// Handles rename.
pub(super) async fn handle_rename(
    backend: &Backend,
    params: RenameParams,
) -> Result<Option<WorkspaceEdit>> {
    text_document::handle_rename(backend, params).await
}

/// Handles goto definition.
pub(super) async fn handle_goto_definition(
    backend: &Backend,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    text_document::handle_goto_definition(backend, params).await
}

/// Handles selection range requests.
pub(super) async fn handle_selection_range(
    backend: &Backend,
    params: SelectionRangeParams,
) -> Result<Option<Vec<SelectionRange>>> {
    text_document::handle_selection_range(backend, params).await
}

/// Handles linked editing range requests.
pub(super) async fn handle_linked_editing_range(
    backend: &Backend,
    params: LinkedEditingRangeParams,
) -> Result<Option<LinkedEditingRanges>> {
    text_document::handle_linked_editing_range(backend, params).await
}

pub(super) async fn handle_on_type_formatting(
    backend: &Backend,
    params: DocumentOnTypeFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    text_document::handle_on_type_formatting(backend, params).await
}

pub(super) async fn handle_document_link(
    backend: &Backend,
    params: DocumentLinkParams,
) -> Result<Option<Vec<DocumentLink>>> {
    text_document::handle_document_link(backend, params).await
}

#[allow(deprecated)]
pub(super) async fn handle_workspace_symbol(
    backend: &Backend,
    params: WorkspaceSymbolParams,
) -> Result<Option<Vec<SymbolInformation>>> {
    workspace::handle_workspace_symbol(backend, params).await
}
