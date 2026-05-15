# CLAUDE.md

**Last modified:** 2026-05-01 09:47 EDT

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A CHAT transcript parser using **re2rust** (re2c's Rust backend) for lexing and **chumsky** parser combinators for parsing. Lives in the `talkbank-tools` workspace as `talkbank-parser-re2c`.

**Status:** Implements the `ChatParser` trait from `talkbank-model`. The public type is `Re2cParser`. Lexer validated against the wild CHAT corpus with zero errors. Integrated into the CLI via `chatter validate --parser re2c`. Substantially faster than TreeSitterParser on the reference corpus.

## Architecture

```
re2c DFA Lexer  -->  Chumsky Combinators  -->  AST  -->  talkbank-model
  (lexer.re)          (parser/*.rs)       (ast.rs)     (convert.rs)
```

**Two-stage pipeline:**
1. **Lexer** (`lexer.re`) — re2c DFA produces rich tokens with tagged field extraction.
2. **Parser** (`parser/`) — chumsky combinators consume `&[Token]` and produce AST types.
3. **Conversion** (`convert.rs`) — `From` impls map AST to talkbank-model. Source-free (AST is self-contained via `raw_text` fields).

### Parser Module Structure

```
src/parser/
  mod.rs              Module declarations, lex_to_tokens helper
  main_tier.rs        Chumsky: contents, words, groups, tier_body, main_tier
  dependent_tiers.rs  Chumsky: %mor, %gra, %pho, %sin, %wor, text tiers
  classify.rs         Token classification: is_terminator, is_annotation, etc.
  word_body.rs        Char-level word body scanner (not chumsky)
  file.rs             Imperative file-level parser with error reporting
  entry_points.rs     Public API: parse_chat_file, parse_main_tier, etc.
  headers.rs          Chumsky: @ID, @Languages, @Participants
```

### Key Design Decisions

**Chumsky** (pinned in `Cargo.toml`). Token-stream input via `&[Token<'a>]`. The `select!` macro matches token variants by value. `recursive()` handles nested groups/quotations.

**Leaked allocations.** `lex_to_tokens()` NUL-pads the input, leaks it, lexes to `Vec<Token>`, leaks that too. This gives chumsky a `&'a [Token<'a>]` with a stable lifetime. Acceptable for a testing/validation tool.

**Imperative file parser.** The file-level parser (`file.rs`) uses an imperative loop rather than chumsky because dependent tier dispatch is prefix-text-based (`%mor:` vs `%gra:` etc.), which doesn't map to chumsky's token-variant matching.

**CA terminator promotion.** CA intonation arrows (⇗ ↗ → ↘ ⇘) serve dual roles: mid-content separators and utterance-final terminators. Chumsky always parses them as separators. `convert.rs` promotes trailing arrows to terminators at the AST-to-model boundary (same strategy as TreeSitterParser's `resolve_ca_terminator`).

**Subtoken word assembly.** When the lexer produces sub-tokens instead of a single rich `Token::Word` (edge cases where the `w_body` regex doesn't match), `subtoken_word()` in `main_tier.rs` assembles them. The `display_text()` helper reconstructs raw_text with structural delimiters (e.g., `Shortening("x")` -> `"(x)"`).

## Architecture: Rich Word Token

The lexer emits a **single `Token::Word`** for each complete word, carrying tagged field boundaries:

```rust
Token::Word {
    raw_text: &str,            // full word text from source
    prefix: Option<&str>,      // "&-", "&~", "&+", or "0"
    body: &str,                // word body (parser handles internals)
    form_marker: Option<&str>, // "f", "z:grm" (content only, no @)
    lang_suffix: Option<&str>, // "eng+zho" (no @s:), "" for bare @s
    pos_tag: Option<&str>,     // "n", "adj" (no $)
}
```

**Body parsing:** The body is too complex for fixed re2c tags (variable-length sequences of text segments, shortenings, compounds, CA markers, etc.). `parse_word_body(body: &str) -> Vec<WordBodyItem>` in `word_body.rs` scans the body string.

### Other Rich Tokens

| Token | Fields | Notes |
|-------|--------|-------|
| `IdFields` | pipe-delimited fields | @ID header, zero-copy |
| `TypesFields` | design, activity, group | @Types header |
| `MorWord` | pos, lemma_features | %mor word |
| `GraRelation` | index, head, relation | %gra relation |
| `MediaBullet` | start_time, end_time | Timestamp extraction |
| `OtherSpokenEvent` | speaker, text | &*SPK:word |

## Strict Adherence to grammar.js

The canonical grammar is `~/talkbank/talkbank-tools/grammar/grammar.js`. All lexer rules and parser logic must be **directly translated** from grammar.js — not invented, not approximated. When implementing a construct:

1. Find the exact rule in grammar.js
2. Translate it to re2c conditions/rules, leveraging re2c features
3. Verify with the matching spec in `resources/spec/constructs/` or `resources/spec/errors/`
4. **Leverage re2c to produce richer tokens** than grammar.js's flat token model can express

Key grammar.js design decisions:
- Terminators are **optional** (`optional($.terminator)` in `utterance_end`) — presence enforced by AST validation, not parsing
- Each tier type has its own content rules (`mor_contents`, `gra_contents`, `pho_groups`, `text_with_bullets`, etc.)

## Conditions (Start States)

re2c conditions are numbered states that change what rules are active:

- `INITIAL` — top-level line classification (@, *, %)
- `MAIN_CONTENT` — main tier body (words, annotations, terminators)
- `MOR_CONTENT`, `GRA_CONTENT`, `PHO_CONTENT`, `SIN_CONTENT` — tier-specific
- `ID_CONTENT`, `TYPES_CONTENT`, `LANGUAGES_CONTENT`, `PARTICIPANTS_CONTENT`, `MEDIA_CONTENT` — header-specific
- `HEADER_CONTENT`, `TIER_CONTENT` — generic structured headers/tiers

**Multiple entry points:** `Lexer::new(input, condition)` allows starting in any condition. This means we can lex a `%mor` tier body in isolation (start in `MOR_CONTENT`), a main tier content item (start in `MAIN_CONTENT`), etc.

**Continuation rule:** The lexer's continuation rule (`<*> [\r\n]+ [\t]`) must NOT reset the condition. Continuation content stays in the same lexer mode.

## Entry Points

| Entry point | Start condition | Input | Output |
|-------------|----------------|-------|--------|
| `parse_main_tier` | `INITIAL` | `*CHI:\thello .\n` | `MainTier` |
| `parse_chat_file` | `INITIAL` | full `.cha` file | `ChatFile` |
| `parse_word` | `MAIN_CONTENT` | `ice+cream@f` | `WordWithAnnotations` |
| `parse_mor_tier` | `MOR_CONTENT` | `pro\|I v\|want .\n` | `MorTier` |
| `parse_gra_tier` | `GRA_CONTENT` | `1\|2\|SUBJ 2\|0\|ROOT` | `GraTier` |
| `parse_pho_tier` | `PHO_CONTENT` | `wɑ+kɪŋ hɛloʊ .\n` | `PhoTier` |
| `parse_text_tier` | `TIER_CONTENT` | text with bullets | `TextTierParsed` |
| `parse_id_header` | `ID_CONTENT` | `eng\|corpus\|CHI\|...` | `IdHeaderParsed` |

## ChatParser Trait

`Re2cParser` implements `ChatParser` from `talkbank-model`, providing all parse methods:
- File-level: `parse_chat_file`
- Line-level: `parse_header`, `parse_utterance`, `parse_main_tier`
- Token-level: `parse_word`, `parse_mor_word`, `parse_gra_relation`
- Tier-level: `parse_mor_tier`, `parse_gra_tier`, `parse_pho_tier`, plus all text tiers

Conversion functions in `convert.rs` are source-free — all use `From` impls or take only AST types.

## Performance

Benchmarked with divan on reference corpus files. All content pre-loaded; zero I/O.
Most of the per-parse cost is in the chumsky combinators; re2c lexing is the smaller fraction.
TreeSitter constructor cost is negligible.

Run benchmarks: `cargo bench -p talkbank-parser-re2c --bench parse_comparison`

## Build & Test

```sh
cd ~/talkbank/talkbank-tools
cargo check -p talkbank-parser-re2c
cargo nextest run -p talkbank-parser-re2c     # prefer nextest for speed
cargo test -p talkbank-parser-re2c --jobs 1   # fallback
```

Requires `re2rust` (part of re2c) on PATH: `brew install re2c`.

The build script (`build.rs`) runs `re2rust` on `src/lexer.re` -> `OUT_DIR/lexer.rs`. Edit `lexer.re`, not generated output. Use `\x00` (not `\0`) for NUL — re2c treats `\0` as octal prefix.

## Testing

- **Lexer tests:** `tests/lexer_tests.rs` — unit tests per token type using start conditions.
- **Corpus lexer tests:** `tests/corpus_lex_tests.rs` — lex real lines from the wild data corpus.
- **Parser tests:** `tests/golden_parse.rs`, `tests/parser_fixtures.rs` — parsed AST structures.
- **Equivalence tests:** `tests/equivalence_tests.rs` — Re2cParser vs TreeSitterParser comparison via `ChatParser` trait.
- **Model study:** `tests/model_study.rs` — reference corpus equivalence (a small number of CA files have known raw_text divergences in the subtoken word path).
- **Full corpus tests:** `tests/full_corpus_parse_test.rs` — wild-corpus SemanticEq comparison.
- **Benchmarks:** `benches/parse_comparison.rs` — divan benchmarks comparing both parsers.
- **When a test fails, STOP and ask.** CHAT semantics are domain-specific.
- **Slow tests:** Mark with `#[ignore]` and run via `--ignored` flag.

### Running Corpus Tests

Corpus tests can take many minutes (release mode). They write reports to `/tmp/re2c_*.json`.

```bash
# Full parse comparison (SemanticEq on the wild corpus)
cargo test -p talkbank-parser-re2c --test full_corpus_parse_test --release -- --ignored --nocapture

# Categorize divergences (span-stripped JSON diff)
cargo test -p talkbank-parser-re2c --test categorize_divergences --release -- --ignored --nocapture

# Sub-categorize main tier divergences
cargo test -p talkbank-parser-re2c --test subcategorize_main_tier --release -- --ignored --nocapture
```

**Pitfalls:**
- Do NOT pipe corpus test output through grep — it loses data. Run directly and use `tail` on the output file.
- Always check `/tmp/re2c_divergence_categories.json` timestamp after runs to verify freshness.
- If results look stale, `cargo build --release -p talkbank-parser-re2c --tests` forces recompilation.
- Reports are overwritten on each run. Compare timestamps, not just content.

### Lexer Validation Status

The lexer has been validated against the wild `.cha` corpus with ZERO errors on valid CHAT data.

## Error Token Design

Every re2c condition has a per-condition error fallback (`ErrorInMainContent`, `ErrorInMorContent`, etc.) that:
1. Consumes exactly one character
2. Stays in the same condition (lexing continues)
3. Carries context about WHERE the error occurred

The lexer NEVER fails — it always returns tokens, some of which may be error tokens.

## MISSING-Token Recovery Policy

When the canonical (tree-sitter) parser encounters a malformed input
that would otherwise fail to parse, it recovers by inserting a
zero-length **MISSING** placeholder for the expected terminal and
continues — visible in `tree-sitter parse` output as
`(MISSING <kind> [row, col] - [row, col])`. The talkbank-parser
tree-sitter side handles this with a two-track strategy: silent
recovery in the model AST plus a `ParseError` diagnostic for each
MISSING node (see
`crates/talkbank-parser/src/parser/tree_parsing/parser_helpers/error_checking.rs`
and the `// CRITICAL: Check for MISSING nodes - tree-sitter error
recovery` comments at every tier-level CST entry).

**Re2cParser must mirror that strategy** when it discovers a missing
expected terminal: produce the same recovered model shape (so
`SemanticEq` agrees on the AST) AND emit a matching diagnostic via
`ErrorCollector` (so the malformed input remains visible to validators
and the CLI). "Treat as known divergence" is *not* an option — the
parity goal is whole-AST agreement, including on recovered shapes.

Full policy + concrete examples + the table of construct/recovery
pairs: `docs/parity-report.md` § MISSING-Token Recovery Policy.

## Rust Coding Standards

- Rust **2024 edition**, `cargo fmt` before committing.
- `thiserror` for domain errors, `miette` for rich diagnostics.
- No panics for recoverable conditions. No silent swallowing.
- `tracing` for library logging, never `println!`.
- Every `pub` type and function has a doc comment.
- File size: ≤400 lines recommended, ≤800 hard limit.

## Pipeline Integration

The re2c parser is wired into the main pipeline as an alternative to TreeSitterParser:

```bash
# Use re2c parser for validation
chatter validate --parser re2c resources/corpus/reference/

# Use re2c parser with roundtrip testing
chatter validate --parser re2c --roundtrip resources/corpus/reference/
```

Key integration points:
- `ParserKind::Re2c` in `talkbank-transform/src/validation_runner/config.rs`
- `ParserDispatch` enum in `worker.rs` wraps both parser backends
- `ParserBackend` CLI enum in `chatter-cli/src/cli/args/core.rs`
- Cache keys include the parser label (`"re2c"` vs `"tree-sitter"`)
- TreeSitterParser remains the default; LSP always uses TreeSitterParser (needs incremental parsing)

## Equivalence Status

All reference corpus files pass SemanticEq equivalence with TreeSitterParser.
All reference corpus files validate and roundtrip successfully with `--parser re2c`.
