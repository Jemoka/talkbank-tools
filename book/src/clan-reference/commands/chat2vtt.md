# CHAT2VTT -- CHAT to WebVTT Subtitle Conversion

**Status:** Current
**Last updated:** 2026-05-02 03:00 EDT

## Purpose

Converts a CHAT file to **WebVTT** subtitle format for captioned video.
WebVTT (`.vtt`) is the W3C-standard subtitle format used by HTML5
`<track>` elements; it is closely related to SRT but uses `.` instead of
`,` as the millisecond separator and includes a `WEBVTT` header.

Internally, `chat2vtt` is a thin CLI surface over the same converter
crate as [`chat2srt`](chat2srt.md): both subcommands take a CHAT file
with timing bullets and emit numbered subtitle blocks, just in
different file formats. The shared infrastructure lives at
`crates/clan/clan-core/src/converters/chat2srt.rs` (`chat_to_vtt()` for
this subcommand, `chat_to_srt()` for SRT).

## Usage

```bash
# Write WebVTT to stdout
chatter clan chat2vtt input.cha

# Write to a file
chatter clan chat2vtt input.cha -o input.vtt
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-o`, `--output <PATH>` | stdout | Destination file. Omit to print to stdout. |

The subcommand takes a single positional `<PATH>` argument (one CHAT
file at a time). It does not accept directory inputs, file lists, or
the `CommonAnalysisArgs` filtering flags (`--speaker`, `--gem`, etc.) —
WebVTT conversion is structural, not analytic.

## Input Format

Standard CHAT (`.cha`) files with timing bullets (`•start_end•`) on
utterances. Utterances without timing bullets are excluded from the
output, since WebVTT requires every cue to carry a timestamp.

## Output

Numbered subtitle cues with `HH:MM:SS.mmm --> HH:MM:SS.mmm` timestamps
and cleaned text content (no CHAT markers, annotations, or speaker
codes). The output begins with the `WEBVTT` header line that the
WebVTT specification requires.

```rust,ignore
WEBVTT

1
00:00:01.000 --> 00:00:03.500
hello world

2
00:00:04.200 --> 00:00:06.800
how are you
```

## Difference from `chat2srt`

The two converters share extraction logic and only differ in the
output formatting layer:

| Subcommand | Output format | Timestamp separator | Header |
|---|---|---|---|
| `chat2srt` | SRT (`.srt`) | `,` (e.g. `00:00:01,000`) | none |
| `chat2vtt` | WebVTT (`.vtt`) | `.` (e.g. `00:00:01.000`) | `WEBVTT` |

For the rationale on which utterances are emitted vs skipped, the
shape of cleaned text, and what's intentionally not included from
CLAN's GUI-driven Subtitle Writer flow, see
[`chat2srt`](chat2srt.md). Everything documented there about
extraction also applies here; only the serialization differs.

## Reference

The CLAN manual section [CHAT2SRT](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409296)
covers the original SRT-conversion workflow. WebVTT is an extension
on the Rust side; the manual does not document a separate `CHAT2VTT`
binary.
