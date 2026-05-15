# Reference Corpus Overhaul

**Status:** Reference
**Last updated:** 2026-05-01 09:47 EDT

## Motivation

The reference corpus (`resources/corpus/reference/`) is the 100%-pass quality gate for all
parser/grammar changes. The parser must handle every file at 100%.
Before this overhaul, the corpus had three problems:

1. **Language monoculture**: 345 files, all English. We have 100K+ real files
   across 42 languages in the corpus data directory but the gate only tested English.
2. **Construct gaps**: 18 concrete grammar node types were never exercised
   (e.g., `interrupted_question`, `scoped_best_guess`, `trailing_off_question`).
   A grammar regression affecting these constructs would pass CI undetected.
3. **Error coverage gaps**: 27 error specs were stubs (no CHAT example), 4 error
   codes had no spec file at all.

## Strategy

**Fresh build, not incremental patching.** We kept the existing 345 English files
as-is (they encode years of parser fixes) and added multilingual files +
construct gap-fillers on top.

### Phase 0: Coverage Tooling

Built `corpus_node_coverage` (`crates/spec/talkbank-spec-testgen/src/bin/corpus_node_coverage.rs`) to
measure which of the 334 concrete grammar node types the corpus exercises.
Running against the old 345-file corpus confirmed exactly 18 gaps.

### Phase 1: Language Selection & File Extraction

Built `extract_corpus_candidates` (`crates/spec/talkbank-spec-testrun/src/bin/extract_corpus_candidates.rs`)
to automatically select representative files from the corpus data directory for 20 target
languages:

```text
eng, zho, fra, deu, spa, jpn, nld, heb, por, ell,
tur, hrv, pol, ita, hun, rus, est, dan, ara, isl
```

Selection criteria:
- Clean tree-sitter parsing (no ERROR nodes) — mandatory
- Short files (under 200 lines, preferring 15–100)
- Varied tiers (%mor/%gra/%pho/%com)
- Multiple speakers preferred
- Privacy: explicitly skip `Password` directories in the corpus data directory

For each language, the tool scored and ranked candidates. We selected 1–2 files
per language (25 files total across 20 language subdirectories).

### Phase 2: Construct Gap-Filling

Created 4 handcrafted files in `resources/corpus/reference/constructs/` to exercise the 18
missing node types that don't appear in real-world data:

| File | Node types exercised |
|------|---------------------|
| `rare-terminators.cha` | `interrupted_question`, `self_interrupted_question`, `self_interruption`, `trailing_off_question` |
| `uptake.cha` | `uptake_symbol` |
| `best-guess.cha` | `scoped_best_guess` |
| `unsupported.cha` | `thumbnail_header`, `unsupported_header`, `unsupported_dependent_tier`, `unsupported_line`, `unsupported_header_prefix`, `unsupported_tier_prefix` |

Other gaps (`l1_of_header`, `utf8_header`, etc.) were already covered by the
language files or were confirmed as supertypes (not concrete).

**Result: 334/334 concrete types exercised (100%).**

### Phase 3: Tier Regeneration

Ran batchalign3 morphotag on all 25 language files to generate fresh %mor/%gra
tiers:

```bash
cd /path/to/batchalign3
uv run batchalign3 morphotag /path/to/talkbank-tools/resources/corpus/reference/{lang}/ --in-place
```

All 20 languages are covered by Stanza's UD models. Validation confirmed all
374 files pass parser equivalence and roundtrip.

### Phase 4: Error Corpus Expansion

**4.1: Created 3 missing error specs** (E707, E711, E717) with CHAT examples and
metadata. Fixed E376 (had wrong error code E208 in metadata).

**4.2: Filled 17 triggerable stub specs** with CHAT examples:
- Cross-utterance validation (E341, E351–E355)
- Parser recovery warnings (E319–E322, E325, E326)
- Underline tier errors (E356–E357)
- Overlap index errors (E373)
- Direct parser tier errors (E381, E384)

**4.3: Documented 12 untriggerable stubs** (internal, deprecated, or not-yet-wired
error codes) with explanations of why no example is possible: E001, E002, E211,
E317, E318, E340, E374, E377, E378, E380, E385, E386.

**4.4: Corrected 5 misclassified specs** where examples triggered different error
codes than intended (E319–E322, E376). Added `Status: not_implemented` and
explanatory notes.

**4.5: Built perturbation tool** (`crates/spec/talkbank-spec-testgen/src/bin/perturb_corpus.rs`) with 11
mutation strategies that take a valid `.cha` file and produce controlled mutations
targeting specific error codes:

| Perturbation | Target Error |
|-------------|-------------|
| `delete-participants` | E501 |
| `delete-languages` | E503 |
| `delete-id` | E504 |
| `undeclared-speaker` | E308 |
| `delete-terminator` | E305 |
| `extra-mor-word` | E706 |
| `fewer-mor-words` | E705 |
| `delete-begin` | E502 |
| `delete-end` | E510 |
| `duplicate-participants` | E511 |
| `mor-terminator-mismatch` | E716 |

Also includes a mining mode (`--mine DIR`) that scans real data for tree-sitter
ERROR nodes, with automatic `Password` directory exclusion.

**4.6: Regenerated golden artifacts** — all 8 golden generators + audit + bootstrap:

| Artifact | Lines |
|----------|-------|
| `golden_words.txt` | 769 (1949 unique words) |
| `golden_mor_tiers.txt` | 405 |
| `golden_gra_tiers.txt` | 7 |
| `golden_main_tiers.txt` | 607 |
| `golden_pho_tiers.txt` | 25 |
| `golden_wor_tiers.txt` | 7 |
| `golden_sin_tiers.txt` | 5 |
| `golden_com_tiers.txt` | 24 |
| `golden_words_featured.txt` | 96 |
| `golden_words_minimal.txt` | 62 |

Bootstrap regenerated `reference_corpus.rs` with 374 test cases.

### Phase 5: CI Integration & Validation

All verification gates pass:
- Parser equivalence: 377/377 (374 files + 3 extra)
- Node coverage: 334/334 (100%)
- Error coverage: 181/181 (100%), 169 with CHAT examples, 12 documented stubs
- `bazel build //... && bazel test //...` passes all gates (G0–G10)

### Phase 6: Cleanup & Documentation

- Updated file count references (339→374) across CLAUDE.md files
- Rewrote `corpus/README.md` with new structure
- Updated memory files

## Final State

```text
resources/corpus/reference/           374 files total
  *.cha                     345 files (original English corpus)
  constructs/                 4 files (rare grammar constructs)
  {20 language dirs}/        25 files (multilingual, from corpus data)
```

| Metric | Before | After |
|--------|--------|-------|
| Total files | 345 | 374 |
| Languages | 1 (English) | 20 |
| Concrete node coverage | 316/334 (94.6%) | 334/334 (100%) |
| Error specs | 177/181 (97.8%) | 181/181 (100%) |
| Error specs with examples | ~150 | 169 |
| Documented stubs | 0 | 12 |
| Golden artifacts | Stale | Freshly regenerated |

## Tools Built

| Tool | Path | Purpose |
|------|------|---------|
| `corpus_node_coverage` | `crates/spec/talkbank-spec-testgen/src/bin/` | Grammar node type coverage |
| `extract_corpus_candidates` | `crates/spec/talkbank-spec-testrun/src/bin/` | Automated file selection from corpus data |
| `perturb_corpus` | `crates/spec/talkbank-spec-testgen/src/bin/` | Error file generation by mutation |

## What Worked

- **extract_corpus_candidates**: Automated scoring eliminated guesswork in file
  selection. Files were high-quality, short, and diverse.
- **construct gap-filling**: 4 handcrafted files closed 18 gaps efficiently.
- **Keeping existing 345 files**: No breakage, no regressions. The new files are
  purely additive.
- **batchalign3 morphotag**: Generated correct %mor/%gra for all 20 languages
  without manual intervention.

## What Didn't Work / Lessons Learned

- **Mining real errors from corpus data**: The MacWhinney subcorpus (407 files) had
  zero tree-sitter parse errors — the data is too clean. Mining is slow on large
  directories (>4 minutes for all of Eng-NA). The perturbation approach is more
  effective for systematic error coverage.
- **Parser recovery error specs (E319–E322)**: Writing examples that trigger
  specific tree-sitter error recovery codes is very difficult. Tree-sitter's
  error recovery is robust and routes most malformed input through generic paths
  (E316) rather than the specific recovery codes. These remain as documented
  stubs.
- **Direct parser vs unsupported.cha** *(historical — direct parser has been removed)*:
  The former Chumsky direct parser could not handle `unsupported_line` nodes
  (failed on `constructs/unsupported.cha`). This is no longer relevant since
  tree-sitter is now the sole parser.

## Known Remaining Gaps

1. **12 untriggerable error stubs**: Internal (E001, E002), deprecated (E211,
   E317, E318, E340, E374, E377, E378, E380, E385, E386). These are
   legitimate — the codes either have no emission path or are reserved.
2. **No audio files**: Phase 3.3 (audio subset with %wor tiers) was deferred.
   Adding ~10 short audio clips would test the alignment pipeline end-to-end.
3. **Direct parser roundtrip** *(historical — direct parser has been removed)*:
   373/374 passed under the former Chumsky direct parser (unsupported.cha failed).
   No longer relevant since tree-sitter is now the sole parser.
4. **5 parser recovery specs not_implemented**: E319–E322, E376. Examples don't
   trigger the intended codes due to tree-sitter's error recovery routing.

