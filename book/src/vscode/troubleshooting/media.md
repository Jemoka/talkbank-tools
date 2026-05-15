# Media Not Found

**Status:** Current
**Last updated:** 2026-03-30 13:40 EDT

This chapter covers issues with media playback -- audio or video files not found, unsupported formats, and remote connection limitations.

## @Media Header Missing

**Symptom:** Play at Cursor, Continuous Play, and Waveform View commands do nothing or show an error about missing media.

**Fix:** The `.cha` file must have an `@Media:` header that references the audio or video file:

```text
@Media:	interview, audio
```

The value before the comma is the base filename (without extension). The value after the comma is the media type (`audio` or `video`). Add this header manually or use the [Participant Editor](../editing/participant-editor.md) to edit file metadata.

## Media File Not Found

**Symptom:** The `@Media:` header is present, but the extension reports that the media file cannot be found.

The extension resolves media files by searching for a file whose base name matches the `@Media` value, in these locations:

1. **Same directory** as the `.cha` file
2. **`media/` subdirectory** relative to the `.cha` file

For example, if the `.cha` file is at `/data/resources/corpus/interview.cha` and the header says `@Media: interview, audio`, the extension looks for:

- `/data/resources/corpus/interview.mp3`
- `/data/resources/corpus/interview.wav`
- `/data/resources/corpus/interview.m4a`
- `/data/resources/corpus/interview.mp4`
- `/data/resources/corpus/media/interview.mp3`
- (and so on for other supported extensions)

**Fixes:**

1. Ensure the media file exists in one of these locations
2. Ensure the base filename in `@Media:` matches the actual file (case-sensitive on macOS/Linux)
3. Check that the file extension is one the extension supports (see below)

## Unsupported Format

The extension uses the browser's built-in audio/video API (Web Audio API and HTML5 `<audio>`/`<video>` elements) for playback. Supported formats depend on the Electron/Chromium runtime bundled with VS Code:

| Format | Extension | Typically Supported |
|--------|-----------|-------------------|
| MP3 | `.mp3` | Yes |
| WAV | `.wav` | Yes |
| AAC / M4A | `.m4a`, `.aac` | Yes |
| MP4 (video) | `.mp4` | Yes |
| OGG Vorbis | `.ogg` | Yes |
| WebM | `.webm` | Yes |
| FLAC | `.flac` | Usually yes |
| WMA | `.wma` | No |
| AVI | `.avi` | No (use MP4) |

If your media file is in an unsupported format, convert it to MP3 or WAV using a tool like `ffmpeg`:

```bash
ffmpeg -i recording.wma recording.mp3
```

## No Audio on Remote Connection

**Symptom:** Media playback does not work when connected via VS Code Remote SSH or Remote Containers.

This is a known limitation. VS Code's webview panels (which the media player uses) have limited media support over remote connections. The audio file must be accessible to the VS Code client, not just the remote server.

**Workarounds:**

- **Copy the media file locally** and open the `.cha` file locally
- **Use VS Code's port forwarding** to make the media accessible (may not work for all formats)
- **Use the desktop `chatter` app** instead of the VS Code extension for remote media review

## Waveform Not Rendering

**Symptom:** The waveform panel opens but shows a blank or error state.

The waveform is rendered using the Web Audio API, which must decode the entire audio file into memory. Issues can occur with:

- **Very large files** -- audio files over 1 GB may exceed memory limits
- **Unsupported codecs** -- the Web Audio API decoder may reject certain codec variants even if the `<audio>` element can play them
- **Corrupted files** -- truncated or corrupted audio files will fail to decode

**Fixes:**

1. Try a smaller audio file to confirm the waveform feature works
2. Convert the file to WAV or MP3 format
3. Check the developer console (Help > Toggle Developer Tools) for specific decode errors

## Playback and Timing Mismatch

**Symptom:** The highlighted utterance does not match what you hear, or segments seem offset.

This can happen when:

- **Timing bullets are incorrect** -- the bullet values in the transcript do not match the actual audio timing. Use [Review Mode](../workflows/post-alignment-review.md) to verify and correct.
- **Variable bitrate encoding** -- some VBR-encoded files have seeking inaccuracies. Convert to constant bitrate (CBR) if precision is important.

## Related Chapters

- [Media Resolution](../media/resolution.md) -- how the extension finds media files
- [Waveform View](../media/waveform.md) -- waveform rendering details
- [Transcription Workflow](../workflows/transcription.md) -- creating transcripts from audio
- [Post-Alignment Review](../workflows/post-alignment-review.md) -- reviewing timing accuracy
