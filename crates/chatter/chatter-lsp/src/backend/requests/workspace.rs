use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{SymbolInformation, WorkspaceSymbolParams};

use crate::backend::features;
use crate::backend::state::Backend;

#[allow(deprecated)]
pub(super) async fn handle_workspace_symbol(
    backend: &Backend,
    params: WorkspaceSymbolParams,
) -> Result<Option<Vec<SymbolInformation>>> {
    let query = &params.query;
    let mut all_symbols = Vec::new();

    for entry in backend.documents.iter() {
        let uri = entry.key();
        let doc = entry.value();
        let symbols = features::workspace_symbols_for_document(uri, doc, query);
        all_symbols.extend(symbols);
    }

    if all_symbols.is_empty() {
        Ok(None)
    } else {
        Ok(Some(all_symbols))
    }
}
