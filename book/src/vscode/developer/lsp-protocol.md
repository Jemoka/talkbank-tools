# LSP Protocol

**Status:** Current
**Last updated:** 2026-04-16 22:12 EDT

This chapter documents all Language Server Protocol capabilities
advertised by `chatter-lsp`. The capability set is declared in
`crates/chatter-lsp/src/backend/capabilities.rs` via
`build_initialize_result()`. Per-feature handlers live under
`backend/features/` (request-agnostic feature code) and
`backend/requests/` (request-dispatcher shims that pull out the
parts of `Backend` each handler needs).

The twelve custom `talkbank/*` commands (also advertised through this
mechanism) have their own per-endpoint reference at
[RPC Contracts](../reference/rpc-contracts.md); this chapter stays
focused on the standard LSP surface.

## Advertised Capabilities

The server advertises 23 standard LSP capabilities plus 12 custom commands.

### Text Document Sync

| Capability | Mode | Description |
|-----------|------|-------------|
| Text document sync | Incremental | Receives incremental edits (not full document re-sends) |

### Text Document Features

| Capability | Handler | Description |
|-----------|---------|-------------|
| Hover | `features/hover.rs` | Cross-tier alignment display for main, %mor, %gra, %pho, %sin tiers. Also covers headers and timing bullets. |
| Completion | `features/completion.rs` | Speaker codes from @Participants, tier prefixes, postcode suffixes, header names, bracket annotations. Triggers on `*`, `%`, `+`, `@`, `[`. |
| Definition | `requests/goto_definition.rs` | Navigate from speaker code to @Participants declaration, from dependent tier to aligned main tier word (chunks → host items via `MorTier::item_index_of_chunk`). |
| References | `features/references.rs` | Find all occurrences of a speaker code across @Participants, @ID headers, and main tier lines. |
| Document Highlight | `features/highlights/` | Bidirectional cross-tier alignment highlighting. Click a word on any tier to highlight aligned items on all other tiers. |
| Document Symbol | `features/document_symbol.rs` | Two-level outline: transcript as Module, per-utterance String symbols labeled by speaker. Powers Cmd+Shift+O and the Outline view. |
| Code Action | `features/code_action.rs` | Quick fixes for 21 error codes (E241, E242, E244, E258, E259, E301, E305, E306, E308, E312, E313, E322, E323, E362, E501-E504, E506, E507, E604). |
| Formatting | `requests/formatting.rs` | Re-serializes the document through the canonical CHAT serializer. |
| On-Type Formatting | `features/on_type_formatting.rs` | Auto-inserts leading tab on continuation lines after tier prefix. Trigger character: `:`. |
| Rename | `features/rename.rs` | Rename speaker code across @Participants, @ID headers, and all main tier lines. Supports prepare rename. |
| Code Lens | `features/code_lens.rs` | Utterance counts per speaker above @Participants (e.g., "CHI: 42 utterances"). |
| Semantic Tokens | `semantic_tokens.rs` | Full and range-based semantic tokens. 11 token types (keyword, variable, string, comment, type, operator, number, function, tag, punctuation, error). |
| Diagnostic | `diagnostics/` | Push (`publishDiagnostics`) and pull (`textDocument/diagnostic`, `workspace/diagnostic`) models. |
| Selection Range | `features/selection_range.rs` | Smart expand/shrink: word to utterance content to tier block to transcript. |
| Linked Editing Range | `features/linked_editing.rs` | Simultaneous editing of matching speaker codes. |
| Document Link | `features/document_link.rs` | Makes @Media: header values clickable links. |
| Folding Range | `features/folding_range.rs` | Fold utterance blocks (main tier + dependent tiers) and the header block. |
| Inlay Hint | `features/inlay_hints.rs` | Alignment count mismatch hints (e.g., `[alignment: 3 main <> 2 mor]`). |

### Workspace Features

| Capability | Handler | Description |
|-----------|---------|-------------|
| Workspace Symbol | `features/workspace_symbol.rs` | Cross-file search by speaker code and utterance content. |
| Did Change Configuration | `backend/mod.rs` | Responds to settings changes from the client. |

### Execute Command

The server advertises 12 custom commands via `workspace/executeCommand`
(the `talkbank/*` family). Per-endpoint reference:
[RPC Contracts](../reference/rpc-contracts.md). Architecture and
sequence-diagram: [Custom Commands](custom-commands.md).

## Error contract

Feature handlers inside `chatter-lsp` return
`Result<_, LspBackendError>` rather than stringly `Result<_, String>`.
`LspBackendError` (`backend/error.rs`) is a `thiserror` enum with
variants for tier/alignment state (`MissingTier { tier: TierName }`,
`TierAlignmentMissing { tier: TierName }`,
`AlignmentMetadataMissing`), document state (`DocumentNotFound`,
`JsonSerializeFailed(#[from] serde_json::Error)`), parse state
(`ParseFailure { count, first_message }`), executeCommand argument
decoding (`ArgumentMissing`, `ArgumentInvalid`, `InvalidUriParse`,
`UriNotFilePath`), regex (`InvalidRegex`), init failure
(`LanguageServicesUnavailable(#[from] BackendInitError)`), external
service failure (`ExternalServiceFailed`), and a wrapper for
graph-specific failures (`Graph(#[from] GraphEdgeError)`).

Handlers that return typed errors get collapsed to the `tower-lsp`
string form only at the protocol boundary (`requests/*.rs`). This
means every feature can match on its classification without
parsing free-form text.

## Completion Trigger Characters

The completion provider activates on these trigger characters:

| Character | What it completes |
|-----------|------------------|
| `*` | Speaker codes from @Participants |
| `%` | Dependent tier prefixes (mor, gra, pho, sin, etc.) |
| `+` | Postcode punctuation suffixes |
| `@` | Header names (@Participants, @ID, @Languages, etc.) |
| `[` | Bracket annotations |

## Semantic Token Types

The semantic tokens provider uses 11 token types, mapped to VS Code's theme colors:

| Index | Type | Used For |
|-------|------|----------|
| 0 | keyword | Headers (@Begin, @End), tier prefixes |
| 1 | variable | Speaker codes (*CHI, *MOT) |
| 2 | string | Quoted strings, word content |
| 3 | comment | Comment lines |
| 4 | type | Type annotations, complex structures |
| 5 | operator | Postcodes, morphological separators |
| 6 | number | Timing values, indices |
| 7 | function | Dependent tier prefixes, special markers |
| 8 | tag | Tier labels, annotation markers |
| 9 | punctuation | Terminators (. ? !), special punctuation |
| 10 | error | Syntax errors, malformed tokens |

Semantic tokens are computed on every request (full or range-based). The TextMate grammar (`chat.tmLanguage.json`) provides fallback highlighting before the LSP finishes starting.

## Diagnostic Tags

The server uses LSP diagnostic tags for visual de-emphasis:

| Tag | Used For |
|-----|----------|
| `Unnecessary` | Empty utterances, empty colons -- shown with fade-out styling |
| `Deprecated` | Deprecated CHAT constructs |

## Related Chapters

- [Architecture](architecture.md) -- overall system design
- [Custom Commands](custom-commands.md) -- the 12 non-standard commands
- [Adding Features](adding-features.md) -- how to add a new capability
