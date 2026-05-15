# KEYMAP — Contingency Tables for Coded Data

**Status:** Current
## Purpose

Builds contingency tables for coded interactional data. The legacy manual describes `KEYMAP` as choosing initiating or beginning codes on a specific coding tier, then examining all codes on that same tier in the next utterance.

In `clan-core`, given a set of keyword codes, `KEYMAP` tracks each keyword occurrence on a specified coding tier and records what code items appear in the immediately following utterance, broken down by speaker.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409207) for the original KEYMAP command specification.

## Usage

```bash
chatter clan keymap file.cha --keyword code1 --keyword code2
chatter clan keymap file.cha --keyword code1 --tier spa
chatter clan keymap file.cha -k code1 -k code2          # short form
```

## Options

| Option | CLAN flag | Description |
|--------|-----------|-------------|
| `-k, --keyword <code>` | — | Primary code to track. Pass repeatedly to track multiple codes. Required. |
| `--tier <name>` | — | Tier label to read codes from (default: `cod`) |
| `--speaker <code>` | `+t*CODE` | Restrict to specific speaker |
| `--format <fmt>` | — | Output format: text, json, csv |

## Output

Per speaker per keyword:

- Total keyword occurrences
- Following codes with speaker attribution and frequency counts

## Differences from CLAN

- **Manual intent**: The legacy manual explicitly treats `KEYMAP` as a coding-tier command and says that only symbols beginning with `$` are considered on that tier; all other strings are ignored.
- Code extraction for `%cod` now uses a clan-local semantic `%cod` item layer derived from the parsed AST rather than flattened tier text
- **Selector handling**: `%cod` selectors such as `<w4>` and `<w4-5>` are treated as item scope, not as stand-alone codes, when deriving keyword and following-code items.
- **Manual constraint not yet fully enforced**: `KEYMAP` currently retains a generic non-`%cod` tier fallback. The manual suggests tighter coding-tier semantics than that fallback provides.
- Keyword matching is case-insensitive by default
- Output supports text, JSON, and CSV formats
- Deterministic ordering via `BTreeMap`
- **Golden test parity**: Verified against CLAN C binary output
