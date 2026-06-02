# MLU -- Mean Length of Utterance

**Status:** Current
**Last updated:** 2026-05-11 17:14 EDT

## Purpose

Calculates mean length of utterance in morphemes from the `%mor` tier. When no `%mor` tier is available and `--words` was not passed, reports "utterances = 0, morphemes = 0" (matching CLAN behavior -- no fallback to word counting).

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409094) for the original MLU command specification.

## Usage

```bash
chatter clan mlu file.cha
chatter clan mlu --speaker CHI file.cha
chatter clan mlu --words file.cha
chatter clan mlu --format json corpus/
```

## Options

| Option | CLAN Flag | Description |
|--------|-----------|-------------|
| `--speaker <CODE>` | `+t*CHI` | Include speaker |
| `--exclude-speaker <CODE>` | `-t*CHI` | Exclude speaker |
| `--words` | -- | Count words from main tier instead of morphemes from `%mor` |
| `--gem <LABEL>` | `+g"label"` | Restrict to gem segment |
| `--range <START-END>` | `+z25-125` | Utterance range |
| `--format <FMT>` | -- | Output format: text, json, csv, clan |

## CLAN Equivalence

| CLAN command | Rust equivalent |
|---|---|
| `mlu file.cha` | `chatter clan mlu file.cha` |
| `mlu +t*CHI file.cha` | `chatter clan mlu file.cha --speaker CHI` |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** The
> rewriter at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`, but no `clap` field consumes that token
> today. Drafted from CLAN manual §7.21.2 (`Unique Options`, MLU) for
> PI review. Plan: `<workspace>/docs/superpowers/plans/2026-05-11-clan-rewriter-honor-three-flags.md`
> Phase 3.

MLU's `+d` table is small — two N-values, both Excel-friendly output
formats. Quoted from CLAN manual §7.21.2:

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "You can use this switch, together with the ID specification to output data for Excel." Example: `mlu +d +tCHI sample.cha` produces a one-line @ID-keyed record: ``en\|sample\|CHI\|1;10.4\|female\|\|\|Target_Child\|\| 5  7 1.400 0.490`` (fields: @ID, utterance count, morpheme count, MLU, MLU std dev). Requires `@ID` headers per participant. |
| `+d1` | "This level of the `+d` switch outputs data in another systematic format, with data for each speaker on a single line. However, this form is less adapted to input to a statistical program than the output for the basic `+d` switch. Also, this switch works with the `+u` switch, whereas the basic `+d` switch does not." Example: ``*CHI:  5  7 1.400 0.490``. |

### Open questions for PI review

1. `+d` (no number) maps cleanly to `--format csv` in chatter. Should
   `--display-mode 0` (or absent N) imply `--format csv`, or remain a
   separate axis?
2. `+d1` is "less adapted to statistical input" yet combinable with
   `+u`. That combinability is the differentiating feature; should
   chatter expose it as a `--display-mode merged-by-speaker` enum
   variant?
3. The `+d` output requires `@ID` headers per participant. Should
   `--display-mode` error early if `@ID` rows are missing for any
   matched speaker, or fall back to the speaker-code-only form
   silently?

## Algorithm

For each utterance with a `%mor` tier:

1. Count **1 per stem** (the base morpheme word)
2. Count **+1 per bound morpheme suffix** -- but ONLY these 7 suffix strings: `PL`, `PAST`, `Past`, `POSS`, `PASTP`, `Pastp`, `PRESP`
3. Count **+1 per clitic stem** (`~` separated)
4. Count clitic suffixes using the same 7-string rule
5. **Fusional features** (`&PRES`, `&INF`, etc.) do NOT count

Per speaker, compute:
- Number of utterances
- Total morphemes
- **MLU** (mean = total morphemes / utterances)
- **Standard deviation** (population SD, dividing by n)
- **Range** (min, max morphemes per utterance)

### Brown's Morpheme Rules

This was a key discovery during parity verification. CLAN only counts 7 specific suffix strings as bound morphemes:

| Suffix | Meaning |
|--------|---------|
| `PL` | Plural |
| `PAST` | Past tense |
| `Past` | Past tense (alternate) |
| `POSS` | Possessive |
| `PASTP` | Past participle |
| `Pastp` | Past participle (alternate) |
| `PRESP` | Present participle |

All other suffixes (including fusional features like `&PRES`, `&INF`, `&3S`) are ignored for MLU counting. This matches Brown's (1973) original operationalization of "morpheme" for child language analysis.

### Example

Given `%mor: pro|I v|want-PAST det|a n|cookie-PL`:

- `pro|I` = 1 stem = **1**
- `v|want-PAST` = 1 stem + 1 suffix (PAST) = **2**
- `det|a` = 1 stem = **1**
- `n|cookie-PL` = 1 stem + 1 suffix (PL) = **2**
- Total: **6 morphemes**

## Output

```text
Speaker: CHI
  Utterances: 42
  Morphemes: 168
  MLU: 4.000
  SD: 1.732
  Range: 1-9
```

## Differences from CLAN

### Standard deviation

Uses **population SD** (dividing by n), not sample SD (dividing by n-1). Verified against CLAN output -- CLAN uses population SD too.

### Morpheme counting

Uses parsed `%mor` tier structure (`MorWord` features and post-clitics) rather than text splitting on spaces and delimiters. The semantic result is identical thanks to applying Brown's 7-suffix rule, but the mechanism is type-safe.

### No %mor tier behavior

When no `%mor` tier exists and `--words` was not passed, reports 0 utterances for the speaker (matching CLAN). Does not silently fall back to word counting.

### Output formats

Supports text, JSON, and CSV. CLAN produces text only.

### Golden test parity

100% parity with CLAN C binary output.
