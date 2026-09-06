# Adding Features

**Status:** Current
**Last updated:** 2026-04-16 22:07 EDT

Step-by-step for adding a new LSP capability. The pattern is
consistent across all features; follow it and the compiler will
refuse to let you leave a capability half-wired.

## Standard LSP capability

### 1. Advertise the capability

Edit `crates/chatter-lsp/src/backend/capabilities.rs` and add the
provider to `build_initialize_result()`:

```text
// Example: adding rename support
rename_provider: Some(OneOf::Left(true)),
```

Without this, VS Code never sends requests for the feature.

### 2. Create the feature handler

Add a new file in `crates/chatter-lsp/src/backend/features/`
(e.g. `rename.rs`). Handlers are pure functions over backend state:

```rust,ignore
use tower_lsp::lsp_types::*;
use crate::backend::LspBackendError;
use crate::backend::chat_file_cache::load_chat_file;
use crate::backend::state::Backend;

pub(crate) fn handle_rename(
    backend: &Backend,
    params: RenameParams,
) -> Result<Option<WorkspaceEdit>, LspBackendError> {
    let uri = &params.text_document_position.text_document.uri;
    let text = backend
        .documents()
        .get_text(uri)
        .ok_or(LspBackendError::DocumentNotFound)?;
    let chat_file = load_chat_file(backend, uri, &text)?;
    // ... build the WorkspaceEdit from the typed model
    Ok(Some(edit))
}
```

Two invariants:

- **Return `Result<_, LspBackendError>`**, never a stringly
  `Result<_, String>`. Add a new variant to `LspBackendError` (or a
  subsystem sub-enum like `GraphEdgeError`) when the failure
  doesn't fit an existing one.
- **Route through `chat_file_cache::load_chat_file`** (or its pair
  variant `load_document_and_chat_file`) for parsed-document access.
  Do not re-implement the cache-then-reparse shape. The single loader
  emits the `tracing::debug!` used for stale-baseline observability.

### 3. Wire it into the LanguageServer trait

Edit the appropriate request-handler module under
`crates/chatter-lsp/src/backend/requests/`:

```rust,ignore
async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    Ok(features::rename::handle_rename(&self.backend, params)
        .map_err(stringify_for_lsp)
        .ok()
        .flatten())
}
```

The `tower-lsp` trait's return type is `jsonrpc::Result<_>` so the
typed error collapses to a stringified LSP error at the protocol
boundary. Everything before that point stays typed.

### 4. Register the feature module

Add the new module to `backend/features/mod.rs`:

```rust,ignore
pub(crate) mod rename;
```

### 5. Write tests

Add a `#[cfg(test)] mod tests` inside the feature file. Use the
shared fixtures:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{parse_chat, parse_tree};

    #[test]
    fn rename_emits_workspace_edit() {
        let content = "@UTF8\n@Begin\n...\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // ... call handle_rename and assert
    }
}
```

Never redefine `parse_chat` / `parse_tree` locally —
`crate::test_fixtures` owns the shared helpers. Duplicating these
helpers in a new test module leads to drift between how production
and test code construct CHAT inputs and has caused real bugs to slip
through in the past.

## Standard handler pattern

Every feature handler follows the same shape:

1. Pull the document text via `backend.documents().get_text(uri)`.
2. Get (or cache-miss-then-parse) the `ChatFile` via
   `chat_file_cache::load_chat_file` (or `_document_and_chat_file`
   when text is also needed).
3. Get the tree-sitter `Tree` via `backend.language_services` when
   the handler walks the CST rather than the model.
4. Compute the result using the typed model (`ChatFile`,
   `AlignmentSet`, `MorTier`, …). Never re-parse CHAT text.
5. Convert to LSP types and return.

All CHAT semantics live in `talkbank-model`; the LSP layer is format
conversion and routing. If a feature needs a new primitive on the
model (e.g. a chunk-walking iterator), add it in `talkbank-model`
and delegate — do not grow the primitive in the LSP crate. See
[`crates/chatter-lsp/CLAUDE.md`][lsp-claude].

[lsp-claude]: https://github.com/TalkBank/batchalign/blob/main/crates/chatter-lsp/CLAUDE.md

## Adding a VS Code command

### In `package.json`

Add the command declaration:

```json
{
    "command": "talkbank.myFeature",
    "title": "My Feature",
    "category": "TalkBank"
}
```

Add keybindings, menu entries, and `when` clauses as needed.

### In `src/activation/commands/<family>.ts`

Register the handler through the effect runtime:

```ts
import { Effect } from 'effect';
import { registerEffectCommand } from '../../effectCommandRuntime';

export function registerMyFeatureCommand(
    context: vscode.ExtensionContext,
    services: ExtensionRuntimeServices,
): void {
    registerEffectCommand(context, 'talkbank.myFeature', () =>
        Effect.gen(function* () {
            // ... typed-effect work, including LSP RPC calls
        }),
    );
}
```

Direct `vscode.commands.registerCommand` is reserved for the
Validation Explorer (see [ADR-002](../design/adr-002-effect-runtime.md)).
New commands default to the effect runtime — its typed error
algebra composes across RPC + webview + per-feature boundaries.

### Where command handlers live

| Registration path | Where | What goes here |
|---|---|---|
| `registerEffectCommand(...)` via `src/activation/commands/{analysis,editor,media,utility}.ts` | Effect runtime with typed errors | Default choice for new commands. |
| `vscode.commands.registerCommand(...)` via `src/activation/validation.ts` | Direct VS Code API | Validation Explorer only — the tree view's context-menu state does not fit the effect runtime. |
| `crates/chatter-lsp/src/backend/execute_commands.rs` + per-family handler | Server-side LSP RPC dispatch | The 12 `talkbank/*` commands invoked from TS via `workspace/executeCommand`. See [RPC Contracts](../reference/rpc-contracts.md). |

A registration sanity test asserts every
`contributes.commands` entry has a handler in one of the three
paths — adding a command to `package.json` without wiring it is a
test failure, not a silent runtime miss.

## Adding a custom LSP RPC command

For features that don't map to standard LSP capabilities, use
`workspace/executeCommand`.

1. Add a new variant to `ExecuteCommandName` and `ExecuteCommandRequest`
   in `crates/chatter-lsp/src/backend/execute_commands.rs`. Assign it
   to a feature family (`Documents`, `Analysis`, `Participants`,
   `ChatOps`).
2. Add the command-name string to `ExecuteCommandName::as_str` /
   `::parse` / `::ALL`. The unit test in the same file will pin the
   change against `advertised_commands()`.
3. Add the handler in the appropriate family module
   (`analysis.rs`, `participants.rs`, `chat_ops/`, etc.) returning
   `Result<serde_json::Value, LspBackendError>`.
4. Add dispatch to the family's `CommandService` in
   `backend/requests/execute_command.rs`.
5. On the TS side, add a typed method to
   `TalkbankExecuteCommandClient` in `src/lsp/executeCommandClient.ts`
   and the request/response schemas in
   `src/lsp/executeCommandPayloads.ts`.
6. Document the new endpoint in
   [reference/rpc-contracts.md](../reference/rpc-contracts.md) — per
   the workspace policy, RPC contracts are book-first.

## Related chapters

- [Architecture](architecture.md) — system design and module map
- [LSP Protocol](lsp-protocol.md) — standard capabilities advertised by the server
- [RPC Contracts](../reference/rpc-contracts.md) — per-endpoint reference
- [Testing](testing.md) — how to test new features
- [ADR-002: Effect-based command runtime](../design/adr-002-effect-runtime.md)
