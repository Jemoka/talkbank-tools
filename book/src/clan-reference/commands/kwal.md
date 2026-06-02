# KWAL -- Keyword And Line

**Status:** Current
**Last updated:** 2026-05-11 17:10 EDT

## Purpose

Searches for clusters containing specified keywords and displays the matching lines with context. The legacy manual gives `KWAL` a dedicated section and describes it as operating on "clusters": the main tier plus the selected dependent tiers associated with that line.

In `clan-core`, keywords are currently matched against countable words on the main tier, with the matched utterance shown in context.

## Usage

```bash
chatter clan kwal -k want file.cha
chatter clan kwal -k want --speaker CHI file.cha
chatter clan kwal -k want -k cookie file.cha
```

## Options

| Option | Description |
|--------|-------------|
| `--speaker <CODE>` | Include speaker |
| `-k <WORD>` / `--keyword <WORD>` | Keyword to search for (repeatable) |
| `--format <FMT>` | Output format: text, json, csv, clan |

## CLAN Equivalence

| CLAN command | Rust equivalent |
|---|---|
| `kwal +s"want" file.cha` | `chatter clan kwal file.cha -k want` |
| `kwal +s"want" +t*CHI file.cha` | `chatter clan kwal file.cha -k want --speaker CHI` |

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** The
> rewriter at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`, but no `clap` field consumes that token
> today. This table is drafted from CLAN manual §7.17.5
> (`Unique Options`, KWAL) verbatim for PI review. Tracked in
> `<workspace>/docs/superpowers/plans/2026-05-11-clan-rewriter-honor-three-flags.md`
> Phase 3.

`KWAL` uses `+d` to switch the output shape (plain CHAT, with filenames,
Excel form, etc.). Quoted from CLAN manual §7.17.5:

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d` (no number) | "Normally, kwal outputs the location of the tier where the match occurs. When the `+d` switch is turned on you can \[output\] in these formats: ... outputs legal CHAT format." |
| `+d1` | "Outputs legal CHAT format plus file names and line numbers." |
| `+d2` | "Outputs file names once per file only." |
| `+d3` | "Outputs ONLY matched items." |
| `+d30` | "Outputs ONLY matched items without any defaults removed. The `+d30` and the `+d3` switches can be combined." |
| `+d99` | "Convert 'word \[x 2\]' to 'word \[/\] word' and so on." |
| `+d4` | "Outputs for Excel." |
| `+d40` | "Outputs for Excel, repeating the same tier for every keyword match." |
| `+d7` | "Compares items across dependent tiers." Example: `kwal +d7 +s@\|-cop +sROOT +t%gra +t%mor t.cha` |

### Open questions for PI review

1. `+d` (no number): the manual implies the default behavior already
   prints match location; `+d` *changes* the format to legal CHAT.
   Does `--display-mode 0` (or `--display-mode chat`) feel right as a
   chatter spelling? An enum-valued flag would be more honest than a
   `0..99` numeric range.
2. `+d30` is "`+d3` + don't strip defaults" — combinable. Maps to
   `--display-mode matched --no-strip-defaults` or
   `--display-mode 30`?
3. `+d99` is conceptually orthogonal to the others (it's a
   transformation, not an output shape). Worth splitting into a
   separate `--expand-repetition` flag rather than overloading
   `--display-mode`.
4. `+d4`/`+d40` for Excel: same Excel question as FREQ — overlap with
   `--format csv`.
5. `+d7` cross-tier comparison: deeply specific. In scope for the
   first `--display-mode` pass, or future work alongside `freqpos` /
   `mortable`?

## Output

Each matching utterance with:

- Speaker code
- Full utterance text
- File path (for multi-file searches)
- Match count summary per keyword

## Differences from CLAN

- **Manual intent**: `KWAL` is a cluster-oriented search command, not just a main-tier keyword matcher.
- **Search**: Operates on parsed AST word content rather than raw text lines.
- **Word identification**: Uses AST-based `is_countable_word()` instead of CLAN's string-prefix matching.
- **Scope reduction**: The legacy manual describes richer tier-selection and output-shaping behavior, including cluster searches over selected dependent tiers and `%mor`/`%gra` combined searches with `+d7`. The current implementation is narrower.
- **Output formats**: Supports text, JSON, and CSV formats (CLAN produces text only).
- **Golden test parity**: Verified against CLAN C binary output.
