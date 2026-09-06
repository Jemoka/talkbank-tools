# CLAN Line Numbering

**Status:** Current
**Last updated:** 2026-03-16

When you click "Open in CLAN" in the desktop app or press Enter in the TUI,
chatter sends the error location to the CLAN editor. CLAN opens the file and
places the cursor at the error. This usually works seamlessly, but there is
one caveat: CLAN and chatter count lines differently.

## Hidden Headers

CLAN hides five header types from its editor display:

| Header | Purpose |
|--------|---------|
| `@UTF8` | Character encoding declaration |
| `@PID` | Persistent identifier |
| `@Font` | Display font settings |
| `@ColorWords` | Color coding rules |
| `@Window` | Window position/size |

These headers are present in the `.cha` file but invisible in CLAN's editor.
CLAN's line numbers skip them entirely. A file that starts with `@UTF8` on
line 1 will show `@Begin` as "line 1" in CLAN's display, even though it's
actually line 2 in the file.

## What Chatter Does

Chatter automatically adjusts line numbers before sending to CLAN:

1. Compute the error's line number in the source file
2. Count how many hidden headers appear before that line
3. Subtract the hidden count to get CLAN's line number
4. Send the adjusted line number to CLAN

This happens transparently — you don't need to do anything.

## Edge Case: Errors on Hidden Lines

If an error is on a hidden header itself (e.g., a malformed `@UTF8` line),
CLAN cannot navigate to it because CLAN doesn't display that line. In this
case, "Open in CLAN" will show an error message explaining why.

## For Developers

The shared resolution logic lives in `talkbank_model::resolve_clan_location()`.
Both the TUI and the desktop app call this function — it resolves line/column
from byte offsets when needed and adjusts for hidden headers.

See [`clan_location.rs`](https://github.com/TalkBank/batchalign/blob/main/crates/core/talkbank-model/src/errors/clan_location.rs)
for the implementation and tests.
