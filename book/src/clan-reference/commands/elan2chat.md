# ELAN2CHAT -- ELAN XML to CHAT Conversion

**Status:** Current
**Last updated:** 2026-05-12 11:27 EDT

## Purpose

Converts ELAN annotation files (`.eaf`) into CHAT format. ELAN uses a time-aligned annotation format stored as XML, with time slots referenced by alignable annotations within tiers.

## Usage

```bash
chatter clan elan2chat input.eaf
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-l`, `--language` | `"eng"` | ISO 639 language code for the `@Languages` header |
| `-o`, `--output` | stdout | Output CHAT file path |

The corpus name in `@ID` headers is hardcoded to `"elan_corpus"`
(`crates/clan/clan-core/src/converters/elan2chat.rs:230`); there is
no CLI flag to override it.

## Conversion Details

- ELAN tier IDs are mapped to CHAT speaker codes (first 3 characters, uppercased)
- Time slots are resolved to millisecond timing bullets
- Annotations are merged across tiers and sorted by start time
- All speakers are assigned the `Unidentified` participant role

## Input Format

ELAN XML (`.eaf`) files containing `<TIER>` elements with `<ALIGNABLE_ANNOTATION>` entries referencing `<TIME_SLOT>` elements for timing.

## Output

A well-formed CHAT file with `@UTF8`, `@Begin`/`@End` headers, `@Languages`, `@Participants`, and `@ID` headers for each discovered speaker. Each ELAN annotation becomes a timed utterance.

## Implementation Note

Uses simple string-based XML parsing to avoid adding a `quick-xml` dependency. Sufficient for well-formed ELAN files.

## Differences from CLAN

- Uses typed AST for CHAT generation
- Produces valid, well-formed CHAT output

## Reference

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409298) for the original ELAN2CHAT command documentation.
