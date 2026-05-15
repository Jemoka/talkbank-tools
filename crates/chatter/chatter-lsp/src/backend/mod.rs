//! LSP backend implementation for CHAT format.
//!
//! This crate wires together parser initialization, cache management, diagnostics,
//! requests, and LSP feature handlers (`hover`, `inlay hints`, `diagnostics`, etc.).
//! The backend maintains shared mutable state (`Backend`) and reuses `UnifiedCache`
//! / validation artifacts across feature handlers so they avoid re-parsing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod analysis;
pub mod chat_file_cache;
pub(crate) mod chat_ops;
pub mod contracts;
pub mod diagnostics;
pub mod error;
mod execute_command_args;
pub(crate) mod execute_commands;
pub mod features;
pub mod incremental;
pub(crate) mod language_services;
pub(crate) mod line_offsets;
pub(crate) mod participants;
pub mod utils;
pub mod validation_cache;

mod capabilities;
mod documents;
mod requests;
mod state;

pub use error::{LspBackendError, TierName};
pub use state::ParseState;

use serde_json::Value;
use tower_lsp::LanguageServer;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

pub use state::Backend;

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// Advertise server capabilities during LSP initialization.
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(capabilities::build_initialize_result())
    }

    /// Handle post-initialize client notification.
    async fn initialized(&self, _: InitializedParams) {
        capabilities::log_initialized(&self.client).await;
    }

    /// Allow clean server shutdown.
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// Handle document-open notifications.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        documents::handle_did_open(self, params).await;
    }

    /// Handle document-change notifications.
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        documents::handle_did_change(self, params).await;
    }

    /// Handle document-save notifications.
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        documents::handle_did_save(self, params).await;
    }

    /// Handle document-close notifications.
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        documents::handle_did_close(self, params).await;
    }

    /// Resolve hover requests.
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        requests::handle_hover(self, params).await
    }

    /// Resolve code-action requests.
    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        requests::handle_code_action(self, params).await
    }

    /// Resolve completion requests.
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        requests::handle_completion(self, params).await
    }

    /// Handle execute-command requests.
    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        requests::handle_execute_command(self, params).await
    }

    /// Resolve inlay-hint requests.
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        requests::handle_inlay_hint(self, params).await
    }

    /// Resolve full semantic-token requests.
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        requests::handle_semantic_tokens_full(self, params).await
    }

    /// Resolve range semantic-token requests.
    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        requests::handle_semantic_tokens_range(self, params).await
    }

    /// Resolve document-highlight requests.
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        requests::handle_document_highlight(self, params).await
    }

    /// Resolve document-symbol requests.
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        requests::handle_document_symbol(self, params).await
    }

    /// Resolve folding-range requests.
    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        requests::handle_folding_range(self, params).await
    }

    /// Resolve document-formatting requests.
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        requests::handle_formatting(self, params).await
    }

    /// Resolve go-to-definition requests.
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        requests::handle_goto_definition(self, params).await
    }

    /// Resolve find-references requests.
    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        requests::handle_references(self, params).await
    }

    /// Resolve code-lens requests.
    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        requests::handle_code_lens(self, params).await
    }

    /// Validate that the cursor is on a renameable speaker code.
    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        requests::handle_prepare_rename(self, params).await
    }

    /// Rename a speaker code across all occurrences.
    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        requests::handle_rename(self, params).await
    }

    /// Resolve selection range requests.
    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        requests::handle_selection_range(self, params).await
    }

    /// Resolve linked editing range requests.
    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        requests::handle_linked_editing_range(self, params).await
    }

    /// On-type formatting: auto-insert tab after tier prefix.
    async fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        requests::handle_on_type_formatting(self, params).await
    }

    /// Resolve document link requests (e.g., @Media file references).
    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        requests::handle_document_link(self, params).await
    }

    /// Workspace symbol search across all open CHAT documents.
    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        requests::handle_workspace_symbol(self, params).await
    }

    /// Pull-based diagnostics for a single document (LSP 3.17).
    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let items = self
            .last_diagnostics
            .get(&uri)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            }),
        ))
    }

    /// Pull-based workspace diagnostics across all open documents (LSP 3.17).
    async fn workspace_diagnostic(
        &self,
        _params: WorkspaceDiagnosticParams,
    ) -> Result<WorkspaceDiagnosticReportResult> {
        let items: Vec<WorkspaceDocumentDiagnosticReport> = self
            .last_diagnostics
            .iter()
            .map(|entry| {
                WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                    uri: entry.key().clone(),
                    version: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: None,
                        items: entry.value().clone(),
                    },
                })
            })
            .collect();

        Ok(WorkspaceDiagnosticReportResult::Report(
            WorkspaceDiagnosticReport { items },
        ))
    }
}
