# COOCCUR — Word Co-occurrence (Bigram) Counting

**Status:** Current
**Last updated:** 2026-05-11 17:32 EDT

## Purpose

Counts adjacent word pairs (bigrams) across utterances. For each utterance, every pair of consecutive countable words is recorded as a directed bigram. Pairs are directional: ("put", "the") and ("the", "put") are counted separately.

COOCCUR is part of the FREQ family of commands and is useful for studying word collocations and sequential patterns in speech.

## Usage

```bash
chatter clan cooccur file.cha
chatter clan cooccur file.cha --speaker CHI
```

## Options

| Option | CLAN flag | Description |
|--------|-----------|-------------|
| `--speaker <code>` | `+t*CODE` | Restrict to specific speaker |
| `--format <fmt>` | — | Output format: text, json, csv |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.8.1 (`Unique Options`, COOCUR) for
> PI review. Manual uses CLAN's `COOCUR` spelling; chatter's
> subcommand is `cooccur`.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Strip the numbers from the output data that indicate how often a certain cluster occurred." |

### Open questions for PI review

1. `+d` here removes frequency counts from the output, leaving just
   the cluster strings. That's a content-stripping switch, not a
   format selector. Map to `--no-counts` boolean or
   `--display-mode tokens-only` enum variant?
2. There is no `+d1`/`+d2`/... documented for COOCUR — only the
   bare `+d`. The clap `--display-mode N` shape may want a
   `Option<u8>` where `None` = default, `Some(0)` = bare `+d` form.

## Output

- Table of adjacent word pairs with co-occurrence counts
- Default sort: by frequency descending, then alphabetically
- CLAN output: sorted alphabetically by pair display form
- Summary: unique pair count, total pair instances, total utterances

## Differences from CLAN

- Word identification uses AST-based `is_countable_word()` instead of CLAN's string-prefix matching
- Bigram extraction operates on parsed AST content rather than raw text
- Output supports text, JSON, and CSV formats (CLAN produces text only)
- Deterministic output ordering via sorted collections
- **Golden test parity**: Verified against CLAN C binary output
