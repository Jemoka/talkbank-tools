# chatter-lsp — Debugging & Manual Testing

## Enabling Debug Logs

The LSP uses `tracing` for structured logging. Set the environment variable before
launching the LSP server (or VS Code with the extension):

```bash
RUST_LOG=chatter_lsp=debug
```

All validation-path log messages include a `path` field you can filter on:

| `path` value | Meaning |
|---|---|
| `incremental` | Same utterance count, only affected utterances re-validated |
| `incremental-rebuild` | Cache miss after incremental parse — full cache rebuilt |
| `incremental-error` | Parse errors in updated utterances — old baseline kept |
| `splice-insert` | New utterance detected (count +1), spliced into ChatFile |
| `splice-delete` | Utterance removed (count −1), removed from ChatFile |
| `splice` | Cache splice completed (or splice parse error fallback) |
| `fallback-full` | Full reparse + validate (no usable baseline) |

## Manual Testing — Incremental Validation

After making changes to the validation orchestrator, incremental parsing, or
cache logic, verify the following scenarios in VS Code with a `.cha` file open.

### Prerequisites

1. Build the LSP: `cargo build -p chatter-lsp`
2. Ensure the VS Code extension points to the local binary
3. Open a valid `.cha` file (e.g., from `resources/corpus/reference/`)
4. Open the Output panel → "TalkBank Language Server" channel (or check
   `RUST_LOG` output wherever the server logs)

### Test 1: Edit within an utterance (incremental path)

1. Find an utterance like `*CHI: hello world .`
2. Change `hello` to `goodbye`
3. **Expected**: Diagnostics update for that utterance only. Logs show
   `path="incremental"` with `affected=1`.
4. Other utterances' diagnostics should NOT flash/flicker (they weren't
   re-validated).

### Test 2: Introduce and fix a syntax error

1. In an utterance, delete the terminator (the final `.`)
2. **Expected**: Red squiggle appears on that utterance. Logs show either
   `path="incremental"` or `path="incremental-error"`.
3. Re-type the `.` to fix the error
4. **Expected**: Squiggle clears. The fix should go through the incremental
   path (not `fallback-full`).

### Test 3: Insert an utterance (splice-insert path)

1. Position cursor after an existing utterance line
2. Type a new utterance: `*CHI:\tnew line .` and press Enter
3. **Expected**: Logs show `path="splice-insert"`. Only the new utterance is
   validated. Existing diagnostics on other utterances remain unchanged.

### Test 4: Delete an utterance (splice-delete path)

1. Select an entire utterance line (including its dependent tiers)
2. Delete it
3. **Expected**: Logs show `path="splice-delete"`. The deleted utterance's
   diagnostics disappear. Other utterances' diagnostics remain.

### Test 5: Features work during parse errors

1. Introduce a syntax error in one utterance (e.g., delete `@End`)
2. Try hovering over a different, valid utterance
3. **Expected**: Hover info still works (ChatFile is preserved even when
   parse errors exist).
4. Try completion on a participant code (`*` then trigger completion)
5. **Expected**: Completion still works.

### Test 6: Decorative header edit (no full rebuild)

1. Edit a decorative header: `@Comment:`, `@Date:`, `@Location:`, etc.
2. **Expected**: Only header errors are re-checked. Logs should show
   `path="incremental-rebuild"` with a "reusing header validation" message,
   NOT a full validation context rebuild.

### Test 7: Context-affecting header edit (full rebuild)

1. Edit `@Participants:` (e.g., add a new participant code)
2. **Expected**: Full validation context rebuild triggers. Logs may show
   `path="fallback-full"` or `path="incremental-rebuild"` without the
   "reusing header validation" message.

### Test 8: Large edit falls back to full parse

1. Paste a large block of text (multiple utterances at once)
2. **Expected**: `path="fallback-full"` in logs. The splice path only
   handles ±1 utterance count changes; larger structural changes use the
   full fallback.

## Automated Tests

```bash
# All LSP tests (60 tests)
cargo nextest run -p chatter-lsp

# Splice detection tests only
cargo nextest run -p chatter-lsp -- detect_splice

# Clippy
cargo clippy -p chatter-lsp --all-targets -- -D warnings
```

## See Also

- [CLAUDE.md](CLAUDE.md) — LSP coding conventions and reliability rules
- [ARCHITECTURE.md](ARCHITECTURE.md) — Module structure
- [src/backend/diagnostics/validation_orchestrator.rs](src/backend/diagnostics/validation_orchestrator.rs) — Core orchestration logic
- [src/backend/incremental.rs](src/backend/incremental.rs) — Splice detection, utterance collection
- [src/backend/validation_cache.rs](src/backend/validation_cache.rs) — Cache structure and splice helpers

---
Last Updated: 2026-02-24
