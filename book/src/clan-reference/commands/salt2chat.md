# SALT2CHAT -- SALT Transcription to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 13:36 EDT

## Purpose

Converts SALT (Systematic Analysis of Language Transcripts) transcription files into CHAT format. SALT is a widely used clinical transcription system with its own conventions for speaker codes, morpheme annotations, and error marking.

## Usage

```bash
chatter clan salt2chat input.slt
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"salt_corpus"`
(`crates/clan/clan-core/src/converters/salt2chat.rs:193`); there is
no CLI flag to override it. Same pattern as the other converters
in this directory.

## Speaker Mapping

| SALT code | CHAT speaker | Role |
|-----------|-------------|------|
| `C` | `CHI` | Target_Child |
| `E` | `EXA` | Investigator |
| `P` | `PAR` | (Parent) |
| `I` | `INV` | (Investigator) |

## SALT Annotation Stripping

SALT-specific annotations are removed during conversion:

- Morpheme codes (`word/3s` --> `word`)
- Error markers (`word*` --> `word`)
- Maze markers (`(word)` --> skipped)
- Comment markers (`{...}`, `[...]` --> skipped)
- Bound morpheme markers (`_word` --> `word`)

## Input Format

SALT transcription files with header lines (starting with `$` or `+`) followed by speaker-prefixed utterance lines. SALT uses single-letter speaker codes and inline annotation conventions.

## Output

A well-formed CHAT file with SALT speakers mapped to standard CHAT speaker codes, SALT-specific annotations stripped, and proper CHAT headers generated. Header metadata (participant name, age, gender, context) is extracted from SALT `$` lines when available.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output
