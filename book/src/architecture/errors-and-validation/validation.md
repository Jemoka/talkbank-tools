# Validation

**Status:** Current
**Last updated:** 2026-05-01 17:07 EDT

CHAT validation runs at multiple points in the processing pipeline.
All validation logic is in Rust (`crates/batchalign/src/validate.rs`
for the Batchalign side; `talkbank-model::validation` for CHAT-core
validation). This page covers validity levels, pre/post validation
gates, severity posture, the verification-gate set (G0–G14), and how
validation failures interact with caches and bug reports.

For error-code infrastructure (codes, sinks, severities, layers), see
[talkbank-tools-errors](talkbank-tools-errors.md). For the
diagnostic UX standard, see
[error-diagnostics-ux](error-diagnostics-ux.md).

## Validity Levels

The `ValidityLevel` enum defines three cumulative validation levels.
Each level includes all checks from lower levels.

| Level | Name | Checks |
|---|---|---|
| L0 | `Parseable` | No parse errors (clean tree-sitter CST) |
| L1 | `StructurallyComplete` | `@Participants` and `@Languages` present, all speaker codes declared, every utterance has a terminator |
| L2 | `MainTierValid` | Well-formed words, valid timing bullets if present |

### Pre-validation gates

Each command requires input to meet a minimum level before
processing:

| Command | Required level |
|---|---|
| `morphotag` | `MainTierValid` |
| `utseg` | `StructurallyComplete` |
| `translate` | `StructurallyComplete` |
| `coref` | `StructurallyComplete` |
| `align` | `Parseable` (lenient — must handle messy real-world files) |

`validate_to_level()` checks the file against the required level and
returns all failures found. Invalid files are rejected early with
diagnostics, before any compute is spent on inference.

## Post-Serialization Validation

After an orchestrator injects results and serializes CHAT output, the
server runs `validate_output()`:

1. **Alignment validation** — checks that `%mor`/`%gra`/`%wor` tier
   word counts match the main tier. ParseHealth-aware: utterances
   flagged as unparseable during lenient parsing are excluded.
2. **Semantic validation** — full CHAT validation:
   - **E362** — non-monotonic timestamps (utterance bullets must
     increase).
   - **E701** / **E704** — temporal constraints (overlap rules,
     same-speaker timing).
   - Header correctness — required headers present and well-formed.
   - Cross-utterance patterns — speaker code consistency.

Only blocks on `severity="error"`, not warnings.

## Severity Posture

Validation intentionally distinguishes errors from warnings:

- **Errors** block output. The server will not write CHAT with
  error-level validation failures.
- **Warnings** are reported but do not block. Legacy corpora contain
  widespread minor violations that must remain processable.

This distinction matters especially for `%gra`:

- Existing broken `%gra` in old corpora may be accepted with warnings
  so files remain processable.
- Newly generated `%gra` from batchalign3 is validated more strictly
  before writeback.

## Bug Reports and Cache Purges

When post-serialization validation fails:

1. A structured bug report is written to
   `~/.batchalign3/bug-reports/`.
2. Cache entries that produced the invalid output are purged
   (self-correcting cache).

This prevents broken results from being served on future runs.

## Verification Gates (`bazel build //... && bazel test //...`)

The mandatory pre-merge / pre-push gate. All gates must pass before
any code or doc change ships:

| Gate | What it checks |
|---|---|
| G0 | Parser signature guardrail |
| G1 | Rust workspace compile check |
| G2 | Spec tools compile check |
| G3 | Spec runtime tools compile check |
| G4 | CHAT manual anchor links |
| G5 | Generated parser corpus equivalence suite |
| G6 | Golden fragment validity (words + tiers) |
| G7 | Bare-timestamp regression gate |
| G8 | Reference corpus semantic equivalence |
| G9 | `%wor` tier parsing and alignment |
| G10 | Golden tier roundtrip (`%mor`, `%gra`, `%pho`, `%wor`) |
| G11 | Reference corpus node coverage |
| G12 | Generated artifacts match committed sources |
| G13 | Fuzz workspace isolation |
| G14 | Imported Batchalign Rust/PyO3 gate |

The reference corpus at `resources/corpus/reference/` is the sacred semantic
target — every file must be valid CHAT, and `bazel build //... && bazel test //...` runs each
gate against it.

The pre-push hook (`ln -sf ../../scripts/pre-push.sh .git/hooks/pre-push`) runs the fast subset (`fmt`,
affected compile, parser guardrail, `generated-check`, `fuzz-check`)
locally before push. CI runs the full set.

## Validation in the PyO3 Interface

The `ParsedChat` PyO3 handle exposes validation methods for the
Python API path:

| Method | Returns | Description |
|---|---|---|
| `validate()` | `list[str]` | Tier alignment errors (human-readable) |
| `validate_structured()` | JSON string | Tier alignment errors (structured) |
| `validate_chat_structured()` | JSON string | Full semantic validation |

These are used by the direct Python pipeline facade and tests. The
server path uses the same underlying validation functions directly
on the `ChatFile` AST.

## Known limitations

- **Validation rules are intentionally permissive on legacy data.**
  Some checks emit warnings rather than errors so legacy corpora
  remain processable while still surfacing the issue. Examples: pre-existing
  malformed `%gra` (warned, not blocked, so files that already shipped
  with bad `%gra` round-trip cleanly); some bullet-format minor
  variants. Newly generated tiers from batchalign are validated more
  strictly before writeback.
- **`%wor` word counts are not validated against the main tier.** `%wor`
  is a timing-annotation tier with no downstream positional indexing;
  legacy files may have `xxx`, fragments, or nonwords in `%wor`
  without producing alignment errors.
- **Cross-utterance quotation validation is gated off by default**
  (`enable_quotation_validation` flag) — the cross-utterance walker
  exists but is not yet wired into the standard validation gate.
- **Some error-spec / validator pairs are not yet implemented.**
  Tracked in `resources/spec/errors/` files marked `Status: not_implemented`;
  these generate `#[ignore]` tests via `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` rather than
  failing CI. Run `grep -rl "Status.*not_implemented" resources/spec/errors/`
  to enumerate.
