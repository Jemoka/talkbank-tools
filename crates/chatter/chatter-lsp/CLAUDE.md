# `chatter-lsp` — Language Server

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

Guidance for Claude Code when working inside `crates/chatter/chatter-lsp/`. Read the
workspace-level `talkbank-tools/CLAUDE.md` first — this file layers **LSP-specific
rules** on top of the cross-cutting design rules.

## What this crate is

A tower-lsp / tokio-based Language Server Protocol implementation for CHAT
files, driven over stdio by the VS Code extension (`talkbank-tools/apps/vscode-extension/`)
and by any other LSP client. The crate is a **thin protocol adapter** over
`talkbank-parser`, `talkbank-model`, `talkbank-transform`, and
`clan-core`. It owns:

- LSP request routing (`backend/`)
- Incremental document state (`backend/documents.rs`)
- Validation cache (`backend/validation_cache.rs`)
- Per-feature handlers (`backend/features/`)
- Hover / alignment presentation (`alignment/tier_hover/`, `alignment/formatters/`)
- `%gra` dependency-graph DOT rendering (`graph/`)
- Semantic-token generation (`semantic_tokens.rs`)
- Custom `talkbank/*` RPC endpoints for the VS Code extension
  (`backend/features/execute_commands.rs`, `backend/analysis.rs`,
  `backend/participants.rs`, `backend/chat_ops/`)

What it **must not** own: parsing rules, validation logic, alignment
algorithms, CHAT serialization, CLAN command semantics, or any domain state
that survives the current request. All of that lives in the shared crates.

## The hard rule: alignment is in `talkbank-model`

Every cross-tier alignment computation — main ↔ `%mor`, `%mor` ↔ `%gra`,
main ↔ `%pho`, main ↔ `%sin`, main ↔ `%wor` — is implemented **once**, in
`talkbank-model`'s `src/alignment/` tree. This crate consumes that output;
it never recomputes it.

**Concrete guidance when touching any `alignment/`, `tier_hover/`,
`highlights/`, or `graph/` code in this crate:**

- **Projecting a `%gra` relation index** (1-indexed `relation.index` /
  `relation.head`) to a `%mor` chunk → call `MorTier::chunk_at(n - 1)`.
  The returned `MorChunk<'_>` exposes `.kind()`, `.word()`, `.lemma()`,
  `.host_item()`, `.terminator_text()`.
- **Iterating the `%mor` chunk sequence** for any purpose (rendering,
  labeling, counting, matching against `%gra.relations`) → call
  `MorTier::chunks()`. Do **not** hand-roll `for item in mor.items { … for
  clitic in &item.post_clitics { … } }` — that is the exact shape of the
  bug we fixed.
- **Classifying a chunk** for diagnostic or display text → match on
  `MorChunkKind { Main | PostClitic | Terminator }`. Do not inspect
  serialized text (`~`, punctuation) to tell the kinds apart.
- **Going from a `%mor` item to its alignable main-tier word** → read
  `Utterance.alignments.as_ref().and_then(|a| a.mor.as_ref())` and look up
  the pair; do not re-derive the main-tier alignment from counts.
- **Going from a `%gra` relation to its `%mor` chunk** → use
  `gra_alignment.pairs` directly. The pair's `mor_chunk_index` is the
  authoritative 0-indexed chunk position. Only use `gra_relation.index`
  when you specifically want the author's written value (e.g. to flag a
  typo that the validator would already catch with E712).
- **`%pho`, `%sin` alignment** → same principle: use the `AlignmentSet`
  on `Utterance`, not ad hoc counting.
- **`%wor` is not an alignment** — it is a timing sidecar. Read
  `AlignmentSet::wor_timings: Option<WorTimingSidecar>` and call
  `.is_positional()` / `.positional_count()` before any positional zip;
  treat `Drifted { .. }` as "no timing recovery available," not as an
  error. Do not reach for `%mor`/`%pho`-style alignment helpers here.
  See KIB-016.

If you find yourself needing a chunk primitive the model doesn't expose,
add it in `crates/talkbank-model/src/model/dependent_tier/mor/chunk.rs`
(or the analogous location for the tier in question) and delegate from
here. Do not grow a second walker in this crate.

## Three distinct index spaces (do not conflate)

`%mor` and `%gra` involve three index spaces whose confusion is the
single most common source of alignment bugs. Name them explicitly in
variable names, even when their types are still raw `usize`:

| Space | 0- or 1-indexed | Over what | Example source |
|-------|-----------------|-----------|----------------|
| Mor **item** index | 0-indexed | `MorTier::items` | `AlignmentPair.target` (main↔mor) |
| Mor **chunk** index | 0-indexed | `MorTier::chunks()` sequence (item mains + post-clitics + terminator) | `GraAlignmentPair.mor_chunk_index` |
| Semantic **word** index | **1-indexed** | Author-written position in a `%gra` relation | `GrammaticalRelation::index`, `GrammaticalRelation::head` |

Rules of thumb:

- Convert 1-indexed semantic positions to 0-indexed chunk indices with
  `word_index.checked_sub(1)` — never `word_index - 1` (which panics at 0
  and silently "works" at higher values).
- `relation.head == 0` is not a chunk — it is ROOT. Guard before indexing.
- If you need to go from a chunk index to a `%mor` *item* (e.g. to reach
  through the main↔mor alignment), use `chunk.host_item()` — do not do
  index arithmetic.
- Until typed indices catch the confusion at compile time, be
  deliberate when handling these three spaces.

## Other LSP-specific rules

- **No panics in request handlers.** Per the workspace rule, plus the
  LSP-specific reason: a panic tears down the server and the client
  usually respawns with stale state. Always return typed errors or empty
  responses with tracing warnings.
- **Degrade gracefully on parse failure.** `ParseHealth` propagates from
  parser into model; if a document is tainted, feature handlers must
  either return empty results or scope their work to the pre-taint
  region. Do not surface validation errors for downstream-tier constructs
  when the main tier itself failed to parse.
- **Incremental reparse is the hot path.** Tree-sitter does the
  incremental work; the LSP must pass the right edit deltas. When adding
  a feature that touches `DocumentState`, verify that the feature works
  after an edit (not just on open).
- **Custom RPC endpoints must be typed.** Every `talkbank/*` command has
  a request and response shape. Use `schemars::JsonSchema` + `serde` on
  the payloads; do not accept `serde_json::Value` into feature code.
- **Don't fabricate alignment when it's missing.** If
  `utterance.alignments.as_ref()` is `None`, return a graceful fallback
  (e.g. "alignment not computed" hover, empty highlights), not a synthetic
  1:1 mapping.
- **Tests go per-feature and must exercise clitic (`~`) cases.**
  Post-clitic handling is the class of bug we keep finding. Every feature
  that touches `%mor` or `%gra` needs at least one test using a fixture
  like `pron|it~aux|be` or `pron|I~aux|will`.

## Where things live

```
crates/chatter/chatter-lsp/
├── src/
│   ├── bin/            # tower-lsp binary entry point (stdio transport)
│   ├── lib.rs          # module tree exported for tests
│   ├── backend/
│   │   ├── mod.rs              # LanguageServer impl (tower-lsp trait)
│   │   ├── capabilities.rs     # ServerCapabilities advertisement
│   │   ├── documents.rs        # DocumentState, incremental text sync
│   │   ├── validation_cache.rs # grouped-by-scope error cache
│   │   ├── requests.rs         # request routing dispatch
│   │   ├── analysis.rs         # `talkbank/analyze` RPC (→ clan-core)
│   │   ├── participants.rs     # `talkbank/getParticipants`, formatIdLine
│   │   ├── chat_ops/           # filterDocument, getSpeakers, scopedFind, getUtterances
│   │   └── features/           # LSP feature handlers + custom execute_commands
│   ├── alignment/
│   │   ├── tier_hover/         # per-tier hover resolvers
│   │   ├── formatters/         # display formatters (mor, content, …)
│   │   ├── finders.rs          # AlignableContent lookup helpers
│   │   └── types.rs            # AlignmentHoverInfo presentation shape
│   ├── graph/                  # %gra DOT rendering
│   ├── semantic_tokens.rs      # semantic token legend + full/range handlers
│   └── utils/                  # workspace-internal helpers
├── tests/                      # integration tests (rare; most tests are in-module)
└── CLAUDE.md                   # this file
```

## Build, test, run

```bash
# Build (debug)
cargo build -p chatter-lsp

# Build (release — what VS Code loads when unbundled)
cargo build --release -p chatter-lsp

# Run all crate tests via nextest (preferred)
cargo nextest run -p chatter-lsp

# A focused test
cargo nextest run -p chatter-lsp -E 'test(gra_word_label_with_post_clitic)'

# Regression gates after any alignment-touching change (mandatory):
cargo nextest run -p talkbank-model
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
```

## Related documentation

- Public user + developer docs for the VS Code extension:
  `talkbank-tools/book/src/vscode/` (the VS Code section of the
  unified TalkBank Toolchain mdBook).
- Unified book, chatter architecture chapter:
  `talkbank-tools/book/src/architecture/alignment.md`.
- VS Code extension guidance: `talkbank-tools/apps/vscode-extension/CLAUDE.md`.
- Custom RPC contract reference (planned): `book/src/vscode/reference/`.
