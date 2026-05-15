# FREQPOS — Word Frequency by Position

**Status:** Current
**Last updated:** 2026-05-11 17:35 EDT

## Purpose

Counts how often each word appears in initial, final, other (middle), or one-word positions within utterances. FREQPOS is part of the FREQ family of commands and is useful for studying positional word preferences -- for example, whether a child tends to place certain words at the beginning or end of utterances.

### Position Classification

- **Initial**: first word of a multi-word utterance
- **Final**: last word of a multi-word utterance
- **Other**: any middle word of a multi-word utterance (3+ words)
- **One-word**: the sole word in a single-word utterance

## Usage

```bash
chatter clan freqpos file.cha
chatter clan freqpos file.cha --speaker CHI
```

## Options

| Option | CLAN flag | Description |
|--------|-----------|-------------|
| `--speaker <code>` | `+t*CODE` | Restrict to specific speaker |
| `--format <fmt>` | — | Output format: text, json, csv |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.12.1 (`Unique Options`, FREQPOS) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Count words in either first, second, or other positions. The default is to count by first, last, and other positions." |

### Open questions for PI review

1. FREQPOS's `+d` switches the position-classification scheme from
   "first / last / other / one-word" (default) to
   "first / second / other". That's not a display change — it's a
   *bucketing* change. Map to `--positions <first-last|first-second>`
   enum rather than `--display-mode`.
2. If we keep the `--display-mode` translation, `+d` (no number)
   corresponds to a single behavior (the alternative bucketing), so
   `--display-mode 0` is the only valid value. The clap field should
   probably be an enum with two variants (`Default`,
   `FirstSecondOther`), not a numeric `Option<u8>`.

## Output

Global word list (sorted alphabetically by display form) with positional breakdown (initial/final/other/one-word counts per word), followed by aggregate position totals.

## Differences from CLAN

- Word identification uses AST-based `is_countable_word()` instead of CLAN's string-prefix matching
- Position classification operates on parsed AST word lists rather than raw text token splitting
- Output supports text, JSON, and CSV formats (CLAN produces text only)
- Deterministic output ordering via sorted collections
- **Golden test parity**: Verified against CLAN C binary output
