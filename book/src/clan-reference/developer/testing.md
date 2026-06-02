# Testing Strategy

**Status:** Current
**Last updated:** 2026-05-12 21:14 EDT

## Unit tests

Each command module includes `#[cfg(test)]` unit tests that verify counting logic, edge cases, and output formatting against known inputs.

```bash
cargo nextest run -p clan-core          # All tests (preferred)
cargo test -p clan-core                 # Alternative
cargo test --doc -p clan-core           # Doctests only (nextest can't run these)
```

## Snapshot tests

Snapshot tests use `insta` to capture command output and detect regressions. They live alongside the test that produces them:

- `crates/clan/clan-core/tests/snapshots/` — snapshots from `transform_golden.rs`, `converter_golden.rs`, and other top-level tests in the `tests/` directory
- `crates/clan/clan-core/tests/clan_golden/snapshots/` — snapshots produced by the `clan_golden` test module (paired `@clan`/`@rust` pairs)

```bash
cargo insta review -p clan-core         # Review pending snapshot changes
cargo insta accept --all                    # Accept all pending changes
```

When you change a command's output format, the snapshot test will fail. Review the diff to confirm the change is intentional, then accept.

## Golden tests

Golden tests compare our output against legacy CLAN C binaries. See [Golden Tests](golden-tests.md) for details.

## Running specific tests

```bash
# By name pattern
cargo nextest run -p clan-core -E 'test(freq)'

# Only golden tests
cargo nextest run -p clan-core -E 'test(golden)'

# Only indent tests
cargo nextest run -p clan-core -E 'test(indent)'
```

## Test fixtures

Test fixtures are real CHAT files from the reference corpus at `resources/corpus/reference/` (at the repo root). Never create ad hoc `.cha` test files — use existing corpus files or ask for new ones to be added to the reference corpus.

## Current test counts

The `clan-core` suite spans unit, snapshot, golden, and integration
tests. The absolute count moves as commands are added or extended;
get the live number with:

```bash
cargo nextest list -p clan-core
```

The last line of the summary is the total count.
