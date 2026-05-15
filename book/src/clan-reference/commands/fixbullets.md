# FIXBULLETS -- Fix Timing Bullet Consistency

**Status:** Current
## Purpose

Repairs timing bullets that link CHAT to audio or video. The legacy manual describes `FIXBULLETS` more broadly: converting old-format bullets to new format, inserting `@Media`, merging multiple bullets, adding language tags, and shifting global timing offsets.

`clan-core` now supports the AST-safe subset of that behavior: main-tier monotonic bullet repair, global millisecond offsets on parsed bullet timings, and tier-scoped bullet repair using parsed tier kinds.

## Usage

```bash
chatter clan fixbullets file.cha
chatter clan fixbullets file.cha --offset 800
chatter clan fixbullets file.cha --tier cod
chatter clan fixbullets file.cha --exclude-tier com
```

## Options

- `--offset N`
  Shift parsed bullet timings by `N` milliseconds. Negative offsets fail if they would move a parsed bullet before `0`.
- `--tier S`
  Restrict processing to selected tier kinds such as `cod`, `%cod`, or `*` for main tiers.
- `--exclude-tier S`
  Exclude selected tier kinds from processing.

## Behavior

The transform iterates through the parsed AST and:

1. Shifts parsed bullet timings on supported AST locations: main-tier terminal bullets, main-tier inline word/internal bullets, bullet-content dependent tiers such as `%act/%cod/%com`, and `%wor` inline bullets.
2. Enforces non-overlapping, monotonic timing windows on main-tier terminal bullets.
3. Preserves duration when repairing an overlapping main-tier terminal bullet, using a minimum duration of 1 ms.

Utterances without main-tier terminal bullets are skipped for monotonic tracking.

## Differences from CLAN

- **Current supported scope**: `FIXBULLETS` supports global offsets and tier-scoped AST-native bullet repair on parsed bullet locations, including bullet-bearing `@Comment` headers.
- **Scope reduction remains:** the
  [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) documents a
  broader repair tool than the current implementation provides.
- **Not yet implemented:** old-to-new bullet conversion, `@Media` insertion, multi-bullet merge (`+b`), and `+l` language-tag insertion described in the manual.
- Operates on AST rather than raw text.
- Uses the framework transform pipeline (parse -> transform -> serialize -> write).
