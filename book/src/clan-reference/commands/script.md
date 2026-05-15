# SCRIPT — Compare Utterances to a Template

**Status:** Current
**Last updated:** 2026-05-11 17:47 EDT

## Purpose

Compares subject CHAT data against an ideal template file to compute accuracy metrics: words produced vs. expected, correct matches, omissions (in template but not produced), and additions (produced but not in template). Useful for evaluating scripted language samples such as picture descriptions or story retells.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409234) for the original SCRIPT command specification.

## Usage

```bash
chatter clan script file.cha --template template.cha
chatter clan script corpus/ --template template.cha --speaker CHI
```

## Algorithm

1. Parse the template CHAT file and build a word frequency map (ideal counts)
2. For each subject utterance, accumulate word frequency counts
3. At finalization, compute per-word matches (minimum of ideal and actual), omissions, and additions

## Options

| Option | CLAN flag | Description |
|--------|-----------|-------------|
| `--template <path>` | — | Path to template/script file (required) |
| `--speaker <code>` | `+t*CODE` | Restrict to specific speaker |
| `--format <fmt>` | — | Output format: text, json, csv |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.25.5 (`Unique Options`, SCRIPT) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Outputs default results in SPREADSHEET format." |
| `+d1` | "Outputs ratio of words and utterances over time duration." |
| `+d10` | "Outputs above, `+d1`, results in SPREADSHEET format." |

### Open questions for PI review

1. `+d` (spreadsheet) overlaps with `--format csv` exactly. Should
   `+d` rewrite to `--format csv` instead of `--display-mode 0`?
2. `+d10` = `+d1` + spreadsheet — a combinator. The numeric encoding
   `10` mixes a "ratio mode" with the "spreadsheet" output format on
   one axis. Better factored as
   `--display-mode ratio --format csv` (two orthogonal flags) rather
   than `--display-mode 10`.
3. `+d1` requires "time duration" data — implies bullet timings or
   `@Duration` headers in the input. Should chatter validate that
   prerequisite at clap-parse time or fail at runtime with a clear
   error?

## Output

Per file:

- Words produced by subject
- Words expected from template
- Correct words (matched)
- Omitted words (in template but not produced)
- Added words (produced but not in template)
- Percentage correct

Overall totals across all files.

## Differences from CLAN

- Template file is parsed into a typed AST (not raw text comparison)
- Word matching uses `NormalizedWord` for case-insensitive comparison
- Omissions and additions are computed from frequency maps rather than positional alignment, which may produce different results when word order matters
- Output supports text, JSON, and CSV formats
- **Golden test parity**: Verified against CLAN C binary output
