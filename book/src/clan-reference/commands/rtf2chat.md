# RTF2CHAT -- Rich Text Format to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 13:35 EDT

## Purpose

Converts Rich Text Format (RTF) files into CHAT format by stripping RTF formatting commands and extracting plain text content.

## Usage

```bash
chatter clan rtf2chat input.rtf
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"rtf_corpus"`
(`crates/clan-core/src/converters/rtf2chat.rs:212`); there is
no CLI flag to override it. Same pattern as the other converters
in this directory.

## Processing Steps

1. **RTF stripping**: Removes control words, groups, font/color/stylesheet tables, and converts Unicode escapes (`\uN?`) to characters. Handles `\par` (newline) and `\tab` (tab).
2. **Turn extraction**: Looks for CHAT-style speaker prefixes (`*CHI:`, `*MOT:`) in the plain text. If none are found, all text is assigned to a default `SPK` speaker.
3. **CHAT construction**: Builds a proper `ChatFile` with headers, participants, and utterances.

## Input Format

RTF (`.rtf`) files, optionally containing CHAT-style speaker prefixes embedded in the rich text. Standard RTF control sequences are supported including font tables, color tables, stylesheets, Unicode escapes, and nested groups.

## Output

A well-formed CHAT file. If the RTF contains CHAT-style speaker codes (`*CHI:`, `*MOT:`, etc.), those are preserved as proper CHAT speaker codes. Otherwise, all text is assigned to a default `SPK` speaker.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
