# CHAT Dependent Tier Handling by Command

**Status:** Current
Each command reads and writes different dependent tiers (`%mor`, `%gra`, `%wor`, `%xtra`).
This determines which parse mode is used at pipeline entry.

## Parse modes

- **Strict** (`ParsedChat.parse()`): Rejects the file on ANY parse error. Used when the
  input is expected to be valid — i.e., output from a previous pipeline stage.
- **Lenient** (`ParsedChat.parse_lenient()`): Error recovery — keeps parseable content and
  drops broken tiers. Used at pipeline entry because input may have malformed dependent
  tiers from legacy CLAN runs or previous batchalign versions.

## Where each parse mode is used

| Location | Mode | Why |
|----------|------|-----|
| Rust server (per-file dispatch) | Lenient | Input CHAT may have broken dep tiers |
| Rust server (post-injection re-parse) | Strict | Engine output should be valid; catch bugs early |
| Rust server (comment insertion) | Strict | Pipeline output should be valid |

## Per-command tier handling

| Command | Reads Dep Tiers | Writes Dep Tiers | Notes |
|---------|----------------|-----------------|-------|
| morphotag | None (clears %mor/%gra first) | %mor, %gra | `clear_morphosyntax()` strips existing tiers before processing |
| align | %wor (for UTR) | %wor | Regenerates timing from scratch |
| translate | None | %xtra | Adds translation tier |
| utseg | None | (restructures utterances) | Splits/merges utterance boundaries |
| transcribe | N/A (generates from audio) | All | Creates fresh CHAT |

## Why morphotag clears before processing

The Rust `collect_morphosyntax_payloads()` function skips utterances that already
have a `%mor` tier (optimization for cache injection). Without clearing first, files
with existing `%mor` would be silently round-tripped unchanged. Calling
`handle.clear_morphosyntax()` at the top of `process_morphosyntax()` ensures
all utterances are reprocessed. Cache hits still work — the cache lookup happens
*after* clearing but *before* Stanza runs.
