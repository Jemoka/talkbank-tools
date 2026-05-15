# Error Spec Format Reference

This document defines the exact format of `resources/spec/errors/*.md` files. These files
are the **source of truth** for error code test cases. Generators in
`spec/tools/` read them to produce tree-sitter corpus tests, Rust tests, and
documentation.

## File Naming

```
resources/spec/errors/E{NNN}_{suffix}.md
```

- `NNN`: 3-digit error code (e.g., `202`, `501`)
- `suffix`: typically `auto` (auto-generated from corpus) or a descriptive name
- Examples: `E202_auto.md`, `E501_auto.md`, `E707.md`

## Required Sections

### H1: Title

```markdown
# E202: Missing form type after @
```

Format: `E{code}: {human-readable name}`. The code must match the filename.

### Description

```markdown
## Description

Missing form type after @ symbol in a word. The `@` character marks a
special form type (e.g., `@b` for babbling, `@l` for letter) but the
form type identifier is missing.
```

First paragraph under `## Description` is extracted as the spec's description.

### Metadata

```markdown
## Metadata

- **Error Code**: E202
- **Category**: Parser error
- **Level**: word
- **Layer**: parser
- **Status**: not_implemented
```

All fields use the `**Field**: value` format inside a markdown list.

#### Metadata Fields

| Field | Required | Values | Effect on Test Generation |
|-------|----------|--------|--------------------------|
| **Error Code** | Yes | `E{NNN}` or `W{NNN}` | Must match filename. Used for expected error code assertions. |
| **Category** | Yes | Free text | Grouping only. Common: `Parser error`, `Header validation`, `Word validation`, `parser_recovery`, `tier_parse` |
| **Level** | Yes | `word`, `tier`, `utterance`, `header`, `file` | Determines which parse method the test calls |
| **Layer** | Yes | `parser` or `validation` | **Critical.** Determines test structure (see below). |
| **Status** | No | `not_implemented` | If present, generates `#[ignore]` on the test function. |

#### Layer: How It Affects Tests

**`Layer: parser`** — The generated test calls `parser.parse_chat_file()` and
expects it to return `Err`. The test then checks that the returned errors contain
the expected error code. Use this for inputs that cause a hard parse failure
(the parser cannot produce a valid AST at all).

```rust
// Generated code for Layer: parser
let result = parser.parse_chat_file(input);
let errors = match result {
    Ok(_) => return Err("Expected parse error but parsing succeeded"),
    Err(errors) => errors,
};
// Assert expected error code is in errors
```

**`Layer: validation`** — The generated test uses the streaming parse+validate
path. The parser may succeed (return `Ok`) but errors are collected in the
error sink during both parsing and validation. Use this for inputs where the
parser recovers but reports warnings/errors, or where the error is caught by
post-parse validation.

```rust
// Generated code for Layer: validation
let (chat_file, errors) = parse_and_validate(input);
// Assert expected error code is in errors
```

**Common mistake**: Parser recovery errors (e.g., E326 `unsupported_line`) should
use `Layer: validation` because the tree-sitter parser recovers and returns `Ok`
with errors in the sink. Using `Layer: parser` would fail because
`parse_chat_file()` succeeds.

#### Status: not_implemented

Adds `#[ignore]` to the generated test. Use for:
- Error codes defined in Rust but not yet wired to emission sites
- Error codes that are internal/deprecated (E001, E002, E211, etc.)
- Specs where the example doesn't trigger the intended code due to tree-sitter
  error recovery routing

### Examples

```markdown
## Example 1

**Source**: `error_corpus/E2xx_word_errors/E202_empty_word.cha`
**Trigger**: @ symbol with no form type marker
**Expected Error Codes**: E316

\```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@ world .
@End
\```
```

#### Example Fields

| Field | Required | Effect |
|-------|----------|--------|
| **Source** | No | Provenance note (informational only) |
| **Trigger** | No | Human description of what triggers the error |
| **Expected Error Codes** | No | Comma-separated list. **Overrides** the spec's own error code for this example. |

#### Code Block Info String

The code fence info string (language tag) determines the parse context:

| Info string | Parse method called | When to use |
|-------------|-------------------|-------------|
| `chat` | `parse_chat_file()` | Full CHAT file with headers, utterances, `@End` |
| (empty) | `parse_utterance()` | Bare utterance content |
| `standalone_word` | `parse_word()` | Single word |
| `mor_dependent_tier` | `parse_mor_tier()` | %mor tier content |
| `gra_dependent_tier` | `parse_gra_tier()` | %gra tier content |
| Other tier types | Corresponding parse method | Tier-specific parsing |

**Most error specs use `` ```chat `` `` because errors typically need full file
context (headers, speakers, etc.).**

When `context == "chat"`, the generator maps it to `"chat_file"` internally.
When the info string is empty, it defaults to `"utterance"`.

#### Expected Error Codes Override

By default, each example tests for the spec's own error code (from the Metadata
section). The `**Expected Error Codes**` field overrides this per-example. This
is useful when:
- A spec's input triggers a different error code than the spec itself documents
- Multiple error codes are expected from one input
- The spec demonstrates related errors

Example:
```markdown
**Expected Error Codes**: E316, E501
```

### Expected Behavior

```markdown
## Expected Behavior

The parser should reject this input and report E202 at the location of the
bare @ symbol.
```

Optional. Human-readable description of what should happen.

### CHAT Rule

```markdown
## CHAT Rule

See CHAT manual: https://talkbank.org/0info/manuals/CHAT.pdf
```

Optional. Link to the relevant CHAT manual section.

### Notes

```markdown
## Notes

- Auto-generated from error corpus
- The tree-sitter grammar routes this through the X fallback path
```

Optional. Implementation notes, caveats, status explanations.

## Multiple Examples

A spec can have multiple examples. Each gets its own test function:

```markdown
## Example 1

**Trigger**: First trigger scenario

\```chat
... first CHAT input ...
\```

## Example 2

**Trigger**: Second trigger scenario
**Expected Error Codes**: E316

\```chat
... second CHAT input ...
\```
```

Generated test names: `test_e202_auto_utf8_begin_languages_0` (example 1),
`test_e202_auto_utf8_begin_languages_1` (example 2).

## Complete Example

```markdown
# E501: MissingParticipantsHeader

## Description

The required @Participants header is missing from the CHAT file.

## Metadata

- **Error Code**: E501
- **Category**: Header validation
- **Level**: header
- **Layer**: parser

## Example 1

**Trigger**: CHAT file without @Participants line

\```chat
@UTF8
@Begin
@Languages:	eng
@End
\```

## Expected Behavior

The parser should report E501 because @Participants is a required header.

## CHAT Rule

CHAT files must contain @Participants before any utterance lines.
See: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- One of the most common validation errors in real-world CHAT files.
```

---

# Construct Spec Format Reference

Construct specs in `resources/spec/constructs/` define **valid** CHAT examples with their
expected CST (Concrete Syntax Tree). These drive tree-sitter corpus test
generation.

## File Location

```
resources/spec/constructs/{category}/{name}.md
```

Categories: `header/`, `main_tier/`, `tiers/`, `utterance/`, `word/`

## Required Sections

### H1: Name

```markdown
# mor_basic_3
```

Used as the test name in the generated tree-sitter corpus file.

### Description (paragraph)

```markdown
Basic %mor tier with adjective and noun
```

Brief description (informational).

### Input

````markdown
## Input

```standalone_word
a:
```
````

The code fence info string specifies the **template** used to wrap the fragment
into a valid CHAT file for testing.

#### Templates

Templates live in `crates/spec/talkbank-spec-testgen/templates/`. Each wraps a fragment in the minimal
CHAT structure needed for tree-sitter to parse it:

| Template | Use for |
|----------|---------|
| `standalone_word` | Single word fragments |
| `utterance` | Utterance content (words + annotations) |
| `main_tier` | Full main tier line (`*CHI: ...`) |
| `mor_dependent_tier` | %mor tier content |
| `gra_dependent_tier` | %gra tier content |
| `pho_dependent_tier` | %pho tier content |
| `wor_dependent_tier` | %wor tier content |
| `com_dependent_tier` | %com tier content |
| `chat` | Full CHAT file (no wrapping needed) |
| `participants_header` | @Participants header line |
| `languages_header` | @Languages header line |
| `overlap_point` | Overlap point markers |

If the info string doesn't match any template, test generation fails.

### Expected CST

````markdown
## Expected CST

```cst
(standalone_word
  (word_body
    (initial_word_segment)
    (word_content
      (colon)
    )
  )
)
```
````

S-expression of the expected tree-sitter parse tree. Must match the output of
`tree-sitter parse` for the wrapped input. Whitespace and indentation are
normalized during comparison.

### Metadata

```markdown
## Metadata

- **Level**: word
- **Category**: lengthening
```

| Field | Required | Values |
|-------|----------|--------|
| **Level** | Yes | `word`, `tier`, `utterance`, `header`, `file` |
| **Category** | Yes | Free text, matches directory name |

## Workflow

1. Create or edit spec in `resources/spec/constructs/{category}/`
2. Ensure a matching template exists in `crates/spec/talkbank-spec-testgen/templates/`
3. `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` — regenerates `tree-sitter-talkbank/test/corpus/`
4. `cd ../tree-sitter-talkbank && tree-sitter test` — verify
5. `bazel build //... && bazel test //...` — full gate check

---

# Tools Reference

## Generators (run via `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`)

| Binary | What it generates |
|--------|-------------------|
| `gen_tree_sitter_tests` | Tree-sitter corpus tests from construct specs |
| `gen_rust_tests` | Rust error tests from error specs |
| `gen_validation_tests` | Rust validation-layer tests from error specs |
| `gen_error_docs` | Markdown error documentation pages |

## Validators

| Binary | What it checks |
|--------|----------------|
| `validate_spec` | Construct spec format integrity |
| `validate_error_specs` (`spec/runtime-tools`) | Error spec format, layer correctness |

## Coverage

| Binary | What it measures |
|--------|-----------------|
| `coverage` | Error spec coverage (specs per error code) |
| `corpus_node_coverage` | Grammar node type coverage in corpus |

## Corpus Tools

| Binary | Purpose |
|--------|---------|
| `bootstrap` | Initial spec bootstrapping |
| `bootstrap_tiers` (`spec/runtime-tools`) | Tier spec bootstrapping |
| `corpus_to_specs` | Convert error corpus fixtures to specs |
| `extract_corpus_candidates` (`spec/runtime-tools`) | Select reference corpus files from corpus data |
| `perturb_corpus` | Generate error files by mutating valid files |
| `enhance_specs` | Bulk-fix spec metadata and formatting |
| `fix_spec_layers` | Auto-correct parser/validation layer mismatches |

## Golden Artifact Generators (in talkbank-parser-tests)

| Binary | Output |
|--------|--------|
| `generate_golden_words` | `golden_words.txt` — word corpus |
| `generate_golden_mor_tiers` | `golden_mor_tiers.txt` — %mor tiers |
| `generate_golden_gra_tiers` | `golden_gra_tiers.txt` — %gra tiers |
| `generate_golden_pho_tiers` | `golden_pho_tiers.txt` — %pho tiers |
| `generate_golden_wor_tiers` | `golden_wor_tiers.txt` — %wor tiers |
| `generate_golden_sin_tiers` | `golden_sin_tiers.txt` — %sin tiers |
| `generate_golden_com_tiers` | `golden_com_tiers.txt` — %com tiers |
| `generate_golden_main_tiers` | `golden_main_tiers.txt` — main tiers |
| `audit_golden_words` | `golden_words_featured.txt`, `golden_words_minimal.txt` |
| `bootstrap_reference_corpus` | `tests/generated/reference_corpus.rs` |

---
Last Updated: 2026-02-27
