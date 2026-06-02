# COMBO -- Boolean Keyword Search

**Status:** Current
**Last updated:** 2026-05-11 17:20 EDT

## Purpose

Searches for utterances matching boolean combinations of keywords. Supports AND (`+`) and OR (`,`) logic with case-insensitive substring matching. This is the primary search tool for finding utterances containing specific words or word combinations.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409095) for the original COMBO command specification.

## Usage

```bash
chatter clan combo -S "want+cookie" file.cha
chatter clan combo -S "want,milk" file.cha
chatter clan combo -S "want+cookie" --speaker CHI file.cha
```

## Options

| Option | Description |
|--------|-------------|
| `--speaker <CODE>` | Include speaker |
| `-S`, `--search <EXPR>` | Search expression (required, repeatable; multiple `--search` flags combined with OR) |
| `--format <FMT>` | Output format: text, json, csv, clan |

## Search Syntax

- `+` between terms means AND (all terms must be present in the utterance)
- `,` between terms means OR (at least one term must be present)
- Terms are case-insensitive substring matches against countable words
- Multiple `--search` flags are combined with OR (any expression matching counts)
- AND takes precedence if both `+` and `,` appear in one expression

## CLAN Equivalence

| CLAN command | Rust equivalent |
|---|---|
| `combo +s"want^cookie" file.cha` | `chatter clan combo file.cha -S "want+cookie"` |
| `combo +s"want\|milk" file.cha` | `chatter clan combo file.cha -S "want,milk"` |
| `combo +s"want^cookie" +t*CHI file.cha` | `chatter clan combo file.cha -S "want+cookie" --speaker CHI` |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.7.10 (`Unique Options`, COMBO) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Normally, combo outputs the location of the tier where the match occurs. When the `+d` switch is turned on you can output only each matched sentence in a simple legal chat format." |
| `+d1` | "Outputs legal chat format along with line numbers and file names." |
| `+d2` | "Outputs files names once per file only." |
| `+d3` | "Outputs legal chat format, but with only the actual words matched by the search string, along with `@Comment` headers that are ignored by other programs." |
| `+d4` | "Use of the `+d4` switch was described in the previous section." (Manual cross-reference; resolution pending.) |
| `+d7` | "Search for words linked between two tiers." |

### Open questions for PI review

1. `+d`/`+d1`/`+d2` parallel KWAL's `+d`/`+d1`/`+d2` almost exactly.
   Worth defining a shared `--display-mode` enum across search-style
   commands (KWAL + COMBO) with the same variant names?
2. `+d3` "matched words plus `@Comment` headers" is COMBO-specific.
   Probably a separate enum variant.
3. `+d4`: manual cross-references the "previous section". This needs
   PI clarification — the immediately-previous section is general
   COMBO description, not a `+d`-table.
4. `+d7` (cross-tier linkage) overlaps with FREQ `+d7`. If both
   commands' `+d7` is "compare two tiers", the enum variant name
   should match.

## Output

Each matching utterance with:

- Source filename
- Speaker code
- Full utterance text (CHAT format)
- Summary counts of matching vs. total utterances

## Differences from CLAN

- **Operator syntax**: CLAN uses `^` for AND and `\|` for OR; this implementation uses `+` and `,` respectively for shell-friendliness.
