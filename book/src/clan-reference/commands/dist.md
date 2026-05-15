# DIST -- Word Distribution Across Turns

**Status:** Current
**Last updated:** 2026-05-11 17:33 EDT

## Purpose

Counts turns and tracks for each word the first and last turn in which it appears. DIST is part of the FREQ family of commands and is useful for studying when words first appear and how their usage is distributed across a conversation.

## Usage

```bash
chatter clan dist file.cha
chatter clan dist --speaker CHI file.cha
chatter clan dist --format json file.cha
```

## Options

| Option | CLAN Flag | Description |
|--------|-----------|-------------|
| `--speaker <CODE>` | `+t*CHI` | Include speaker |
| `--exclude-speaker <CODE>` | `-t*CHI` | Exclude speaker |
| `--format <FMT>` | -- | Output format: text, json, csv |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.9.1 (`Unique Options`, DIST) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Output data in a form suitable for statistical analysis." |

DIST's `+d7` is mentioned in passing in the manual as part of the
FREQ-family `+d7` cross-tier comparison.

### Open questions for PI review

1. "Form suitable for statistical analysis" maps cleanly to
   `--format csv` in chatter. Should `+d` translate directly to
   `--format csv` at rewrite time (drop the `--display-mode`
   translation entirely for DIST), or honour both?
2. The DIST `+d7` mention is a stub — is DIST genuinely a `+d7`
   user, or is the manual cross-referencing FREQ's `+d7`?

## Output

Global word list (sorted alphabetically by display form) with:

- Occurrence count across all turns
- First turn number (1-based) in which the word occurs
- Last turn number (omitted if same as first)
- Total number of turns in the transcript

## Turn Definition

**Every utterance is its own turn**, regardless of whether the speaker changed. This matches CLAN's behavior, which was verified during parity testing. There is no speaker-continuity grouping -- each utterance increments the turn counter.

This is different from how turns are defined in MLT (where consecutive utterances by the same speaker form a single turn).

## Differences from CLAN

### Turn counting

Every utterance = one turn (no speaker-continuity grouping), matching CLAN exactly.

### Word identification

Uses AST-based `is_countable_word()` instead of CLAN's string-prefix matching.

### Output formats

Supports text, JSON, and CSV. CLAN produces text only.

### Golden test parity

100% parity with CLAN C binary output.
