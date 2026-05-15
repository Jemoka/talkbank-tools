# FREQ -- Word Frequency

**Status:** Current
**Last updated:** 2026-05-11 17:02 EDT

## Purpose

Counts word tokens and types and computes type-token ratio (TTR). The legacy manual describes `FREQ` as one of CLAN's most powerful and easiest-to-use programs, producing word-frequency counts and lexical-diversity measures over selected files and speakers.

In `clan-core`, `FREQ` counts words on the main tier by default, or morphemes from the `%mor` tier when `--mor` is set.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409093) for the original FREQ command specification.

## Usage

```bash
chatter clan freq file.cha
chatter clan freq --speaker CHI file.cha
chatter clan freq --format json corpus/
chatter clan freq --mor file.cha
chatter clan freq --include-word "the" file.cha
```

> **`+k` / `--case-sensitive` is currently non-functional.** The
> legacy `+k` flag rewrites to `--case-sensitive` (see
> `crates/clan-core/src/clan_args.rs:104`), but no `clap` field
> consumes that token in the current `Freq`/`CommonAnalysisArgs`
> structs, so passing it produces a parse error. Word matching is
> case-insensitive today.

## Options

| Option | CLAN Flag | Description |
|--------|-----------|-------------|
| `--speaker <CODE>` | `+t*CHI` | Include speaker |
| `--exclude-speaker <CODE>` | `-t*CHI` | Exclude speaker |
| `--include-word <WORD>` | `+s"word"` | Only count matching word |
| `--exclude-word <WORD>` | `-s"word"` | Skip matching word |
| `--gem <LABEL>` | `+g"label"` | Restrict to gem segment |
| `--range <START-END>` | `+z25-125` | Utterance range |
| ~~`--case-sensitive`~~ | ~~`+k`~~ | **Currently non-functional** — see callout above |
| `--format <FMT>` | -- | Output format: text, json, csv, clan |
| `--mor` | -- | Count morphemes from `%mor` tier instead of words from main tier |

## CLAN Equivalence

| CLAN command | Rust equivalent |
|---|---|
| `freq file.cha` | `chatter clan freq file.cha` |
| `freq +t*CHI file.cha` | `chatter clan freq file.cha --speaker CHI` |
| `freq +s"the" file.cha` | `chatter clan freq file.cha --include-word "the"` (case-sensitive matching not currently supported — see callout above) |
| `freq *.cha` | `chatter clan freq corpus/` |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** The legacy
> rewriter at `crates/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`, but no `clap` field consumes that token
> today. This section drafts the per-N table from CLAN manual
> §7.10.15 (`Unique Options`, FREQ) verbatim, for PI review and
> subsequent TDD implementation. Tracked in
> `<workspace>/docs/superpowers/plans/2026-05-11-clan-rewriter-honor-three-flags.md`
> Phase 3.

`FREQ` uses `+d` to switch output format, *not* to vary verbosity. Each
value of N selects a different report shape. Quoted from CLAN manual
§7.10.15:

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Perform a particular level of data analysis. By default, the output consists of all selected words found in the input data file(s) and their corresponding frequencies." (Equivalent to no-flag default.) |
| `+d0` | "Output provides a concordance with the frequencies of each word, the files and line numbers where each word, and the text in the line that matches." |
| `+d1` | "Outputs each of the words found in the input data file(s) one word per line with no further information about frequency. Later this output could be used as a word list file for `kwal` or `combo` programs." |
| `+d2` | "Output is sent to a file in a form that can be opened directly in Excel. To do this, you must include information about the speaker roles you wish to include in the output spreadsheet." (Manual example: `freq +d2 +t@ID="*|Target_Child|*" *.cha`.) |
| `+d3` | "Essentially the same as that for `+d2`, but with only the statistics on types, tokens, and the type–token ratio. Word frequencies are not placed into the output." (Note: `+d2` and `+d3` assume `+f`; no need to pass it explicitly.) |
| `+d4` | "Allows you to output just the type–token information." |
| `+d5` | "Output all words you are searching for, including those that occur with zero frequency. ... Can be combined with other `+d` switches." |
| `+d6` | "When used for searches on the main line, outputs matched forms with a separate tabulation of replaced forms, errors, partial omissions, and full forms." Also `+d6 +sm\|n*,o%` on `%mor` line: produces separate counts per part-of-speech instantiation. |
| `+d7` | "Links forms on a 'source' tier with their corresponding words on a 'target' tier." Default source is `%mor`; pass a tier name to change source. Items on the two tiers must be in one-to-one correspondence. `+c5` swaps source ↔ target. |
| `+d8` | "Outputs words and frequencies of cross tabulation of one dependent tier with another." |

### Open questions for PI review

1. `+d0`: emits a concordance — overlaps with `KWAL` semantically. Should
   chatter's `freq --display-mode 0` reuse the `kwal` output path
   internally, or produce its own concordance shape?
2. `+d1`: word-list output suitable as input to `kwal +s@file`. Should
   the file be auto-named (`<basename>.fre`?) or printed to stdout by
   default?
3. `+d2`/`+d3`: "form that can be opened directly in Excel" maps to
   `--format csv` in chatter. Is this duplication acceptable, or should
   `--display-mode 2` *imply* `--format csv` (and conflict-error
   otherwise)?
4. `+d4`: "type-token information only" — same content as the existing
   text/json default, minus the word frequencies. Add a new
   `Truncated` variant to the output struct, or emit a CSV row with
   just types/tokens/TTR?
5. `+d5`: combinable with other `+d` values. How should this combine in
   clap — a `Vec<DisplayMode>` rather than scalar `Option<u8>`?
6. `+d6`/`+d7`/`+d8`: deeply specific to `%mor` and cross-tier
   tabulation. Are these in scope for chatter's freq, or are they
   future work (probably alongside or instead of `mortable` /
   `freqpos`)?

The `+dCN` form (capital `C` plus a number — "output only words used by
<, <=, =, => or > than N percent of speakers") is a separate flag from
plain `+dN`; the rewriter does not currently handle `+dC...`. It would
get its own clap field (`--speaker-percentage`-style) rather than
overload `--display-mode`.

## Output

Per-speaker frequency tables with:

- Word frequency counts (sorted by count descending, then alphabetically)
- Total types (unique words) and tokens (total words)
- TTR (type-token ratio = types / tokens)

### Example output (text)

```text
Speaker: CHI
  the       12
  I         8
  want      6
  a         5
  go        4
  ...
Types: 45
Tokens: 127
TTR: 0.354
```

### Example output (JSON)

```json
{
  "speakers": {
    "CHI": {
      "words": { "the": 12, "I": 8, "want": 6, ... },
      "types": 45,
      "tokens": 127,
      "ttr": 0.354
    }
  }
}
```

## Word Normalization

Words are grouped using `NormalizedWord`, which lowercases and strips compound markers (`+`) for counting purposes, while preserving the original CLAN display form (with `+`) for output. This means `wanna+go` and `Wanna+Go` are counted as the same word.

## Differences from CLAN

### Word identification

The legacy manual says `FREQ` ignores `xxx`, `www`, and words beginning with `0`, `&`, `+`, `-`, or `#` by default, and also ignores header and code tiers unless selected. CLAN implements much of this with character-level string-prefix matching:

```c
if (word[0] == '0') continue;     // omitted words
if (word[0] == '&') continue;     // fillers/nonwords
if (word[0] == '+') continue;     // terminators
```

Our implementation uses AST-based `is_countable_word()`, which checks semantic type rather than string prefixes. This is more precise -- a filler (`&-um`) and a phonological fragment (`&+fr`) have distinct semantic types in our model, even though CLAN lumps them together under the `&` prefix.

### Manual features not yet mirrored directly

The legacy manual documents several advanced `FREQ` workflows, including `+s@file` lexical-group lists, `%mor`/`%gra` combined search with `+d7`, and multilingual searches. Some of those behaviors are covered in `clan-core` through broader filtering infrastructure, but the command chapter should not imply one-for-one flag parity unless explicitly implemented.

### Output ordering

Output is deterministic via sorted collections (count descending, then alphabetically). CLAN's ordering can vary across runs.

### Output formats

Supports text, JSON, and CSV formats. CLAN produces text only. Use `--format clan` for character-level CLAN-compatible output.

### Multi-file behavior

Results are merged across files by default (`+u` behavior). CLAN requires explicit `+u` flag. Use `chatter clan freq dir/` for recursive directory traversal (CLAN uses shell globs).

### Golden test parity

Verified against CLAN C binary output. 100% parity.
