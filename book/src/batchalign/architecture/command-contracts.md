# Command Contracts: Input Preconditions and Output Guarantees

**Status:** Current
**Last updated:** 2026-05-01 05:25 EDT

This document specifies, for each batchalign3 command that operates
on CHAT files, the minimum input validity required, what the command
reads and writes, and what guarantees the output provides. Each
command applies a **pre-validation gate** so that invalid input is
rejected early rather than wasting compute and producing silently
corrupt output.

Pre-validation and post-validation gates are enforced for the
following commands:

- `morphotag` (`crates/batchalign/src/morphosyntax/`)
- `utseg` (`crates/batchalign/src/utseg.rs`)
- `translate` (`crates/batchalign/src/translate.rs`)
- `coref` (`crates/batchalign/src/coref.rs`)
- `align` (`crates/batchalign/src/fa/`)

These commands call `validate_to_level(...)` before inference and
`validate_output(...)` before serialization. Media-path preflight
validation runs in `crates/batchalign/src/runner/util/` for commands
that consume audio.

---

## Motivation

This contract was introduced to prevent wasted compute and silent corruption
caused by running expensive inference on structurally invalid CHAT content.

`parse_lenient()` still intentionally parses best-effort ASTs, but current server
commands now apply command-specific gates before inference. The remaining task is
to keep this document synchronized with implementation and close any uncovered
command paths.

Representative failure modes that motivated this contract:

1. A file with missing terminators proceeds through forced alignment (92s of GPU),
   only to be rejected by the post-serialization validation gate.
2. A file with a corrupted main tier (e.g., `to+...` parsed as `to+` + `.`) gets
   silently mangled during roundtrip serialization.
3. No command validates that the parts of the file it *reads* are structurally sound
   before investing compute.

The fix: each command should validate exactly what it needs — no more, no less.

---

## Validity Levels

We define four cumulative levels of CHAT validity:

### Level 0: Parseable

The file parses without ERROR nodes in the tree-sitter CST.

- `@UTF8` header present
- `@Begin` / `@End` headers present
- No unrecognized line types (no `unsupported_line` nodes)
- All brackets balanced
- No tokenization errors (no ERROR nodes)

This is the **minimum for any command**. If a file doesn't reach Level 0, it
should be rejected outright with a diagnostic pointing to the parse errors.

### Level 1: Structurally Complete

Level 0, plus:

- `@Participants` header present with at least one participant
- `@Languages` header present with at least one language code
- Every speaker code in utterances is declared in `@Participants`
- Every utterance has a terminator (E304)
- No empty main tiers

This is the minimum for commands that read main tier content.

### Level 2: Main Tier Valid

Level 1, plus:

- No word-level structural errors (E2xx): balanced compound markers (E233),
  valid shortening notation, valid special form markers
- No annotation structural errors: balanced retracing/repetition markers,
  valid overlap notation
- Timing bullets (if present) are well-formed

This is the minimum for commands that extract words for NLP processing.

### Level 3: Fully Valid

Level 2, plus all validation rules pass:

- Tier alignment (E7xx): %mor/%gra/%pho/%sin/%wor word counts match main tier
- Temporal validity (E362, E701, E704): monotonic timestamps, no self-overlap
- Cross-utterance validity: quotation linker pairing, completion annotations
- Header field formats: @Date, @ID fields, @Media format

This is the ideal state, but no command requires it as a precondition.

---

## Command Contracts

### `morphotag` — Morphosyntactic Analysis

**Input:** CHAT file with main tier utterances

**Reads:**
- Main tier words (via `collect_utterance_content()` in `AlignmentDomain::Mor`)
- `@Languages` header (determines Stanza model language)
- Per-utterance language markers (`[- spa]`) for multilingual filtering
- Special form markers (`@s`, `@c`, `@b`) on words for POS override
- Existing `%mor` tiers (to skip already-tagged utterances, unless `--override-media-cache`)

**Writes:**
- `%mor` tier: morphological analysis (POS, lemma, features) per utterance
- `%gra` tier: grammatical dependency relations per utterance
- Main tier words (only when `retokenize=true`): replaces with UD tokenization

**Minimum input validity: Level 2**

The main tier must be fully parseable with valid word structure. Invalid words
(e.g., malformed compound markers) produce garbage NLP input. Existing `%mor`
and `%gra` tiers may be absent or invalid — they will be replaced.

**What may be invalid in the input:**
- `%mor`, `%gra` tiers (will be overwritten)
- `%pho`, `%sin`, `%wor` tiers (untouched, validity irrelevant)
- `%xtra`, `%xcoref` tiers (untouched)
- Timing bullets (untouched, validity irrelevant)
- Temporal ordering (untouched)
- `@Date`, `@ID` field formats (not read)

**Output guarantees:**
- Every utterance with extractable words has a `%mor` and `%gra` tier
- `%mor` word count equals main tier alignable word count
  (enforced by `validate_mor_alignment()`)
- All other tiers and headers are preserved unchanged
- File remains at its input validity level or higher (no new errors introduced
  in parts the command doesn't touch)

**Invariants:**
- Speaker codes are preserved exactly
- Utterance order is preserved exactly
- Terminators are preserved exactly
- Timing bullets are preserved exactly
- Annotations (retracing, repetition, overlap) are preserved exactly

---

### `utseg` — Utterance Segmentation

**Input:** CHAT file with main tier utterances (typically a single long utterance
per speaker turn from ASR output)

**Reads:**
- Main tier words (via `collect_utterance_content()` in `AlignmentDomain::Mor`)

**Writes:**
- Utterance structure: splits multi-word utterances into multiple utterances
  based on constituency-parse-derived sentence boundaries
- Each split utterance gets a period terminator (`.`)
- **All dependent tiers on split utterances are dropped** (fresh utterances
  have no `%mor`, `%gra`, `%wor`, etc.)

**Minimum input validity: Level 1**

Only needs structurally complete utterances with parseable words. Since all
dependent tiers are dropped during splitting, their validity is irrelevant.

**What may be invalid in the input:**
- All dependent tiers (will be dropped on split utterances)
- Timing bullets (dropped on split utterances)
- Word-level details beyond basic parseability (NLP operates on cleaned text)

**Output guarantees:**
- Every output utterance has a terminator (period)
- Single-word utterances are never split (passed through unchanged with all
  tiers preserved)
- Non-utterance lines (headers, comments) are preserved in original positions
- If inference produces an assignment that doesn't match the word count, the
  original utterance is preserved unchanged

**Invariants:**
- Speaker codes are preserved
- Header structure is preserved
- Utterance order is preserved (splits are in-place)

**Caveats:**
- Splitting destroys dependent tiers — `utseg` should typically be run
  *before* `morphotag` and `align`, not after
- Original terminators on split utterances are replaced with period (`.`)

---

### `translate` — Translation

**Input:** CHAT file with main tier utterances

**Reads:**
- Main tier words (space-joined into text for translation)

**Writes:**
- `%xtra` tier: translated text as a `UserDefined` dependent tier
- Replaces existing `%xtra` if present

**Minimum input validity: Level 1**

Only needs parseable main tier text. Word-level structural validity is not
critical since words are joined into a plain text string for the translation API.

**What may be invalid in the input:**
- All dependent tiers (untouched except `%xtra`)
- Timing bullets (untouched)
- Word-level markers (joined into text, model handles gracefully)

**Output guarantees:**
- Every utterance with extractable words has a `%xtra` tier (unless the
  translation is empty)
- All other tiers and headers are preserved unchanged

**Invariants:**
- Main tier content is never modified
- All non-`%xtra` dependent tiers are preserved
- Utterance order and structure preserved

---

### `coref` — Coreference Resolution

**Input:** CHAT file with main tier utterances (English only)

**Reads:**
- Main tier words from all utterances (document-level context)
- `@Languages` header (English-only gate)

**Writes:**
- `%xcoref` tier: bracket-notation coreference annotations (sparse — only
  utterances with actual chains)
- Replaces existing `%xcoref` if present

**Minimum input validity: Level 1**

Only needs parseable main tier text. Non-English files pass through unchanged.

**What may be invalid in the input:**
- All dependent tiers (untouched except `%xcoref`)
- Everything else (untouched)

**Output guarantees:**
- English files get `%xcoref` tiers on utterances with coreference chains
- Non-English files are returned unchanged
- All other content is preserved

**Invariants:**
- Main tier content is never modified
- Not cached (document-level context makes per-utterance caching meaningless)

---

### `align` — Forced Alignment

**Input:** CHAT file with main tier utterances + corresponding audio file

**Reads:**
- Main tier words (for transcript-to-audio alignment)
- Existing timing bullets on utterances (for audio window grouping)
- Audio file (resolved from same-stem sibling: `.wav`, `.mp3`, etc.)
- Audio duration via `ffprobe`
- `@Options: NoAlign` (to skip files that opt out)

**Writes:**
- Word-level timing bullets on `Word` nodes in the main tier
- Utterance-level timing bullet (first-word-start to last-word-end)
- `%wor` tier: regenerated from scratch (mirrors main tier words with
  individual timing bullets)

**Minimum input validity: Level 2**

The main tier must be structurally valid with correct word structure. Invalid
words produce incorrect alignment transcripts. Terminators must be present and
correct (the `to+...` bug demonstrated that terminator corruption propagates
through FA serialization). Existing timing bullets should be well-formed if
present (they're used for audio window grouping).

**Additional preconditions:**
- Audio file must exist and be accessible
- Audio file must be a known format (`.wav`, `.mp3`, `.mp4`)
- Audio file must be non-empty

**What may be invalid in the input:**
- `%mor`, `%gra` tiers (untouched)
- `%xtra`, `%xcoref` tiers (untouched)
- Existing `%wor` tier (will be regenerated)
- `@Date`, `@ID` field formats (not read)

**Output guarantees:**
- Every alignable word has a timing bullet (or `None` if alignment failed)
- Utterance-level bullets span first-to-last word timing
- `%wor` tier mirrors main tier words 1:1 with timing
- Temporal monotonicity enforced (`enforce_monotonicity()` strips timing from
  backwards utterances)
- Same-speaker self-overlap stripped (`strip_e704_same_speaker_overlaps()`)
- Untimed words get interpolated timing (proportional fill)

**Invariants:**
- Main tier word content is never modified (only timing added)
- Speaker codes preserved
- Annotations preserved
- Non-`%wor` dependent tiers preserved
- Utterance order preserved

---

### `transcribe` — ASR Transcription

**Input:** Audio file (NOT a CHAT file)

**Reads:**
- Audio file content

**Writes:**
- Creates an entirely new CHAT file from scratch:
  - `@UTF8`, `@Begin`, `@End` headers
  - `@Languages`, `@Participants`, `@ID` headers
  - `@Media` header referencing the audio file
  - Main tier utterances with speaker codes and timing bullets
  - `%wor` tiers with word-level timing
  - Utterance terminators

**Minimum input validity:** N/A (no CHAT input)

**Preconditions:**
- Audio file must exist, be non-empty, have a known extension
- For Rev.AI: valid API key configured

**Output guarantees:**
- Output is a complete, valid CHAT file
- Pre-serialization validation runs both alignment and semantic gates
- If validation fails, the file is not written and a bug report is filed

---

### `benchmark` — ASR Evaluation

**Input:** Audio file (NOT a CHAT file)

Same contract as `transcribe` plus evaluation metrics output.

---


**Input:** Audio file

**Reads:** Audio content only
**Writes:** JSON metrics (not CHAT)

No CHAT contract applies.

---


**Input:** Paired `.cs`/`.sv` audio files

**Reads:** Audio content only
**Writes:** JSON metrics (not CHAT)

No CHAT contract applies.

---

## Pre-Validation Gate Design

### Proposed Implementation

Each command declares its minimum validity level. Before dispatching to the
orchestrator, the runner validates the parsed CHAT file to that level:

```text
Runner receives file
  → parse_lenient() (always, for error recovery diagnostics)
  → check parse errors (Level 0)
  → if command needs Level 1+: check structural completeness
  → if command needs Level 2+: check main tier word validity
  → if preconditions fail: reject with diagnostics, skip file, continue job
  → if preconditions pass: dispatch to orchestrator
```

### Command → Validity Level Mapping

| Command | Min Level | Additional Preconditions |
|---------|-----------|--------------------------|
| `morphotag` | Level 2 | `@Languages` present |
| `utseg` | Level 1 | — |
| `translate` | Level 1 | — |
| `coref` | Level 1 | English language |
| `align` | Level 2 | Audio file exists |
| `transcribe` | N/A | Audio file exists |
| `benchmark` | N/A | Audio file exists |

### Rejection Behavior

When a file fails pre-validation:

1. The file is marked as `error` in the job status with category `"validation"`
2. The specific validation errors are reported (e.g., "E304: Missing terminator
   on line 15", "E233: Empty compound trailing part on line 22")
3. Processing continues with the next file in the job (partial job completion)
4. No compute is wasted on the invalid file
5. A bug report is filed if the errors suggest a parser/pipeline bug rather
   than input data quality

### Lenient vs Strict Parsing

We keep `parse_lenient()` as the parsing mode — it provides better error
recovery and diagnostics than `parse_strict()` (which just fails on first
error). The pre-validation gate inspects the parse errors and the resulting
AST to determine if the file meets the command's minimum validity level.

This is different from switching to `parse_strict()`: we parse leniently but
validate strictly against the command's requirements.

---

## Post-Processing Validation

### Current Gap

The Python pipeline runs `validate_output()` (alignment + semantic validation)
before serializing. The Rust server-side orchestrators do NOT — they only run
command-specific post-checks (e.g., `validate_mor_alignment` for morphotag).

### Proposed Fix

All server-side orchestrators should run a post-processing validation gate
before returning the serialized CHAT. The gate should check:

1. **Alignment validation**: tier word counts match (for commands that write
   dependent tiers)
2. **Temporal validation**: monotonic timestamps, no self-overlap (for commands
   that write timing)
3. **Structural validity**: the output file meets at least its input validity
   level (no degradation)

On failure: file the bug report, mark the file as error, return the original
input file unchanged (do not write corrupt output).

---

## Appendix: What Each Command Preserves

A command's **preservation set** is every part of the CHAT file it does not
modify. The pre-validation gate does NOT check the preservation set — those
parts can be invalid without affecting the command's operation.

| Command | Preservation Set |
|---------|-----------------|
| `morphotag` | Headers, timing, annotations, `%pho`/`%sin`/`%wor`/`%xtra`/`%xcoref`, `%com` |
| `utseg` | Headers, non-utterance lines (dependent tiers on split utterances are NOT preserved) |
| `translate` | Headers, main tier, timing, all tiers except `%xtra` |
| `coref` | Headers, main tier, timing, all tiers except `%xcoref` |
| `align` | Headers, main tier words/annotations, `%mor`/`%gra`/`%pho`/`%sin`/`%xtra`/`%xcoref` |
