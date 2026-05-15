//! Execute-command request dispatch for the LSP backend.

use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::ExecuteCommandParams;

use crate::backend::execute_commands::{
    DocumentPositionRequest, DocumentUriRequest, ExecuteCommandFamily, ExecuteCommandRequest,
};
use crate::backend::state::Backend;
use crate::backend::utils;
use crate::backend::{
    analysis::AnalysisCommandService, chat_ops::ChatOpsCommandService,
    participants::ParticipantCommandService,
};

use super::alignment_sidecar;
use super::context::{document_text, get_chat_file};

/// Composition root for execute-command feature services.
struct ExecuteCommandServices {
    documents: DocumentCommandService,
    analysis: AnalysisCommandService,
    participants: ParticipantCommandService,
    chat_ops: ChatOpsCommandService,
}

impl ExecuteCommandServices {
    /// Create the stateless service set used to route execute-command requests.
    fn new() -> Self {
        Self {
            documents: DocumentCommandService,
            analysis: AnalysisCommandService,
            participants: ParticipantCommandService,
            chat_ops: ChatOpsCommandService,
        }
    }

    /// Route one typed execute-command request to its feature service.
    fn dispatch(&self, backend: &Backend, request: ExecuteCommandRequest) -> Result<Option<Value>> {
        match request.family() {
            ExecuteCommandFamily::Documents => self.documents.dispatch(backend, request),
            ExecuteCommandFamily::Analysis => self.analysis.dispatch(request),
            ExecuteCommandFamily::Participants => self.participants.dispatch(backend, request),
            ExecuteCommandFamily::ChatOps => self.chat_ops.dispatch(backend, request),
        }
    }
}

/// Service object for document-local graph and alignment commands.
struct DocumentCommandService;

impl DocumentCommandService {
    /// Dispatch one document-command request.
    fn dispatch(&self, backend: &Backend, request: ExecuteCommandRequest) -> Result<Option<Value>> {
        // Routing invariant: only `ExecuteCommandFamily::Documents`
        // variants reach here via the family() partitioning above.
        // Follow-up: typed sub-enum per family — see
        // `book/src/operations/panic-audit/chatter-lsp.md`.
        #[allow(clippy::unreachable)]
        match request {
            ExecuteCommandRequest::ShowDependencyGraph(request) => {
                self.handle_dependency_graph_command(backend, &request)
            }
            ExecuteCommandRequest::GetAlignmentSidecar(request) => {
                self.handle_alignment_sidecar_command(backend, &request)
            }
            _ => unreachable!("document service received unsupported execute-command request"),
        }
    }

    /// Resolve an alignment-sidecar request against one open document.
    fn handle_alignment_sidecar_command(
        &self,
        backend: &Backend,
        request: &DocumentUriRequest,
    ) -> Result<Option<Value>> {
        let text = match document_text(backend, &request.uri) {
            Some(text) => text,
            None => return Ok(Some(Value::String("Document not found".to_string()))),
        };
        let chat_file = match get_chat_file(backend, &request.uri, &text) {
            Ok(file) => file,
            Err(error) => return Ok(Some(Value::String(error.to_string()))),
        };

        let sidecar = alignment_sidecar::build_alignment_sidecar(&request.uri, &text, &chat_file);
        match serde_json::to_value(sidecar) {
            Ok(value) => Ok(Some(value)),
            Err(err) => Ok(Some(Value::String(format!(
                "Sidecar serialization error: {}",
                err
            )))),
        }
    }

    /// Resolve a dependency-graph request at one cursor position.
    fn handle_dependency_graph_command(
        &self,
        backend: &Backend,
        request: &DocumentPositionRequest,
    ) -> Result<Option<Value>> {
        let text = match document_text(backend, &request.uri) {
            Some(text) => text,
            None => return Ok(Some(Value::String("Document not found".to_string()))),
        };
        let chat_file = match get_chat_file(backend, &request.uri, &text) {
            Ok(file) => file,
            Err(error) => return Ok(Some(Value::String(error.to_string()))),
        };
        let utterance = match utils::find_utterance_at_position(&chat_file, request.position, &text)
        {
            Some(u) => u,
            None => {
                return Ok(Some(Value::String(
                    "No utterance at cursor position".to_string(),
                )));
            }
        };

        let response = crate::graph::build_dependency_graph_response(
            utterance,
            backend.parse_state(&request.uri),
        );
        match serde_json::to_value(response) {
            Ok(value) => Ok(Some(value)),
            Err(err) => Ok(Some(Value::String(format!(
                "Failed to serialize dependency-graph response: {}",
                err
            )))),
        }
    }
}

/// Decode and dispatch one `workspace/executeCommand` request.
pub(super) async fn handle_execute_command(
    backend: &Backend,
    params: ExecuteCommandParams,
) -> Result<Option<Value>> {
    let request = match ExecuteCommandRequest::parse(params) {
        Ok(request) => request,
        Err(error) => return Ok(Some(Value::String(error.to_string()))),
    };

    ExecuteCommandServices::new().dispatch(backend, request)
}
