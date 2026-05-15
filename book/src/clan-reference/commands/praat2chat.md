# PRAAT2CHAT -- Praat TextGrid Bidirectional Conversion

**Status:** Current
**Last updated:** 2026-05-12 11:29 EDT

## Purpose

Converts between Praat TextGrid files and CHAT format. TextGrid files contain time-aligned interval tiers widely used in phonetic research.

## Usage

```bash
chatter clan praat2chat input.TextGrid
```

## Conversion Functions

| Direction | Function | Description |
|-----------|----------|-------------|
| TextGrid to CHAT | `praat_to_chat()` | Convert TextGrid intervals to CHAT utterances |
| CHAT to TextGrid | `chat_to_praat()` | Convert timed CHAT utterances to TextGrid intervals |

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"praat_corpus"`
(`crates/clan-core/src/converters/praat2chat.rs:200`); there is
no CLI flag to override it. Same pattern as the other converters
in this directory.

## TextGrid Format Support

Both long (normal) and short TextGrid formats are supported. Tier names are mapped to CHAT speaker codes (first 3 characters, uppercased). Empty intervals and point tiers are skipped. Untimed utterances are excluded from CHAT-to-TextGrid conversion.

## Input Format

Praat TextGrid files (`.TextGrid`) containing interval tiers with time-aligned text segments.

## Output

**TextGrid to CHAT**: A well-formed CHAT file with timing bullets derived from interval boundaries. Each non-empty interval becomes a timed utterance.

**CHAT to TextGrid**: A Praat TextGrid file with one interval tier per speaker, containing text from timed utterances.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
- Supports bidirectional conversion (CHAT to TextGrid and TextGrid to CHAT)
- Handles both long and short TextGrid formats

## Reference

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409302) for the original PRAAT2CHAT command documentation.
