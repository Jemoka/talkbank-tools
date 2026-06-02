# LIPP2CHAT -- LIPP Phonetic Profile to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 11:25 EDT

## Purpose

Converts LIPP (Logical International Phonetics Programs) phonetic profile data into CHAT format. Each entry becomes an utterance, and phonetic transcriptions are placed on `%pho` dependent tiers.

## Usage

```bash
chatter clan lipp2chat input.lipp
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-s`, `--speaker` | `"CHI"` | Speaker code for all utterances |
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"lipp_corpus"`
(`crates/clan/clan-core/src/converters/lipp2chat.rs:80`); there is
no CLI flag to override it.

## Input Format

Tab-separated word and phonetic transcription, one pair per line:

```text
cat    kaet
dog    dog
```

Lines starting with `#` are treated as comments. Single words without a tab-separated phonetic field are imported without a `%pho` tier.

## Output

A well-formed CHAT file where each LIPP entry becomes an utterance on the main tier. When a phonetic transcription is present, it is placed on a `%pho` dependent tier attached to the utterance.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
