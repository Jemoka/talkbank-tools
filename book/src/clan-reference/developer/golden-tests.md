# Golden Tests

**Status:** Current
**Last updated:** 2026-05-12 17:42 EDT

Golden tests under `crates/clan-core/tests/clan_golden/` (driven by the top-level `clan_golden.rs` entry point) and `crates/clan-core/tests/transform_golden.rs` compare `chatter clan` output against the legacy CLAN C binaries character-by-character.

## How they work

1. Run the legacy CLAN binary on a reference corpus fixture file
2. Run the equivalent `chatter clan` command on the same file
3. Snapshot both outputs with `insta` (suffixed `@clan` and `@rust`)
4. Any difference is a parity divergence that must be explained or fixed

The `@clan` snapshots capture CLAN's exact output and should never change (they represent the ground truth). The `@rust` snapshots capture our output and should match `@clan` as closely as possible.

## Requirements

Golden tests require CLAN binaries. The lookup is a single env var:

- `CLAN_BIN_DIR` — directory containing the CLAN command binaries (`check`, `freq`, `mlu`, …)

See `clan_bin_dir()` and `clan_command_available()` in `crates/clan-core/tests/common/mod.rs`. If `CLAN_BIN_DIR` is unset or the specific command binary is missing from that directory, the test prints a skip notice via `require_clan_command()` and returns early, making it CI-safe.

(Note: the legacy CLAN _library_ paths used by `database_integration.rs` follow a different resolver — `CLAN_SOURCE_DIR` env var → meta-repo sibling `OSX-CLAN/` → `~/OSX-CLAN/`. That is unrelated to the golden-test bin lookup.)

## Fixture files

Test fixtures come from the reference corpus at `resources/corpus/reference/` (at the repo root). Common fixtures include:

- `tiers/mor-gra.cha` — morphology and grammar tiers
- `ca/overlaps.cha` — CA overlap markers
- Various language-specific files for encoding edge cases

## Current parity

The current parity tally and the per-command divergence list live in
[Per-Command Divergences](../divergences/per-command.md) — that page is the single source of truth, so the numbers stay in one place instead of drifting between docs.

## Adding a golden test

The harness in `crates/clan-core/tests/clan_golden/harness.rs` provides two patterns:

- **Paired CLAN + Rust comparison** — declare a `ParityCase` and let the `parity_case_tests!` macro generate the test. CLAN side is auto-skipped when `clan_command_available()` reports the binary missing. Example: `clan_golden/check.rs:3` declares two paired CHECK cases.
- **Rust-only snapshot** (when no CLAN binary corresponds, or the comparison is a one-off) — call the command's typed `Command`/`run_xxx` API directly and snapshot the output. Example: `clan_golden/rust_only.rs` shows the MORTABLE, RELY, and SCRIPT patterns.

Helpers used by both patterns:

- `corpus_file("tiers/mor-gra.cha")` — resolve a path under `resources/corpus/reference/`
- `clan_command_available("freq")` / `require_clan_command("freq", "skip context")` — gate CLAN-side execution
- `run_clan("freq", &file, &["-t", "*CHI"])` — run a legacy CLAN binary, returning `Option<String>`
- `run_rust(...)` / `run_rust_filtered(...)` — run the chatter side with optional filter args

The snapshot naming convention is `<case>@clan` for the CLAN output and `<case>@rust` for the chatter output; both land under `tests/clan_golden/snapshots/`.

For a transform command, see `tests/transform_golden.rs` for the pattern — transforms write to temp files rather than stdout, and `run_rust_transform()` returns the contents of the rewritten file.

## Diagnosing parity breaks

When a golden test fails:

1. Run `cargo insta review` to see the diff
2. Check if the `@clan` snapshot changed (it shouldn't — that means the CLAN binary or fixture changed)
3. If only `@rust` changed, determine whether it's a bug fix or regression
4. If the divergence is intentional and justified, accept it and document in [Per-Command Divergences](../divergences/per-command.md)
