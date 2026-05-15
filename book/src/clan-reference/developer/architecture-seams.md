# Current Architecture Seams

**Status:** Current
**Last updated:** 2026-05-12 17:35 EDT

This page documents the current internal seams that contributors should preserve when adding or restructuring CLAN-related functionality.

## CLI command registration

Top-level CLI argument wiring is no longer in one file.

- shared CLI args live in `crates/chatter-cli/src/cli/args/core.rs`
- shared CLAN filters and common options live in `crates/chatter-cli/src/cli/args/clan_common.rs`
- CLAN command variants live in `crates/chatter-cli/src/cli/args/clan_commands.rs`

If you add a new CLAN command, register it in the appropriate split argument module instead of rebuilding a monolithic `args.rs`.

## CLAN dispatch

`run_clan` now lives in `crates/chatter-cli/src/commands/clan/mod.rs` and dispatches into category files:

- `analysis.rs`
- `transforms.rs`
- `converters.rs`
- `compatibility.rs`
- `helpers.rs` (shared utilities consumed by the others)

Keep family-specific logic in those modules. Shared file resolution, filtering, and output helpers belong in `helpers.rs`, not copied into each family.

## Validation output

Parallel validation output now has a renderer seam:

- orchestration and stats live in `crates/chatter-cli/src/commands/validate_parallel/runtime.rs`
- output shaping lives in `crates/chatter-cli/src/commands/validate_parallel/renderer.rs`
- audit-specific behavior lives in `crates/chatter-cli/src/commands/validate_parallel/audit.rs`

If you need a new output mode, add a renderer implementation instead of extending a large runtime `match`.

Audit-mode JSONL writing is also intentionally isolated. `crates/chatter-cli/src/commands/validate/audit_reporter.rs` owns a dedicated writer thread and a cloneable reporting handle for workers, so future audit changes should preserve that explicit ownership boundary instead of reintroducing shared writer locks.

## Dashboard state ownership

`test-dashboard` (under `src/test_dashboard/` in the root `talkbank-tools` crate) now uses message passing rather than shared UI state. The worker sends `DashboardEvent` values and the UI reduces them into `AppState`.

That architecture is easier to test and reason about than `Arc<Mutex<AppState>>`. New dashboard features should generally be introduced as:

1. a new variant of `DashboardEvent` in `src/test_dashboard/app.rs`
2. reducer logic in `AppState::apply_event` in the same file
3. worker-side emission in `src/test_dashboard/runner.rs`

## Editor integration note

The VS Code extension and `chatter-lsp` use a typed execute-command boundary. The contract surface is documented in:

- `book/src/apps/vscode-extension/reference/rpc-contracts.md` — the RPC contracts the extension and LSP speak
- `book/src/apps/vscode-extension/developer/custom-commands.md` — how to add a new custom command
- `crates/chatter-lsp/CLAUDE.md` — invariants for the server side

