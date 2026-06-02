# MAXWD -- Longest Words

**Status:** Current
**Last updated:** 2026-05-11 17:34 EDT

## Purpose

Finds the longest words used by each speaker, reporting a ranked table of unique words sorted by character length descending. Word length is measured in characters after normalization (lowercasing, stripping `+` compound markers and `'` apostrophes for CLAN compatibility).

## Usage

```bash
chatter clan maxwd file.cha
chatter clan maxwd --speaker CHI file.cha
chatter clan maxwd --limit 50 file.cha
```

## Options

| Option | CLAN Flag | Description |
|--------|-----------|-------------|
| `--speaker <CODE>` | `+t*CHI` | Include speaker |
| `--exclude-speaker <CODE>` | `-t*CHI` | Exclude speaker |
| `--limit <N>` | -- | Maximum number of words to show (default: 20) |
| `--format <FMT>` | -- | Output format: text, json, csv |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.19.1 (`Unique Options`, MAXWD) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "The `+d` level of this switch produces output with one line for the length level and the next line for the word." |
| `+d1` | "Produces output with only the longest words, one per line, in order, and in legal chat format." |

### Open questions for PI review

1. `+d` is a two-line-per-result format (length on one line, word on
   next). chatter's current MAXWD output is a single-line table.
   Should `--display-mode 0` produce the two-line legacy form, or
   should we treat the table form as the modern default and only
   honour `+d1` (legal CHAT format)?
2. `+d1` "legal chat format" suggests the output is itself a CHAT
   file. That's a transform-flavoured output, not analyze. The
   chatter approach might be to route `--display-mode 1` to an
   explicit `chatter clan maxwd-extract` transform rather than
   overloading the analyze command.

## Output

Per speaker:

- Table of longest words sorted by length descending (up to `limit`)
- **All occurrences with line numbers** (matching CLAN)
- Maximum word length
- Mean word length
- Total and unique word counts

## Differences from CLAN

### Occurrence reporting

Reports **all occurrences with line numbers**, matching CLAN's output format exactly.

### Word normalization

Length is measured after stripping `+` (compound markers) and `'` (apostrophes), matching CLAN's character counting behavior.

### Output formats

Supports text, JSON, and CSV. CLAN produces text only.

### Golden test parity

100% parity with CLAN C binary output.
