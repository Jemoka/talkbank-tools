# Parser Leniency Policy

**Status:** Current
**Last updated:** 2026-05-01 05:19 EDT

This document is the single source of truth for how the tree-sitter grammar,
Rust validation layer, and CLI tooling divide responsibility for enforcing the
CHAT specification. It consolidates decisions scattered across `grammar.js`
comments, analysis documents, and code.

> **Scope**: Documentation only. This document does not implement new validation
> rules — it records what exists, what is intentionally absent, and proposes a
> roadmap for closing gaps.

---

## Philosophy: Parse, Don't Validate

The tree-sitter grammar intentionally accepts a **superset** of valid CHAT. The
rationale:

1. **Maximise parse coverage** — Real-world `.cha` files contain legacy patterns,
   whitespace variations, and edge cases. A grammar that rejects them produces no
   AST and therefore no diagnostics. Accepting them gives the validation layer
   something to work with.

2. **Separate syntax from semantics** — The grammar captures structure (headers,
   utterances, tiers, annotations). The Rust validation layer enforces semantic
   rules (required headers, participant declarations, alignment counts).

3. **Enable configurable strictness** — Different consumers need different
   policies. A roundtrip pipeline can be strict; an editor providing live
   diagnostics should be lenient. Validation profiles (see
   [Validation Profile Infrastructure](#validation-profile-infrastructure)) make
   this possible.

### Three-Tier Classification

Every intentional leniency decision falls into one of three tiers:

| Tier | Label | Meaning |
|------|-------|---------|
| **A** | Parse-lenient + validate-strict | Grammar accepts it; validation **rejects** it as an error |
| **B** | Parse-lenient + validate-warning | Grammar accepts it; validation emits a **warning** |
| **C** | Parse-lenient only | Grammar accepts it; **no validation needed** — the construct is genuinely optional or the broad acceptance is by design |

This classification was proposed in an earlier grammar governance analysis and is
formalised here.

---

## Leniency Matrix

Master table of every documented leniency decision in the grammar. The
**Status** column indicates whether downstream validation compensates for the
grammar's permissiveness.

| # | Grammar Construct | Spec Requirement | Grammar Behavior | Tier | Validation | Error Code | Status |
|---|---|---|---|---|---|---|---|
| 1 | `@UTF8` header | Required, must be first line | Optional (not enforced) | A | Validated | E503 | OK |
| 2 | `@Begin` header | Required | Optional (`grammar.js` ~L104) | A | Validated | E504 | OK |
| 3 | `@End` header | Required | Optional (`grammar.js` ~L106) | A | Validated | E502 | OK |
| 4 | Pre-first-utterance header order | No enforced order (matches CLAN CHECK) | `choice()`, any order (`grammar.js` ~L122–135) | C | N/A (by design) | — | OK |
| 5 | Headers after utterances | Allowed (e.g. `@Bg`, `@Eg`, `@G`, `@Comment`) | Interleaved freely | C | N/A (by design) | — | OK |
| 6 | Content type context restrictions | Unified across contexts | Unified `base_content_item` (`grammar.js` ~L731–738) | C | N/A (by design); specific semantic rules (E371, E372) exist separately | — | OK |
| 7 | Terminator presence | Required (except CA mode) | Optional (`grammar.js` ~L691–692) | A | Validated | E305 | OK |
| 8 | Bare shortening as word | CA mode only | Accepted anywhere | A | Validated | E2xx | OK |
| 9 | Trailing whitespace in annotations | Not specified | Optional trailing space (`grammar.js` ~L957, 966, 975, 1004, 1013) | C | N/A | — | OK |
| 10 | MOR segment Unicode | Very permissive (broad language support) | Exclusion-based regex (`grammar.js` ~L1909–1915) | C | N/A (by design) | — | OK |
| 11 | MOR fusional suffixes with hyphens | ALNUM + IPA only | Allows hyphens (`grammar.js` ~L1942–1945) | C | N/A (by design) | — | OK |
| 12 | MOR nested translations | No nested structures | Allows `()` and `[]` nesting (`grammar.js` ~L1954–1966) | C | N/A (by design) | — | OK |
| 13 | Linkers / language codes | Truly optional | Optional | C | N/A | — | OK |
| 14 | Word annotations | Truly optional | Optional | C | N/A | — | OK |
| 15 | Media bullet | Truly optional | Optional | C | N/A | — | OK |
| 16 | Group whitespace (leading/trailing) | No whitespace inside `<` `>` | Optional (`grammar.js` ~L1097, 1099) | C | N/A | — | OK |
| 17 | Long feature label characters | Limited character set | `/[A-Za-z0-9@%_-]+/` (`grammar.js` ~L1327) | C | N/A | — | OK |
| 18 | Catch-all headers (`$.anything`) | Structured content for some headers | `/[^\r\n]+/` for ~19 header types | C | N/A (content is opaque) | — | OK |
| 19 | Header gap whitespace | Single space/tab | `repeat1(choice(space, tab))` (`grammar.js` ~L467, 477, 489) | C | N/A | — | OK |
| 20 | `@Types` header whitespace | No spaces around commas | Optional whitespace around commas (`grammar.js` ~L584–592) | C | N/A | — | OK |

---

## Permissiveness Regression Decisions

During development, several validation rules were tightened and then relaxed
after they produced false positives against the reference corpus. These
decisions are documented in the permissiveness regression log (archived). Each is
summarised here with its rationale.

### Decision 1: `[*]` bare annotation — E214 disabled

- **Previous behaviour**: `E214` emitted when `[*]` appeared without an explicit
  error code (empty `ContentAnnotation::Error`).
- **Current behaviour**: Bare `[*]` is accepted without error.
- **Implementation**: Removed validation branch in
  `talkbank-model/src/model/annotation/annotated.rs`.
- **Rationale**: Reference files (`errormarkers.cha`, `compound.cha`) use bare
  `[*]` as valid CHAT.
- **Revisit**: If coded error annotations become required, do it behind an
  explicit strict profile.

### Decision 2: `@t` without `@s:<lang>` — E248 disabled

- **Previous behaviour**: `E248` emitted for `@t` markers without an explicit
  language marker.
- **Current behaviour**: `@t` accepted without requiring `@s:<lang>`.
- **Implementation**: Removed checks in
  `talkbank-model/src/validation/word/structure.rs`.
- **Rationale**: Reference file `formmarkers.cha` contains `a@t` and is expected
  to be valid.
- **Revisit**: Scope to explicit strict validation mode if desired.

### Decision 3: Undeclared inline language codes — E254 removed

- **Previous behaviour**: Inline `@s:...` markers with language codes not
  declared in `@Languages` emitted `E254`.
- **Current behaviour**: No `E254` emitted; error code removed from codebase.
- **Implementation**: Removed checks in
  `talkbank-model/src/validation/word/language/resolve.rs`.
- **Rationale**: Reference file `lang-marker.cha` exercises undeclared codes.
- **Revisit**: Team decision needed — strict declaration enforcement vs
  permissive inline experimentation.

### Decision 4: Mixed-language digit legality — permissive-any rule

- **Previous behaviour**: Digits had to be legal in **all** applicable languages
  for mixed/ambiguous markers.
- **Current behaviour**: Digits accepted if legal in **at least one** applicable
  language.
- **Implementation**: Changed from `is_valid_in_all()` to `any()` in
  `talkbank-model/src/validation/word/language/digits.rs`.
- **Rationale**: Prevents false positives in mixed-language reference examples.
- **Revisit**: Confirm spec intent for mixed/ambiguous validation semantics.

### Decision 5: `@Bg` nesting — same-label only

- **Previous behaviour**: Any nested `@Bg` while another gem scope was open
  emitted `E529`.
- **Current behaviour**: `E529` only fires when nesting the **same label** (or
  same unlabeled scope key). Different labels may nest hierarchically.
- **Implementation**: Changed from `any_scope_open` to `same_scope_open` in
  `talkbank-model/src/validation/header/structure.rs`.
- **Rationale**: Avoids false positives on hierarchical markup patterns (e.g.,
  HSLLD corpus).
- **Revisit**: Decide whether nesting policy should be global or per-label.

### Decision 6: Temporal bullets in CA mode — skipped

- **Previous behaviour**: `E701`/`E704` temporal checks ran even for CA-mode
  files.
- **Current behaviour**: Temporal constraints are skipped when file is in CA
  mode.
- **Implementation**: `validate_temporal_constraints()` early-returns when
  `ca_mode` is true (`talkbank-model/src/validation/temporal.rs`).
- **Rationale**: CA reference files include patterns that triggered false
  monotonicity/self-overlap diagnostics.
- **Revisit**: Implement CA-specific temporal policy rather than global skip.

### Decision 7: Pipeline severity threshold — errors only

- **Previous behaviour**: Any validation diagnostic (including warnings) caused
  `PipelineError::Validation`.
- **Current behaviour**: Pipeline returns failure only if at least one diagnostic
  has `Severity::Error`.
- **Implementation**: `talkbank-transform/src/pipeline/parse.rs`.
- **Rationale**: Warnings should not block parse/transform/export pipelines.
- **Revisit**: Keep as default; add explicit `--strict` flag/profile if needed.

### Decision 8: Spacing warnings W210/W211 — disabled

- **Previous behaviour**: Style-level spacing warnings around terminators and
  overlap markers.
- **Current behaviour**: Checks removed from core main-tier validation path.
- **Implementation**: `check_spacing_warnings()` invocation removed from
  `talkbank-model/src/model/content/main_tier.rs`.
- **Rationale**: Generated unexpected diagnostics on files treated as valid in
  reference workflow.
- **Revisit**: Reintroduce as optional lint profile, not core validator hard
  path.

---

## Validation Gap Roadmap

Concrete items where the grammar is lenient but no validation compensates.
Each proposes a new error code and priority.

### ~~Priority 1: `@UTF8` Presence (E503)~~ — DONE

- **Grammar**: `@UTF8` is optional.
- **Spec**: Required, must be the first line.
- **Implemented**: `E503` (`MissingUTF8Header`) added to `check_headers()` in
  `talkbank-model/src/validation/header/structure.rs`.
- **Severity**: Error.
- **Note**: All 340 reference corpus files contain `@UTF8` — zero roundtrip
  impact.

### ~~Priority 2: Pre-First-Utterance Header Order (proposed E534)~~ — Not a Gap

- **Grammar**: `choice()` accepts headers in any order between `@Begin` and the
  first utterance.
- **Assessment**: CLAN CHECK does not enforce any ordering for post-`@Begin`
  headers — it validates presence and format only. Our grammar's flexible
  ordering matches CHECK's behavior.
- **Status**: Reclassified from Tier B (GAP) to Tier C (by design).

### ~~Priority 3: Content Type Context Validation~~ — Not a Gap

- **Grammar**: Unified `base_content_item` accepts any content type in any
  context.
- **Assessment**: The unified rule is correct by design. Nested groups are legal
  CHAT (e.g., `<the <dag> [: dog]> [= something]`). The two specific semantic
  restrictions that do exist (no pauses in pho groups — E371; no nested
  quotations — E372) are already validated.
- **Status**: Reclassified from Tier A (PARTIAL) to Tier C (by design).

---

## Validation Profile Infrastructure

### What Exists

#### `ValidationConfig` (`talkbank-model/src/errors/config.rs`)

Builder-pattern configuration for per-error-code severity overrides.

```rust,ignore
let config = ValidationConfig::new()
    .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
    .disable(ErrorCode::InvalidOverlapIndex)
    .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
```

**API**:
- `new()` — empty config, all codes use original severity
- `downgrade(code, severity)` — lower severity (chainable)
- `disable(code)` — suppress entirely (chainable)
- `upgrade(code, severity)` — raise severity (chainable)
- `set_severity(code, Option<Severity>)` — set or disable (chainable)
- `effective_severity(code, original) -> Option<Severity>` — query
- `is_disabled(code) -> bool` — check

**Pre-built profiles**:
- `lenient()` — Downgrades `IllegalUntranscribed` and `InvalidOverlapIndex` to
  `Severity::Warning`. Designed for legacy corpora gradual migration.
- `strict()` — **Placeholder**. Currently returns `Self::new()` (no overrides).

#### `ConfigurableErrorSink` (`talkbank-model/src/errors/configurable_sink.rs`)

Wrapper that intercepts errors and applies `ValidationConfig` before forwarding
to an inner `ErrorSink`.

```rust,ignore
let inner = ErrorCollector::new();
let sink = ConfigurableErrorSink::new(&inner, config);
// Pass `sink` to parser/validator — disabled errors are filtered,
// severity overrides are applied.
```

#### Runner-Level Flags (`talkbank-transform`, `chatter-cli`)

| Flag | Effect |
|------|--------|
| `--skip-alignment` | Skip tier alignment validation |
| `--roundtrip` | Test serialization idempotency after validation |
| `--force` | Clear cache for path and revalidate |
| `--max-errors N` | Stop after N errors |

### What Is Missing

| Gap | Description | Effort |
|-----|-------------|--------|
| `strict()` profile not implemented | Returns empty config; should upgrade all warning codes to errors | Small |
| No `--profile` CLI flag | Users cannot select `strict` / `lenient` / `lint` from the command line | Medium |
| `ConfigurableErrorSink` not wired into validation pipeline | Infrastructure exists but is not used by `chatter validate` | Medium |
| No lint-style profile | Spacing/style warnings (W210, W211) have no home | Small (once profiles are wired) |
| No profile serialization | Cannot load profiles from TOML/JSON config files | Medium |
| No corpus-specific profiles | E.g., HSLLD-specific rules | Future |

### Proposed Profiles

From the permissiveness regression log:

| Profile | Purpose | Behaviour |
|---------|---------|-----------|
| `reference-compatible` | Current permissive baseline | Default — matches current validation behaviour |
| `strict-chat` | Full spec enforcement | Re-enable selected tightenings (E214, E248, E254, etc.) |
| `lint-style` | Spacing/style warnings only | Enable W210, W211; do not fail pipeline |

The roundtrip gate should be pinned to an agreed profile to prevent future
ambiguity about what "pass" means.

---

## Silent Recovery Points (NLP Pipelines)

An earlier Python-Rust boundary audit identified several
places where `batchalign-core` silently massages data without diagnostics. These
are related to leniency because they represent permissive acceptance without
transparency.

| Pipeline | Recovery Mechanism | Diagnostics? |
|----------|-------------------|-------------|
| Stanza morphosyntax | `retokenize.rs` DP alignment; `Word::new_unchecked` fallback | **No** |
| Whisper/Wave2Vec FA | `forced_alignment.rs` DP "best fit" | **No** |
| Google Translate | Imported verbatim into `%xtra` | **No filtering** |
| Stanza segmentation | Silent abort on assignment mismatch | **No** |

**Key infrastructure gap**: `ParseHealth` exists in `talkbank-model` (per-utterance
tier cleanliness flags with `taint()`, `is_clean()`, `can_align_main_to_mor()`
methods). It is used by the tree-sitter and direct parsers during parsing.
However, `batchalign-core` does **not** read, write, or propagate `ParseHealth`
during any mutation (morphosyntax injection, FA injection, retokenisation). The
infrastructure exists in the model layer but is not connected to the pipeline
layer.

---

## Cross-References

| Source | What It Contains |
|--------|-----------------|
| Grammar governance analysis (archived) | Proposed this document; leniency matrix concept; three-tier classification |
| Permissiveness regression log (archived) | 8 permissiveness regression decisions with rationale |
| Python-Rust boundary audit (archived) | Silent recovery points; ParseHealth gap; NLP pipeline audit |
| `grammar/grammar.js` | Inline comments on each leniency decision (line references in matrix above) |
| `talkbank-model/src/errors/config.rs` | `ValidationConfig` API |
| `talkbank-model/src/errors/configurable_sink.rs` | `ConfigurableErrorSink` adapter |
| `talkbank-model/src/validation/header/structure.rs` | Header validation: E501, E502, E503, E504–E533 |
| `talkbank-model/src/validation/temporal.rs` | Temporal constraint checks (E701, E704); CA-mode skip |
| `talkbank-model/src/model/content/main_tier.rs` | Where W210/W211 were removed |

---

*Last updated: 2026-02-18*
