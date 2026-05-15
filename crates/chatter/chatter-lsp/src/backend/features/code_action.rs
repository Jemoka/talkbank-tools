//! Quick-fix code actions derived from diagnostics.
//!
//! This module is the composition root for diagnostic-driven code actions.
//! Per-diagnostic fix routing lives in `code_action_fixes.rs`, while
//! `code_action_builders.rs` owns the repeated `WorkspaceEdit` construction.

#[path = "code_action_builders.rs"]
mod builders;
#[path = "code_action_fixes.rs"]
mod fixes;

use tower_lsp::lsp_types::*;

/// Build quick-fix actions from server diagnostics for a document.
pub fn code_action(
    uri: Url,
    diagnostics: Vec<Diagnostic>,
    doc: Option<&str>,
) -> Option<Vec<CodeActionOrCommand>> {
    let actions = diagnostics
        .into_iter()
        .flat_map(|diagnostic| fixes::actions_for_diagnostic(&uri, &diagnostic, doc))
        .map(CodeActionOrCommand::CodeAction)
        .collect::<Vec<_>>();

    (!actions.is_empty()).then_some(actions)
}

#[cfg(test)]
#[path = "code_action_tests.rs"]
mod tests;
