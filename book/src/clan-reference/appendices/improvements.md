# CLAN Command Divergences & Improvements

**Status:** Current
**Last updated:** 2026-05-12 04:24 EDT

This document tracks every divergence between the original CLAN commands and
the Rust `chatter analyze` reimplementation. It covers CLI interface, output
format, counting semantics, error handling, and intentional improvements.

**Audience**: Researchers who have existing CLAN pipelines and need to know
what will change, and developers extending these commands.

**Backward compatibility**: Where CLAN's behavior is well-defined and
pipeline-critical, we aim to provide a `--clan-compat` mode in the future.
Until then, this document serves as the authoritative migration reference.

---

## Table of Contents

- [Philosophical Divergence: Semantic Model vs String Matching](#philosophical-divergence)
- [General Framework Divergences](#general-framework-divergences)
- [Analysis Commands](#analysis-commands) (29 commands)
- [Transform Commands](#transform-commands) (18 commands)
- [Format Converters](#format-converters) (12 commands)
- [Key Discoveries](#key-discoveries)

---

## Philosophical Divergence

The most fundamental difference between CLAN and `chatter analyze` is not in
any individual command, but in the underlying approach to analysis.

### CLAN: String Matching Without a Data Model

CLAN was written in the late 1970s before the CHAT format had a formal
specification. It operates directly on raw text strings, using character-level
prefix matching to classify content:

```c
// CLAN's approach to excluding non-lexical items (freq.c, mlu.c, etc.)
if (word[0] == '0') continue;     // skip omitted words
if (word[0] == '&') continue;     // skip fillers/nonwords
if (word[0] == '+') continue;     // skip terminators
if (word[0] == '-') continue;     // unclear intent
if (word[0] == '#') continue;     // skip pauses
if (strcmp(word, "xxx") == 0) continue;  // skip unintelligible
if (strcmp(word, "yyy") == 0) continue;  // skip phonetic coding
if (strcmp(word, "www") == 0) continue;  // skip untranscribed
```

This approach is duplicated across commands and conflates string
representation with semantic meaning. For example, `&-um` (a filler)
and `&~gaga` (a nonword) and `&+fr` (a phonological fragment) are all lumped
together because they share the `&` prefix, even though they have distinct
linguistic meanings.

### Our Approach: Semantic Classification via AST

Our parser produces a typed AST where each of these categories is a distinct
type. Analysis commands operate on semantic types, not string prefixes:

| CLAN text pattern | Semantic intent | Our AST representation | Analysis filtering |
|---|---|---|---|
| `word[0] == '#'` | Skip pauses | `Pause` (separate AST node) | Tree walk never visits |
| `word[0] == '+'` | Skip terminators | `Terminator` (separate AST level) | Tree walk never visits |
| `&=laughs` | Skip events | `Event` (separate AST node) | Tree walk never visits |
| `word == "xxx"` | Skip unintelligible | `Word { untranscribed: Unintelligible }` | `is_countable_word()` returns false |
| `word == "yyy"` | Skip phonetic coding | `Word { untranscribed: Phonetic }` | `is_countable_word()` returns false |
| `word == "www"` | Skip untranscribed | `Word { untranscribed: Untranscribed }` | `is_countable_word()` returns false |
| `word[0] == '0'` | Skip omitted words | `Word { category: Omission }` | `is_countable_word()` returns false |
| `word[0] == '&'` | Skip fillers/nonwords | `Word { category: Filler\|Nonword\|Fragment }` | `is_countable_word()` returns false |
| `word[0] == '-'` | (unclear) | Not a meaningful CHAT category | Not applicable |

**Key insight**: Pauses, events, actions, and terminators are already separate
AST node types that our tree walk never visits. We only need explicit filtering
on `Word` nodes that carry semantic annotations. The `is_countable_word()`
function in `framework/word_filter.rs` encodes this logic once, and all
analysis commands share it.

### What This Means for Compatibility

The *results* should be equivalent: words that CLAN excludes via string prefix
matching, we exclude via semantic type checking. But the mechanism is
fundamentally different:

- CLAN can be tricked by unusual formatting (e.g., a real word that happens
  to start with `&` due to a transcription error) — our AST only marks words
  as fillers if the parser recognized the `&-` prefix *and* classified them
  as such during parsing.
- CLAN's `word[0] == '-'` exclusion has unclear intent and may exclude
  legitimate content — we omit this because there is no corresponding
  semantic category in CHAT.
- CA (Conversation Analysis) omissions `((word))` are countable in our system
  (they represent uncertain but present speech), matching the linguistic intent
  even though CLAN may exclude them via the `0` prefix check.

---

## General Framework Divergences

### CLI Syntax

Both legacy CLAN `+flag`/`-flag` and modern `--flag` syntax are accepted.
The CLI auto-translates via `clan_args::rewrite_clan_args()`.

| Aspect | CLAN | `chatter clan` |
|--------|------|----------------|
| Invocation | `freq file.cha` | `chatter clan freq file.cha` |
| Speaker include | `+t*CHI` | `--speaker CHI` |
| Speaker exclude | `-t*INV` | `--exclude-speaker INV` |
| Tier include | `+t%mor` | `--tier mor` |
| Tier exclude | `-t%gra` | `--exclude-tier gra` |
| Word include | `+s"word"` | `--include-word "word"` |
| Word exclude | `-s"word"` | `--exclude-word "word"` |
| Gem include | `+g"Story"` | `--gem "Story"` |
| Gem exclude | `-g"Warmup"` | `--exclude-gem "Warmup"` |
| ID filter | `+t@ID="..."` | `--id-filter "..."` |
| Utterance range | `+z25-125` | `--range 25-125` |
| Include retracings | `+r6` | `--include-retracings` |
| Case-sensitive | `+k` | `--case-sensitive` |
| Display mode | `+dN` | `--display-mode N` |
| Output to file | `+fEXT` | `--output-ext EXT` |
| Context window | `+wN` / `-wN` | `--context-after N` / `--context-before N` |
| Merge across files | `+u` | Default behavior (always on) |
| Output format | Text only | `--format text\|json\|csv\|clan` |
| Multiple files | `freq *.cha` (glob) | `chatter clan freq dir/` (recursive) |

### Output Envelope

CLAN wraps output in a verbose envelope (command echo, timestamp, filter
summary). `chatter clan` uses `render_clan()` for CLAN-compatible output
format and `render_text()` for clean output. The envelope is omitted in
both modes — the information is available via shell history and `date`.

### Error Handling

| Behavior | CLAN | `chatter clan` |
|----------|------|----------------|
| Unparseable file | Prints error, continues | Logs warning (via `tracing`), continues |
| No `.cha` files found | Varies by command | Prints error, exits with code 1 |
| Missing %mor tier | Skips silently | Reports 0 utterances (MLU), skips (FREQ `--mor`) |

### Multi-File Aggregation

| Behavior | CLAN | `chatter clan` |
|----------|------|----------------|
| Default | Per-file output | Merged across files |
| Merged | Requires `+u` flag | Default behavior |

### File Output

CLAN's `+f` flag writes output to auto-named files (e.g., `sample.frq.cex`).
`chatter analyze` always writes to stdout. Use shell redirection:

```bash
# CLAN:
freq +f sample.cha              # -> sample.frq.cex

# chatter:
chatter analyze freq sample.cha > sample.frq.txt
chatter analyze freq sample.cha --format json > sample.frq.json
```

---

## Analysis Commands

### Clinically Critical Commands

#### FREQ — Word Frequency

- AST-based `is_countable_word()` replaces string-prefix matching
- `NormalizedWord` lowercases and strips compound markers for grouping
- Deterministic sort (count descending, then alphabetical); CLAN's order varies
- JSON and CSV output; `--format clan` for character-level CLAN compatibility
- Golden test parity: verified

#### MLU — Mean Length of Utterance

- **Population SD** (/ n), not sample (/ n-1). Verified against CLAN output.
- **Brown's morpheme rules**: Only 7 suffix strings count: `PL`, `PAST`, `Past`, `POSS`, `PASTP`, `Pastp`, `PRESP`. Each adds +1 to the stem count. Fusional features (`&PRES`, `&INF`) do NOT count.
- When no `%mor` tier exists and not in `--words-only` mode, reports 0 utterances (matching CLAN).
- Golden test parity: 100%

#### MLT — Mean Length of Turn

- **Population SD** (/ n), matching CLAN.
- **SD basis**: Computed over per-utterance word counts, not per-turn totals.
- Turn boundaries detected when a different speaker produces the next utterance.
- Golden test parity: 100%

#### DSS — Developmental Sentence Scoring

- Built-in rules are a simplified subset; supply full `.scr` file for clinical scoring.
- Sentence-point assignment uses heuristic (subject + verb POS) rather than full syntax.
- Up to 50 utterances per speaker scored (configurable via `max_utterances`).
- Golden test parity: verified

#### EVAL — Language Sample Evaluation

- AST-based word/morpheme identification and typed POS categories.
- Error counts (`[*]`) extracted from parsed AST annotations, not text patterns.
- JSON and CSV output (CLAN produces text only).
- Golden test parity: verified

#### KIDEVAL — Child Language Evaluation

- Same AST-based approach as EVAL with age-normed scoring.
- Shares the semantic POS classification and error extraction infrastructure.
- Golden test parity: verified

#### IPSYN — Index of Productive Syntax

- Parses %mor tier structure for syntactic pattern matching.
- The built-in scoring covers a **representative subset** of the ~56 grammatical categories in the full English IPSyn rule set (see `crates/clan/clan-core/src/commands/ipsyn.rs`). Full coverage requires supplying an external rule file.
- Golden test parity: verified for the built-in subset

#### VOCD — Vocabulary Diversity (D Statistic)

- Bootstrap sampling of TTR across sample sizes 35-50, least-squares D-curve fitting.
- **Fusional feature stripping**: `&PRES`, `&INF` etc. stripped from lemmas in %mor echo output.
- Echoes %mor lemma lines after speaker header for insufficient-token warnings.
- D values may differ slightly due to random sampling (stochastic algorithm).
- Golden test parity: 100%

#### FLUCALC — Fluency Calculation

- Counts disfluency types (repetitions, revisions, fillers) from main tier annotations.
- Operates on parsed retrace groups and filler categories, not text patterns.
- Golden test parity: verified

#### SUGAR — Grammatical Analysis

- SUGAR scoring from %mor tier POS categories.
- AST-based morpheme and word classification.
- Golden test parity: verified

### Other Analysis Commands

#### CHAINS — Clause Chain Analysis

- Walks parsed %gra tier dependency structure instead of text-based bracket matching.
- Clause boundaries identified from dependency relations, not punctuation heuristics.
- Golden test parity: verified

#### CHIP — Child/Parent Interaction Profile

- **36-measure matrix format** matching CLAN exactly (ADU/CHI/ASR/CSR columns).
- **Echo**: Main tier + %mor only (not %gra tiers), matching CLAN.
- Interaction scoring (imitation, expansion, reduction) computed from %mor tier alignment.
- Golden test parity: 100%

#### CODES — Code Frequency

- Counts codes from `%cod` tier annotations using AST structure.
- Golden test parity: verified

#### COMBO — Boolean Search

- Operates on parsed AST content rather than raw text pattern matching.
- Boolean AND/OR/NOT composition on word-level matches.
- Golden test parity: verified

#### COOCCUR — Word Co-occurrence

- Bigram counting from countable words per utterance.
- Uses `NormalizedWord` for consistent key normalization.
- Golden test parity: verified

#### DIST — Word Distribution Across Turns

- **Every utterance is its own turn** (no speaker-continuity grouping), matching CLAN.
- Per-speaker distribution of words across turn positions.
- Golden test parity: 100%

#### FREQPOS — Positional Frequency

- Word frequency by utterance position (initial, final, other, one-word).
- AST-based word identification and position tracking.
- Golden test parity: verified

#### GEMLIST — Gem Segments

- Lists `@Bg`/`@Eg` gem boundaries from file headers.
- Golden test parity: verified

#### KEYMAP — Contingency Tables

- Reads coded data from `%cod` tier, builds contingency matrix.
- Golden test parity: verified

#### MAXWD — Longest Words

- Reports **all occurrences with line numbers**, matching CLAN.
- Golden test parity: 100%

#### MODREP — Model/Replica Comparison

- Compares `%mod` (model) and `%pho` (replica) tiers phonologically.
- AST-based phonological comparison rather than string distance.
- Golden test parity: verified

#### MORTABLE — Morphology Tables

- Tabulates POS categories and morphological features from %mor tier.
- Golden test parity: verified

#### PHONFREQ — Phonological Frequency

- Frequency counts from `%pho` tier phonological transcription.
- Golden test parity: verified

#### RELY — Inter-rater Agreement

- Cohen's kappa for inter-rater reliability between two transcription files.
- Golden test parity: verified

#### SCRIPT — Template Comparison

- Compares transcript against template script, computes accuracy metrics.
- Golden test parity: verified

#### TIMEDUR — Time Duration

- Computes duration from media timestamp bullets.
- **Interaction matrix header** includes leading space, matching CLAN exactly.
- Golden test parity: 100%

#### TRNFIX — Tier Comparison

- Compares two dependent tiers side-by-side.
- Uses `∅` for length mismatches between compared tiers.
- Golden test parity: verified

#### UNIQ — Repeated Utterances

- **Includes %mor/%gra dependent tiers** in counts, matching CLAN.
- **Splits multi-line headers** for counting, matching CLAN.
- **1 accepted divergence**: Unicode sort order for `U+230A` (LEFT FLOOR) — C-locale `strcoll()` vs Rust byte-order. Single line position swap, identical content and counts.
- Golden test parity: 99%

#### WDLEN — Word Length Distribution

- **6-section format**: characters (1), words (2), morphemes (3), per-utterance (4), per-word morphemes (5), per-word with POS (6).
- **Brown's morpheme rules**: Section 5 = stem + Brown's suffix (no POS). Section 6 = POS + stem + Brown's suffix.
- **Clitic handling**: Section 5 merges main+clitics as one word. Section 6 counts POS only for main word.
- **Apostrophe stripping**: Characters counted after removing apostrophes, matching CLAN.
- **Reverse speaker order**: CLAN's linked-list prepend pattern replicated.
- **XML footer**: `</Table></Worksheet></Workbook>` appended to match CLAN output.
- Golden test parity: 100%

---

## Transform Commands

All transforms use the AST pipeline: parse → transform → serialize → write.

#### CHSTRING — String Replacement

- Operates on parsed AST word nodes, not raw text. Structural integrity preserved.
- Does not support CLAN's regex-based patterns in changes files.

#### COMBTIER — Combine Tiers

- Merges multiple dependent tiers of the same type via AST manipulation.

#### COMPOUND — Compound Normalization

- Normalizes compound word formatting (dash to plus) in the AST.

#### DATACLEAN — Format Cleanup

- Fixes common CHAT formatting errors (spacing, brackets) in serialized text.
- Operates on serialized output rather than AST (custom run function).

#### DATES — Age Computation

- Computes participant ages from `@Birth` and `@Date` headers.
- Uses Rust `chrono` date arithmetic rather than C-style manual calculation.

#### DELIM — Add Terminators

- Sets the `Terminator` field on the AST, not scanning line endings.
- **4 accepted divergences**: CLAN writes empty file when no changes needed; we always write the full file.

#### FIXBULLETS — Timing Repair

- Enforces monotonic timing bullet ordering via AST timestamp fields.

#### FLO — Fluent Output

- Walks AST nodes (retrace groups, replaced words, events, pauses) instead of regex-stripping.
- Uses shared `is_countable_word()` for filtering.

#### GEM — Gem Extraction

- Extracts utterances within `@Bg`/`@Eg` boundaries.
- Also available as a filter on analysis commands.

#### LINES — Line Numbers

- Operates on serialized text (custom run function).

#### LOWCASE — Lowercase

- Lowercases word content in AST nodes, preserving annotations.

#### MAKEMOD — Model Tier

- Generates `%mod` tier from pronunciation lexicon lookup.

#### ORT — Orthographic Conversion

- Dictionary-based orthographic conversion via AST word replacement.

#### POSTMORTEM — Mor Post-processing

- Pattern-matching rules applied to `%mor` tier AST structure.
- Rule application is structural (MorWord fields) rather than text-based.

#### QUOTES — Extract Quotes

- Operates on serialized text (custom run function).

#### REPEAT — Mark Repetitions

- Marks utterances containing revisions with `[+ rep]` postcode.
- Detection via parsed retrace group annotations.

#### RETRACE — Retrace Tier

- Generates retrace annotation tier from AST structure.

#### TIERORDER — Reorder Tiers

- Reorders dependent tiers by priority (standard CHAT ordering).
- Operates on `DependentTier` vector in the AST.

---

## Format Converters

All converters build/consume `ChatFile` AST structures, not raw text.

#### CHAT2ELAN — CHAT to ELAN

- Produces EAF 3.0 XML with time-aligned annotation tiers.
- CLAN equivalent: separate `chat2elan` binary.

#### CHAT2SRT — CHAT to Subtitles

- Strips CHAT annotations by walking AST content variants.
- Supports both SRT and WebVTT output formats (CLAN: SRT only).

#### ELAN2CHAT — ELAN to CHAT

- ELAN tier IDs mapped to 3-character uppercased speaker codes.
- Simple string-based XML parsing (no `quick-xml` dependency).

#### LAB2CHAT — LAB to CHAT

- Timing label files from speech research converted to timed CHAT utterances.

#### LENA2CHAT — LENA to CHAT

- LENA device XML (`.its`) import with segment timing.
- Maps LENA speaker categories to CHAT participant codes.

#### LIPP2CHAT — LIPP to CHAT

- LIPP phonetic profile import with `%pho` tier generation.

#### PLAY2CHAT — PLAY to CHAT

- PLAY annotation format import.

#### PRAAT2CHAT — Praat TextGrid

- **Bidirectional**: TextGrid → CHAT and CHAT → TextGrid in one module.
- Supports both long and short TextGrid formats.
- CLAN has separate commands for each direction.

#### RTF2CHAT — Rich Text to CHAT

- Rich Text Format import with formatting strip.

#### SALT2CHAT — SALT to CHAT

- SALT transcription format import with speaker/utterance mapping.
- Handles SALT-specific conventions (utterance boundaries, codes).

#### SRT2CHAT — Subtitles to CHAT

- SRT subtitle import with timing bullet generation.
- Handles subtitle numbering, multi-line entries, and timestamp parsing.

#### TEXT2CHAT — Plain Text to CHAT

- Plain text import with sentence splitting heuristics.
- Generates minimal CHAT headers (`@Begin`, `@Participants`, `@End`).

---

## Key Discoveries

These findings were established during parity verification (golden tests
comparing against CLAN C binaries):

1. **Brown's Morpheme Rules**: CLAN MLU/WDLEN count only 7 suffix strings as
   bound morphemes: `PL`, `PAST`, `Past`, `POSS`, `PASTP`, `Pastp`, `PRESP`.

2. **Population SD**: Both MLU and MLT use population SD (/ n), not sample (/ n-1).

3. **MLT SD basis**: Computed over per-utterance word counts, not per-turn totals.

4. **DIST turns**: Every utterance is its own turn (no speaker-continuity grouping).

5. **WDLEN morphemes**: Section 5 = stem + Brown's suffix (no POS). Section 6 =
   POS + stem + Brown's suffix. Clitic pairs = one word in section 5, POS counted
   only for main word in section 6.

6. **WDLEN characters**: CLAN strips apostrophes before counting character length.

7. **Speaker ordering**: CLAN outputs speakers in reverse encounter order (linked-list
   prepend pattern from the C implementation).

8. **Fusional features**: `&PRES`, `&INF` etc. are stored as part of the lemma string
   in our model (for roundtrip fidelity). Strip with `split('&')` when needed.

9. **CHIP echo**: CLAN echoes main tiers + %mor only, not %gra tiers.

---

## Parity Status

**95% (113/118)** — 5 accepted divergences across 2 commands.

| Command | Divergences | Status |
|---------|-------------|--------|
| DELIM | 4 | Accepted: CLAN writes empty file when no changes needed |
| UNIQ | 1 | Accepted: Unicode sort order edge case (U+230A) |

All other commands: 100% parity with CLAN C binary output in golden tests.

---

*Last Updated: 2026-03-05*
*Based on CLAN manual (January 27, 2026 edition) and golden test verification*
