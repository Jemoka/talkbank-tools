# TEXT2CHAT -- Plain Text to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 13:39 EDT

## Purpose

Converts plain text files into CHAT format by splitting on sentence-ending punctuation (`.`, `?`, `!`) and assigning all utterances to a default speaker. This is the simplest converter, useful for bootstrapping CHAT files from raw text.

## Usage

```bash
chatter clan text2chat input.txt
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-s`, `--speaker` | `"SPK"` | Speaker code for all utterances |
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"text_corpus"`
(`crates/clan-core/src/converters/text2chat.rs:37`); there is
no CLI flag to override it. Same pattern as the other converters
in this directory.

## Input Format

Plain text files. Newlines within the input are treated as spaces (not sentence boundaries). The text is split into utterances at sentence-ending punctuation (`.`, `?`, `!`).

## Output

A well-formed CHAT file where each sentence becomes an utterance. Sentence terminators are preserved as CHAT terminators (period, question mark, exclamation point). Trailing text without punctuation receives a default period terminator.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
