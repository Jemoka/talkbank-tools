# ROLES — Rename Speakers

**Status:** Current
**Last updated:** 2026-05-12 13:33 EDT

## Purpose

Renames speaker codes throughout a CHAT file: in `@Participants`, `@ID` headers, and all main-tier speaker prefixes. Used to standardize speaker codes across a corpus.

## Usage

```bash
chatter clan roles --rename "EXP=INV" file.cha
chatter clan roles --rename "Child=CHI" --rename "Mother=MOT" file.cha
```

## Options

| Option | Description |
|--------|-------------|
| `-r`, `--rename "OLD=NEW"` | Rename speaker OLD to NEW (required, can be repeated). Splits on the first `=`; see `crates/chatter/chatter-cli/src/commands/clan/transforms.rs:172` for the parser. |
| `-o`, `--output` | Output CHAT file path (default: stdout). |

## Behavior

Speaker codes are renamed in all structural locations:
- `@Participants` header entries
- `@ID` header speaker fields
- Main-tier speaker prefixes (`*OLD:` becomes `*NEW:`)

## Differences from CLAN

- Operates on the typed AST rather than raw text.
- Speaker codes are renamed in all structural locations via AST manipulation.
