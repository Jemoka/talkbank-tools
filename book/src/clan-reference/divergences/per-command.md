# Per-Command Divergences

**Status:** Current
This page documents every known divergence between the Rust `chatter clan` commands and the original CLAN C binaries. Divergences are verified through golden tests that compare output character-by-character.

**Overall parity: 95% (113/118) -- 5 accepted divergences across 2 commands.**

## Parity Summary

| Status | Commands |
|--------|----------|
| **100% parity** | FREQ, MLU, MLT, VOCD, CHIP, DIST, MAXWD, TIMEDUR, WDLEN |
| **Verified** | DSS, EVAL, KIDEVAL, IPSYN, FLUCALC, SUGAR, CHAINS, CODES, COMBO, COOCCUR, FREQPOS, GEMLIST, KEYMAP, MODREP, MORTABLE, PHONFREQ, RELY, SCRIPT, TRNFIX, CHSTRING, FLO, POSTMORTEM |
| **Accepted divergences** | DELIM (4), UNIQ (1) |

"Verified" means golden tests pass but character-level parity has not been exhaustively confirmed across all edge cases.

---

## Analysis Commands

### FREQ -- Word Frequency

- AST-based `is_countable_word()` replaces string-prefix matching
- The [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) explicitly
  says `FREQ` ignores `xxx`, `www`, and items beginning with `0`, `&`, `+`,
  `-`, or `#` by default; `clan-core` preserves that intent through typed
  word classes rather than raw prefix checks.
- `NormalizedWord` lowercases and strips compound markers for grouping
- Deterministic sort (count descending, then alphabetical); CLAN's order varies
- JSON and CSV output; `--format clan` for character-level CLAN compatibility
- **Parity: 100%**

### MLU -- Mean Length of Utterance

- **Population SD** (/ n), not sample (/ n-1). Verified against CLAN output.
- **Brown's morpheme rules**: Only 7 suffix strings count: `PL`, `PAST`, `Past`, `POSS`, `PASTP`, `Pastp`, `PRESP`. Each adds +1 to the stem count. Fusional features (`&PRES`, `&INF`) do NOT count.
- When no `%mor` tier exists and not in `--words-only` mode, reports 0 utterances (matching CLAN).
- **Parity: 100%**

### MLT -- Mean Length of Turn

- **Population SD** (/ n), matching CLAN.
- **SD basis**: Computed over per-utterance word counts, not per-turn totals.
- Turn boundaries detected when a different speaker produces the next utterance.
- **Parity: 100%**

### DSS -- Developmental Sentence Scoring

- Built-in rules are a simplified subset; supply full `.scr` file for clinical scoring.
- Sentence-point assignment uses heuristic (subject + verb POS) rather than full syntax.
- Up to 50 utterances per speaker scored (configurable via `max_utterances`).
- **Parity: Verified**

### EVAL -- Language Sample Evaluation

- AST-based word/morpheme identification and typed POS categories.
- Error counts (`[*]`) extracted from parsed AST annotations, not text patterns.
- **Parity: Verified**

### KIDEVAL -- Child Language Evaluation

- Same AST-based approach as EVAL with combined metrics.
- VOCD uses simplified TTR-based D estimate in the combined report.
- **Parity: Verified**

### IPSYN -- Index of Productive Syntax

- Parses %mor tier structure for syntactic pattern matching.
- Built-in rule set is a simplified subset; supply rules file for full coverage.
- **Parity: Verified**

### VOCD -- Vocabulary Diversity (D Statistic)

- Bootstrap sampling of TTR across sample sizes 35-50, least-squares D-curve fitting.
- **Fusional feature stripping**: `&PRES`, `&INF` etc. stripped from lemmas in %mor echo output.
- D values may differ slightly due to random sampling (stochastic algorithm).
- **Parity: 100%** (within expected stochastic variation)

### FLUCALC -- Fluency Calculation

- Counts disfluency types from main tier annotations.
- Some categories detected via text pattern matching rather than full AST traversal.
- **Parity: Verified**

### SUGAR -- Grammatical Analysis

- SUGAR scoring from %mor tier POS categories.
- `%mor` post-clitics count as structured morphology, so clitic-bearing chunks contribute to morpheme totals and verb detection.
- Minimum utterance threshold is configurable (CLAN uses fixed value).
- **Parity: Verified**

### CHAINS -- Clause Chain Analysis

- `CHAINS` consumes a clan-local semantic `%cod` item layer, so selectors like `<w4>` scope codes instead of being treated as codes themselves.
- Uses sample SD (N-1), not population SD.
- **Parity: Verified**

### CHIP -- Child/Parent Interaction Profile

- **36-measure matrix format** matching CLAN exactly (ADU/CHI/ASR/CSR columns).
- **Echo**: Main tier + %mor only (not %gra tiers), matching CLAN.
- **Parity: 100%**

### CODES -- Code Frequency

- Codes extracted from parsed `%cod` tier, not raw text.
- `%cod` is interpreted through a clan-local semantic item layer derived from the AST. Optional selectors like `<w4>` or `<w4-5>` scope the next code item instead of being counted as codes themselves.
- **Parity: Verified**

### COMBO -- Boolean Search

- AST-based content matching rather than raw text pattern matching.
- Operator syntax: `+` for AND, `,` for OR (CLAN uses `^` and `|`).
- **Parity: Verified**

### COOCCUR -- Word Co-occurrence

- Bigram counting from countable words per utterance.
- **Parity: Verified**

### DIST -- Word Distribution

- **Every utterance is its own turn** (no speaker-continuity grouping), matching CLAN.
- **Parity: 100%**

### FREQPOS -- Positional Frequency

- Word frequency by utterance position (initial, final, other, one-word).
- **Parity: Verified**

### GEMLIST -- Gem Segments

- Lists `@Bg`/`@Eg` gem boundaries from file headers.
- **Parity: Verified**

### KEYMAP -- Contingency Tables

- Reads coded data from `%cod` tier, builds contingency matrix.
- `%cod` selector tokens are treated as item scope rather than keyword/following-code values; `KEYMAP` consumes semantic `%cod` items.
- **Parity: Verified**

### MAXWD -- Longest Words

- Reports **all occurrences with line numbers**, matching CLAN.
- **Parity: 100%**

### MODREP -- Model/Replica Comparison

- Compares `%mod` and `%pho` tiers phonologically via AST.
- **Parity: Verified**

### MORTABLE -- Morphology Tables

- Tabulates POS categories from %mor tier using script files.
- POS extraction reads typed `%mor` items directly instead of reparsing serialized `%mor` payload text.
- **Parity: Verified**

### KWAL -- Keyword and Line

- The
  [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) does document
  `KWAL`, including cluster-based search semantics.
- **Scope reduction**: The legacy manual describes richer cluster/tier selection and `%mor`/`%gra` combined searches than the current implementation exposes.
- **Parity: Verified**

### PHONFREQ -- Phonological Frequency

- Frequency counts from `%pho` tier.
- **Parity: Verified**

### RELY -- Inter-rater Agreement

- Cohen's kappa for inter-rater reliability.
- `%cod` comparison uses semantic code items from the AST-derived layer rather than whitespace tokens from flattened tier text.
- **Parity: Verified**

### SCRIPT -- Template Comparison

- Word matching uses frequency maps rather than positional alignment.
- **Parity: Verified**

### TIMEDUR -- Time Duration

- **Interaction matrix header** includes leading space, matching CLAN exactly.
- **Parity: 100%**

### TRNFIX -- Tier Comparison

- Uses `∅` for length mismatches between compared tiers.
- `%trn` is treated as a structural alias of `%mor`; `%grt` is treated as a structural alias
  of `%gra`.
- `%mor`/`%gra` comparison preserves typed token boundaries directly from the AST instead of comparing whitespace-split serialized payloads.
- **Parity: Verified**

### UNIQ -- Repeated Utterances

- **Includes %mor/%gra dependent tiers** in counts, matching CLAN.
- **Splits multi-line headers** for counting, matching CLAN.
- **1 accepted divergence**: Unicode sort order for `U+230A` (LEFT FLOOR) -- C-locale `strcoll()` vs Rust byte-order. Single line position swap, identical content and counts.
- **Parity: 99%**

### WDLEN -- Word Length Distribution

- **6-section format** matching CLAN exactly.
- **Brown's morpheme rules**: Section 5 = stem + Brown's suffix (no POS). Section 6 = POS + stem + Brown's suffix.
- **Clitic handling**: Section 5 merges main+clitics as one word. Section 6 counts POS only for main word.
- **Apostrophe stripping**: Characters counted after removing apostrophes.
- **Reverse speaker order**: CLAN's linked-list prepend pattern replicated.
- **XML footer**: `</Table></Worksheet></Workbook>` appended.
- **Parity: 100%**

---

## Transform Commands

All transforms use the AST pipeline: parse -> transform -> serialize -> write.

### DELIM -- Add Terminators

- **4 accepted divergences**: CLAN writes empty file when no changes needed; we always write the full file.
- **Parity: 4 accepted divergences**

### CHSTRING -- String Replacement

- Does not support CLAN's regex-based patterns in changes files.
- **Parity: Verified**

### FLO -- Fluent Output

- Walks AST nodes instead of regex-stripping annotation markers.
- **Parity: Verified**

### POSTMORTEM -- Mor Post-processing

- Typed `%mor` rewrites are intentionally rejected until an AST-native rewrite path exists. `POSTMORTEM` errors explicitly when a rule would modify parsed `%mor`.
- **Current status:** user-defined text tiers remain supported rewrite targets; typed `%mor` rewrite is intentionally unsupported until implemented through the AST.

### Other transforms

- `COMBTIER` preserves bullet/text tier variants such as `%cod` and `%com` instead of degrading them to user-defined tiers.
- `FIXBULLETS` supports global offsets and tier-scoped repair across parsed main-tier, `%wor`, and bullet-content-tier bullets.
- Bullet-bearing `@Comment` headers are parsed structurally, so `FIXBULLETS` can offset those header bullets through the AST as well.
- **Scope reduction remains:** old-format bullet conversion, `@Media` insertion, multi-bullet merge, and `+l` remain unsupported.
- **Scope reduction:** `TIERORDER` currently uses a built-in tier ordering,
  while the
  [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) documents
  `tierorder.cut` as a user-controlled ordering source.
- `TRIM` follows the
  [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) intent by
  removing selected dependent tiers instead of extracting utterance ranges or
  gem segments.
COMBTIER, COMPOUND, DATACLEAN, DATES, FIXBULLETS, LINES, LOWCASE, MAKEMOD, ORT, QUOTES, REPEAT, RETRACE, TIERORDER -- all operate on AST rather than raw text, except for the intentionally text-level formatting transforms (`DATACLEAN`, `LINES`) and layout transforms (`INDENT`, `LONGTIER`) discussed in the dependent-tier semantics audit.

---

## Key Discoveries

These findings were established during parity verification (golden tests comparing against CLAN C binaries):

1. **Brown's Morpheme Rules**: CLAN MLU/WDLEN count only 7 suffix strings as bound morphemes.
2. **Population SD**: Both MLU and MLT use population SD (/ n), not sample (/ n-1).
3. **MLT SD basis**: Computed over per-utterance word counts, not per-turn totals.
4. **DIST turns**: Every utterance is its own turn (no speaker-continuity grouping).
5. **Speaker ordering**: CLAN outputs speakers in reverse encounter order (linked-list prepend).
6. **Fusional features**: `&PRES`, `&INF` etc. are part of the lemma string; strip with `split('&')`.
7. **CHIP echo**: CLAN echoes main tiers + %mor only, not %gra tiers.
8. **WDLEN characters**: CLAN strips apostrophes before counting character length.
