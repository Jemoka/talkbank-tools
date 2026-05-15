# Re2cParser Parity Report

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Overview

The re2c parser is an alternative CHAT parser using a [re2c](https://re2c.org/) DFA
lexer and [chumsky](https://docs.rs/chumsky/1.0.0-alpha.8) parser combinators. It
implements the `ChatParser` trait, making it a drop-in replacement for
`TreeSitterParser`.

TreeSitterParser is the canonical oracle. The re2c parser must match its output
on valid CHAT and detect the same errors on invalid CHAT.

## Current Metrics

| Metric | Value | Notes |
|--------|-------|-------|
| **Speed** | 4-8x faster | 7.2x on 35-file batch (divan benchmarks) |
| **Reference corpus** | 100/100 match | 100% SemanticEq on all reference files (added regression gates: edge-cases/other-spoken-event-no-terminator-space, edge-cases/event-segment-with-caret, edge-cases/group-without-retrace-marker-recovers — see cycle log) |
| **Error detection** | 140/140 (100%) | Every testable error spec is detected |
| **Error recovery** | 241/241 (100%) | Zero panics on any invalid input |
| **Error code match** | 79/140 (56.4%) | Same code as TreeSitter |
| **Both detect, diff code** | 61/140 (43.6%) | Different code, both report error |
| **Silent gaps** | 0 | No case where re2c misses an error |
| **Both empty** | 0 | No spec where neither parser reports |
| **Wild corpus** | ~98.9% parity | residual divergences in event-body alignment and group-recovery edge cases |
| **Crate tests** | 278/278 pass | Plus 18 skipped (not_implemented) |

## Architecture

```
Source text → re2c DFA Lexer → Token slice → Chumsky Combinators → AST → Model
                (lexer.re)       (token.rs)    (parser/*.rs)      (ast.rs) (convert.rs)
```

- **Lexer:** re2c DFA with 13 start conditions, rich tokens (Word, MorWord, etc.)
- **Parser:** 8 chumsky modules (1,838 lines total), no legacy Parser struct
- **Convert:** `From<&ast::Type>` impls, source-free (AST carries `raw_text`)

## Error Detection

Both parsers detect errors on all 140 testable error specs. The 61 cases where
error codes differ are because the architectures produce different diagnostics:

- Re2c lexer errors (token-level) vs TreeSitter ERROR nodes (CST-level)
- Different alignment validation paths for structurally different models
- Re2c reports E321 (UnparsableUtterance) where TS reports E316 (UnparsableContent)

Neither code set is "wrong" — both report user-actionable diagnostics. The
re2c parser's error codes are specific (E321, E319, E316, E602) rather than
generic (E309 was eliminated).

## Known Divergences

### Remaining wild-corpus divergences (~1,300 files)

The pre-chumsky divergence count was 10,068 (10.1%). After the chumsky pivot
and subsequent fixes, it's ~1,300 (~1.3%). Root causes:

| Category | Impact | Status |
|----------|--------|--------|
| Bullet classification | Was #1 (6,680 files) | Fixed (extract_terminal_bullet) |
| Event trailing `>` | Was #2 (570 files) | Fixed (chumsky group scoping) |
| Missing dependent tiers | Was #3 (1,252 files) | Mostly fixed |
| Content length cascade | Was #4 (6,680 files) | Fixed (consequence of above) |
| CA terminator promotion | Dozens of files | Fixed (resolve_ca_terminator ordering) |
| Error marker spaces | MSU03b + similar | Fixed (lexer ErrorMarkerAnnotation fix) |
| Standalone shortening | Several files | Fixed (lexer rule ordering) |
| Standalone colon | Several files | Fixed (Colon separator token) |
| OSE no-terminator-space | 1 file (botes.cha) | Fixed — lexer.re OSE rule body changed `[^ \t\r\n\x00]+` → `w_body` so `&*INV:oh_hm.` stops at the period, not after it |
| Event body chars `^{}\$` over-restricted | 6 files | Fixed — `ev_char` exclusion realigned to grammar.js's `EVENT_SEGMENT_FORBIDDEN` (was forbidding `^`, `{`, `}`, `$` despite grammar.js permitting them inside `&=...` bodies; e.g. `&=reads:boardLas^_Palmas` from shaddend.cha was splitting into event + word + word). |
| Group-without-retrace-marker recovery | 1 file (tele19a.cha) | Fixed — `parser/main_tier.rs::contents_parser` now mirrors tree-sitter's MISSING-token recovery: when `<...>` has no following annotation, produces `Retrace { kind: Full, is_group: true }` matching TS's recovered AST. Diagnostic emission deferred — see § MISSING-Token Recovery Policy. |
| `&=&=label` double event | 2 files | Data quality issue (& now forbidden) |
| Skip bullet dash | 7 files | Data quality issue (skip deprecated) |

### Top remaining categories

Out of 99,907 wild-corpus files, ~1,114 still diverge. The top
categories named by `tests/categorize_divergences.rs`:

| Category | Files | Top representative |
|----------|------:|--------------------|
| `main_tier/other` | 400 | Bergmann/003_Moramin1.cha:9 (re2c content len=3 vs ts len=5 — main-tier content-item count differs; new top after cycle 3 retired the tele19a-class) |
| `dep_tier/other` | 289 | C-ORAL-IC/2018_699.cha:30 (gra-tier item count differs — next-cycle target) |
| `line_count_mismatch` | 1 | CallHome deu/6838.cha (re2c emits 359 lines, ts emits 360 — single residual, likely a header-classification edge case) |
| `header/other` | 1 | Rigol/Sebastian/001001.cha (corpus field missing in re2c) |

These are the next slots for parity-push work. Run
`cargo test -p talkbank-parser-re2c --test categorize_divergences --release -- --ignored`
to refresh; output goes to `/tmp/re2c_divergence_categories.json`.

### Infrastructure improvements landed alongside the OSE fix

- `equivalence_reference_corpus` test now `assert!()`s on `failed_files`
  emptiness. The previous loop only `eprintln!`'d failures; the test
  silently "passed" with mismatches present.
- Reference-corpus directory list extended from
  `[core, content, annotation, tiers, ca, languages]` to also include
  `[edge-cases, audio, word-features]`. Tests had been blind to those
  three subdirs — any reference fixture there bypassed the equivalence
  oracle entirely.

### Not-implemented error specs (88 specs)

These are error specs where the validation code hasn't been written yet. They
apply equally to both parsers (validation-layer, not parser-layer):

- 75 original `not_implemented` specs
- 13 newly added (E360 deprecated skip, etc.)

## CLI Usage

```bash
# Use re2c parser for validation (faster than default tree-sitter)
chatter validate --parser re2c corpus/

# Use re2c parser with roundtrip testing
chatter validate --parser re2c --roundtrip corpus/

# Default: tree-sitter (supports incremental reparsing for LSP)
chatter validate corpus/
```

The `--parser re2c` flag selects the re2c backend for all validation in that
run. The cache is parser-aware (separate entries for `tree-sitter` vs `re2c`).

**When to use re2c:**
- Batch validation of large corpora (4-8x faster)
- Testing parser parity (specification oracle)
- Profiling parse performance

**When to use tree-sitter (default):**
- LSP (requires incremental reparsing)
- When error code specificity matters (TreeSitter has more specific E3xx codes)
- When CST-level information is needed

## Benchmarks

Measured with divan on reference corpus files. All content pre-loaded; zero I/O.

### File-level parse (median, parser reuse)

| File | TreeSitter | Re2c+Chumsky | Speedup |
|------|-----------|-------------|---------|
| basic-conversation (13 lines) | 44 µs | 9.6 µs | 4.6x |
| mor-gra (dependent tiers) | 69 µs | 9.4 µs | 7.3x |
| intonation (CA) | 78 µs | 19 µs | 4.1x |
| zho-conversation (CJK) | 128 µs | 19 µs | 6.6x |
| impdenis (complex, large) | 7,734 µs | 970 µs | 8.0x |

### Batch (35 files)

| Parser | Time | Files/sec |
|--------|------|-----------|
| TreeSitter | 21.7 ms | 1,613 |
| Re2c+Chumsky | 3.0 ms | 11,667 |

Run benchmarks: `cargo bench -p talkbank-parser-re2c --bench parse_comparison`

## Shared Infrastructure

Both parsers share post-hoc promotion logic in the model crate:

- `TierContent::extract_terminal_bullet()` — moves trailing InternalBullet
  to the utterance-level bullet field
- `TierContent::resolve_ca_terminator()` — promotes trailing CA intonation
  arrow separator to terminator
- `parse_bullet_node_timestamps()` — extracts (start_ms, end_ms) from
  structured bullet CST nodes

These shared methods eliminate all duplication between the two parsers' convert
paths.

## MISSING-Token Recovery Policy

Tree-sitter's parser emits **MISSING tokens** as part of its built-in error
recovery: when a required terminal is absent at a position, tree-sitter
inserts a zero-length placeholder of the expected kind so the surrounding
parse can continue. Visible in `tree-sitter parse` output as
`(MISSING <kind> [row, col] - [row, col])` with identical start and end
spans.

The talkbank-parser tree-sitter side handles this with a **two-track strategy**:

1. **Model track (silent recovery).** The CST→model conversion ignores the
   zero-length distinction and treats the MISSING placeholder as if a real
   token were there. So a group `<I don't>` followed by a synthetic-MISSING
   `retrace_complete` becomes a `Retrace { is_group: true, kind: full, ... }`
   in the model — indistinguishable from a properly-marked
   `<I don't> [//]`.
2. **Diagnostic track (loud reporting).** A separate post-parse walker at
   `crates/talkbank-parser/src/parser/tree_parsing/parser_helpers/error_checking.rs`
   recursively visits every node, calls `node.is_missing()`, and converts
   each hit into a `ParseError` with the message
   `"Missing required '<kind>' ... (tree-sitter error recovery)"`. The
   pattern is also inlined at every tier-level CST entry point with the
   marker comment `// CRITICAL: Check for MISSING nodes - tree-sitter
   error recovery` so contributors don't accidentally promote MISSING
   placeholders to first-class data.

Callers therefore see *both*:

- A usable model AST (downstream code that doesn't care about
  malformed-input distinctions just keeps working on the recovered
  structure).
- A diagnostic stream that flags the malformed input (validators,
  CLI exit codes, LSP underlines).

### Re2c parity for the same recovery

`Re2cParser` mirrors the same two-track strategy where it can — re2c
already has an `ErrorCollector` infrastructure (`errors.report(...)` in
`parser/file.rs`) for the diagnostic track. The model track is achieved
inside the chumsky combinators that own each construct: when the
expected terminal would have completed a structured node and isn't
present, the combinator fabricates the same recovered shape tree-sitter
would produce, then the combinator (or its caller) emits a matching
diagnostic via the `ErrorCollector`.

**The contract is: `SemanticEq(ts_model, re2c_model) == true` even on
malformed input.** Diagnostics are not compared by `SemanticEq`, but
producing one is still required — the parser parity oracle measures
model agreement; the validator suite measures diagnostic agreement.

### Concrete examples

| Construct | TS recovery | Re2c recovery | Status |
|-----------|-------------|---------------|--------|
| `<group>` with no following retrace marker | MISSING `retrace_complete` → recovered as `Retrace { kind: full, is_group: true }` + ParseError | (target: same) | **In progress** |
| Other tier-level MISSING terminals | Per-tier `is_missing()` checks at `tier_parsers/{mor,gra,pho,sin}/...` | Per-rule chumsky recovery + `errors.report()` in `parser/file.rs` | Mostly aligned |

If you find a MISSING-token shape where the two parsers disagree, the
expected fix is to mirror the recovery on the re2c side, not to "treat
it as a known divergence" — the recovered AST keeps callers working,
and the diagnostic keeps the malformed input visible.

## Grammar Unification

The grammar now uses a single structured `bullet` rule (was: opaque `media_url`
token + structured `inline_bullet`). Both parsers extract timestamps from
structured CST/AST children. The deprecated `skip` flag (dash before closing
NAK) was removed from the grammar and model.

## Module Structure

```
src/parser/
  mod.rs              (43)   Module decls, lex_to_tokens
  main_tier.rs       (536)   Chumsky: contents, words, groups, tier_body
  dependent_tiers.rs (334)   Chumsky: %mor, %gra, %pho, %sin, %wor, text
  classify.rs        (252)   Token classification functions
  word_body.rs       (231)   Char-level word body scanner
  file.rs            (~300)  Imperative file parser + error reporting
  entry_points.rs    (155)   Public API
  headers.rs          (75)   Chumsky: @ID, @Languages, @Participants
```
