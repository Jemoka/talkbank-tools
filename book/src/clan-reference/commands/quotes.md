# QUOTES -- Extract Quoted Text to Separate Utterances

**Status:** Current
## Purpose

Reimplements CLAN's QUOTES command.

This is a relatively uncommon command used for discourse analysis of reported speech.

## Usage

```bash
chatter clan quotes file.cha
```

## Options

This command has no configurable options.

## Behavior

The Rust port now inspects the parsed CHAT AST directly.

- If no quote-extraction postcode (`[+ "]`) is present, the command is a semantic no-op and emits normalized CHAT.
- If `[+ "]` is present, the command fails with an explicit error. `clan-core` does not silently fall back to post-serialization string manipulation for this transform.

## Differences from CLAN

- Does not currently implement CLAN's text-level extraction rewrite for `[+ "]`.
- Fails explicitly on unsupported quote-extraction postcodes instead of attempting a lossy raw-text rewrite.
