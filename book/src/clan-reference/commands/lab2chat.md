# LAB2CHAT -- LAB Timing Labels to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 11:25 EDT

## Purpose

Converts LAB (label) timing files into CHAT format. LAB files contain time-aligned word or segment labels commonly used in speech research tools (e.g., HTK, Kaldi).

## Usage

```bash
chatter clan lab2chat input.lab
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-s`, `--speaker` | `"SPK"` | Speaker code for all utterances |
| `-L`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header (note: uppercase `-L` because lowercase `-l` would conflict) |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"lab_corpus"`
(`crates/clan/clan-core/src/converters/lab2chat.rs:110`); there is
no CLI flag to override it.

## Supported Formats

- **Three-column**: `start_time end_time label` (times in seconds)
- **Two-column**: `time label` (end time inferred from the next entry)

## Input Format

Plain text files with whitespace-separated columns. Silence markers (`sil`, `sp`, `#`) are skipped during conversion. Comment lines starting with `#` and blank lines are ignored.

Example:

```text
0.0 0.5 hello
0.5 1.2 world
1.2 1.5 sil
```

## Output

A well-formed CHAT file where each non-silence label becomes a separate utterance with timing bullets derived from the LAB timestamps (converted from seconds to milliseconds).

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
