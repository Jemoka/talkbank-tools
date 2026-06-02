# PLAY2CHAT -- PLAY Annotation to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 11:15 EDT

## Purpose

Converts PLAY (Phonological and Lexical Acquisition in Young children) annotation files into CHAT format.

## Usage

```bash
chatter clan play2chat input.play
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"play_corpus"`
(`crates/clan/clan-core/src/converters/play2chat.rs:92`); there is
no CLI flag to override it. If you need a different corpus name,
post-edit the generated `@ID` lines or call
`play_to_chat_with_options()` from Rust.

## Input Format

Tab-separated fields: `speaker`, `start_time`, `end_time`, `text`. Times are in milliseconds and may be empty. Lines starting with `#` or `%` are skipped. Lines with fewer than 2 tab-separated fields are ignored.

Example:

```text
CHI	1000	3500	hello world
MOT	4200	6800	how are you
```

## Output

A well-formed CHAT file with headers and participants. Unique speakers are automatically collected and registered as CHAT participants with the `Unidentified` role. Each PLAY entry becomes an utterance, with timing bullets when start/end times are provided.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
