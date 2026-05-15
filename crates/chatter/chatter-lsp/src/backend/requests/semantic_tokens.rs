//! Semantic-token request handlers for the LSP backend.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::backend::state::Backend;
use crate::backend::utils;

use super::context::document_text;

/// Handle full-document semantic-token requests.
pub(super) async fn handle_semantic_tokens_full(
    backend: &Backend,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;
    let text = match document_text(backend, &uri) {
        Some(text) => text,
        None => {
            backend
                .client
                .log_message(MessageType::WARNING, format!("Document not found: {}", uri))
                .await;
            return Ok(None);
        }
    };

    let tokens_result = backend
        .language_services
        .with_semantic_tokens_provider(|provider| provider.semantic_tokens_full(&text));

    match tokens_result {
        Ok(tokens) => Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        }))),
        Err(e) => {
            backend
                .client
                .log_message(MessageType::ERROR, format!("Semantic tokens error: {}", e))
                .await;
            Ok(None)
        }
    }
}

/// Handle range semantic-token requests.
pub(super) async fn handle_semantic_tokens_range(
    backend: &Backend,
    params: SemanticTokensRangeParams,
) -> Result<Option<SemanticTokensRangeResult>> {
    let uri = params.text_document.uri;
    let text = match document_text(backend, &uri) {
        Some(text) => text,
        None => return Ok(None),
    };

    let start_offset = utils::position_to_offset(&text, params.range.start);
    let end_offset = utils::position_to_offset(&text, params.range.end);

    let tokens_result = backend
        .language_services
        .with_semantic_tokens_provider(|provider| {
            provider.semantic_tokens_range(&text, start_offset, end_offset)
        });

    match tokens_result {
        Ok(tokens) => Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        }))),
        Err(e) => {
            backend
                .client
                .log_message(
                    MessageType::ERROR,
                    format!("Semantic tokens range error: {}", e),
                )
                .await;
            Ok(None)
        }
    }
}
