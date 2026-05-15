# Coding Standards

**Status:** Current
**Last updated:** 2026-05-12 17:36 EDT

## Universal Rust Standards

These apply to all Rust code across the TalkBank workspace.

### Edition and Tooling
- Rust **2024 edition**
- `cargo fmt` before committing
- Prefer `cargo nextest run` for faster test execution
- `cargo clippy --all-targets -- -D warnings` periodically

### Error Handling
- No panics for recoverable conditions — use typed errors (`thiserror`)
- No silent swallowing — no `.ok()`, `.unwrap_or_default()`, or silent fallbacks

### Output and Logging
- Library code: `tracing` macros only — never `println!`/`eprintln!`
- CLI binaries: `println!`/`eprintln!` for user-facing output
- Test code: `println!` is acceptable

### Type Design
- **No boolean blindness.** Enums over bools for anything beyond simple on/off. Banned: 2+ bool params, 2+ related bool fields, opposite bool pairs, ambiguous bool returns
- `BTreeMap` for deterministic JSON in tests
- Prefer explicit enums over ambiguous `Option`

### File Size Limits
- Recommended: 400 lines or fewer
- Hard limit: 800 lines (must be split)

## CLAN-Specific Standards

### Typed Data, Not String Matching

Use the AST (`word.category`, `word.untranscribed()`) instead of string-prefix checks (`starts_with('&')`, `== "xxx"`). This is the fundamental reason for the Rust port.

### No Panics

Handle missing data gracefully. No `%mor` tier? Skip morpheme counting. Use `tracing::warn!` for recoverable file-level issues.

### Typed Results

Every command defines its own result struct implementing `CommandOutput`. Avoid the generic `AnalysisResult` container.

### Stateless Commands

All mutable state goes in the `State` type. Commands hold only config.

### Use Framework Utilities

- `countable_words()` for word iteration
- `NormalizedWord` for frequency maps
- Never check filters in command code — the runner handles them

### Library Crate

No `println!`/`eprintln!` — use `tracing`.

## Documentation Standards

### Use CLAN.html for legacy intent

When a command is documented in the
[CLAN manual](https://talkbank.org/0info/manuals/CLAN.html), command docs
should use that manual to explain the command's intended semantics, examples,
and tier assumptions. Do not infer intent from accidental legacy string
behavior if the manual states the intended behavior more clearly.

### Say when the manual is absent or incomplete

Some implemented commands do not have a standalone section in the
[CLAN manual](https://talkbank.org/0info/manuals/CLAN.html), or are only
mentioned indirectly. In those cases, say so explicitly in the command chapter
and document what other evidence is being used instead.

### Separate CLI docs from GUI docs

Carry over non-GUI command semantics into the `clan-core` book. GUI workflows from the legacy manual should be documented in the TalkBank VS Code extension docs instead of being mixed into the CLI command reference.
