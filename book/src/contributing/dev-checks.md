# Developer Verification Checks

**Status:** Current
**Last updated:** 2026-04-29 10:39 EDT

This page defines the canonical local verification gates that must pass before opening or merging a PR.

## Canonical Command
Run:

```bash
bazel build //... && bazel test //...
```

This runs 15 gates (G0–G14). See [Testing > Verification Gates](testing.md#verification-gates) for the full table. Key gates include:

- **G0** Parser signature guardrail
- **G1** Rust workspace compile check
- **G2** Spec tools compile check
- **G5** Generated parser corpus equivalence suite
- **G6** Golden fragment validity (words + tiers)
- **G8** Reference corpus semantic equivalence
- **G10** Golden tier roundtrip (%mor, %gra, %pho, %wor)
- **G12** Generated artifacts match committed sources
- **G13** Fuzz workspace isolation
- **G14** Imported Batchalign Rust/PyO3 gate

## When to Run
- Always before creating a PR.
- Always before merging parser, spec-tool, or generated artifact changes.
- After rebasing if upstream changed parser/spec-tooling.

## Additional Engineering Checks

Run these in addition to `bazel build //... && bazel test //...` when touching parser/model code:

1. `cargo fmt` from repo root (use `cargo fmt`, not direct `rustfmt`).
2. `cargo test -p talkbank-parser --test test_parse_health_recovery`.
3. `cargo nextest run -p talkbank-parser-tests --test parser_equivalence_files`.

These protect against regressions in:
- parser recovery without sentinel fabrication
- parse-health taint propagation
- parser semantic equivalence

## Failure Policy
- If any gate fails, do not merge.
- Fix the failing gate or scope down the change.
- If the failure is unrelated and pre-existing, document it in the PR and open a blocker issue.

## Recommended Fast Loop During Development
Use narrower loops while iterating, then run `bazel build //... && bazel test //...` before final review:

```bash
cargo test -p talkbank-parser --lib
```

For grammar-only edits, prefer the smallest relevant loop first:

```bash
tree-sitter test
cargo test -p talkbank-parser
```

Only reach for `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` when the change truly affects generated
artifacts.

For dependency-aware local sweeps, the canonical entrypoint is now the Rust
xtask:

```bash
cargo run -q -p xtask -- affected-rust check
cargo run -q -p xtask -- affected-rust test
```

`bazel build //...` and `bazel test //...` both delegate to the same xtask
implementation.
