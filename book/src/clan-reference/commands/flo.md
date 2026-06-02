# FLO -- Simplified Fluent Output

**Status:** Current
## Purpose

Reimplements CLAN's `flo` command, which generates a `%flo:` dependent tier containing a simplified, "fluent" version of each utterance's main line.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409312) for the original command documentation.

## Usage

```bash
chatter clan flo file.cha
```

## Options

This command has no configurable options.

## Behavior

Processing steps:

1. Strips all header lines (no `@UTF8`, `@Begin`, `@End`, etc.)
2. Adds a `%flo:` dependent tier to each utterance containing the simplified main line: just countable words plus the terminator
3. Strips retrace targets (words/groups before `[/]`, `[//]`, `[///]`, `[/-]` — the four `RetraceKind` variants per `crates/core/talkbank-model/src/model/content/retrace.rs`)
4. Strips non-countable words (`xxx`/`yyy`/`www`, `0word`, `&~frag`, `&-um`)
5. Strips events (`&=thing`) and pauses
6. For replaced words (`[: form]`), uses the replacement (corrected form)
7. Keeps existing dependent tiers (`%mor`, `%gra`, etc.)

The `%flo:` tier is inserted at position 0 (before other dependent tiers).

## Differences from CLAN

- Operates on AST rather than raw text.
- Uses the framework transform pipeline (parse -> transform -> serialize -> write).
