# Settings Reference

**Status:** Current
**Last updated:** 2026-04-16 13:37 EDT

All extension settings are accessible via **File > Preferences > Settings** (`Cmd+,` on macOS, `Ctrl+,` on Windows/Linux) and searching for "talkbank". Settings can also be edited directly in `settings.json`.

## Transcription

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `talkbank.transcription.defaultSpeaker` | string | `"CHI"` | Speaker code inserted on new utterance lines during transcription mode. Change this when transcribing a conversation where the primary speaker is not the child (e.g., set to `"PAR"` for participant). |
| `talkbank.transcription.rewindSeconds` | number | `2` | Number of seconds to rewind when pressing `F8`. Range: 0.5--30. |

## Walker and Playback

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `talkbank.walker.autoPlay` | boolean | `true` | Automatically play the media segment when stepping through utterances with the walker (`Alt+Down` / `Alt+Up`). Set to `false` for silent navigation. |
| `talkbank.walker.loopCount` | integer | `1` | Number of times to loop each segment during walker playback. Range: 0--50. Set to `0` to loop indefinitely until the next step command. |
| `talkbank.walker.pauseSeconds` | number | `0` | Seconds to pause between segments during continuous playback or walker stepping. Range: 0--10. Set to `0` for no pause. |
| `talkbank.walker.walkLength` | integer | `0` | Number of utterances to play during continuous walker mode. Range: 0--100. Set to `0` to play all remaining segments to the end of the file. |
| `talkbank.media.defaultSpeed` | integer | `100` | Default playback speed as a percentage. Values: 25, 50, 75, 100, 125, 150, 175, 200. `100` = normal speed, `50` = half speed, `200` = double speed. The media panel toolbar also provides a speed slider for on-the-fly adjustment. |

## Display

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `talkbank.bullets.display` | string | `"dim"` | How timing bullets (`15320_18450`) are displayed in the editor. Options: `"dim"` (35% opacity -- the default, keeps bullets visible but unobtrusive), `"hidden"` (completely invisible -- useful when bullets are distracting during reading), `"normal"` (full visibility -- useful when editing bullet values). |
| `talkbank.inlayHints.enabled` | boolean | `true` | Show inlay hints for alignment count mismatches (e.g., `[alignment: 3 main <> 2 mor]`) and timing durations. Disable if you find the inline annotations distracting. |

## Validation

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `talkbank.validation.severity` | string | `"all"` | Filter which validation diagnostics are displayed in the editor. Options: `"all"` (show errors, warnings, and informational diagnostics), `"errorsOnly"` (show only errors -- useful for large files with many warnings), `"errorsAndWarnings"` (show errors and warnings, hide informational diagnostics). |

## Advanced

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `talkbank.lsp.binaryPath` | string | `""` (empty) | Absolute path to the standalone `chatter-lsp` binary. When empty (the default), the extension searches: (1) bundled binary at `<extension>/server/chatter-lsp[.exe]` (shipped with platform-specific VSIXes), (2) system PATH via `which chatter-lsp`, (3) dev-tree builds at `<extension>/../target/{debug,release}/chatter-lsp`. If none are found, an actionable error notification is shown. Set this only if you need to override the search. |

## Settings in JSON

To edit settings directly in `settings.json`, add entries like:

```json
{
  "talkbank.transcription.defaultSpeaker": "PAR",
  "talkbank.transcription.rewindSeconds": 3,
  "talkbank.walker.autoPlay": true,
  "talkbank.walker.loopCount": 2,
  "talkbank.media.defaultSpeed": 75,
  "talkbank.bullets.display": "hidden",
  "talkbank.inlayHints.enabled": false,
  "talkbank.validation.severity": "errorsOnly",
  "talkbank.lsp.binaryPath": "/usr/local/bin/chatter-lsp"
}
```

## Workspace vs. User Settings

All TalkBank settings can be set at either the **User** level (applies to all workspaces) or the **Workspace** level (applies only to the current workspace). Workspace settings override user settings.

This is useful when different corpora need different configurations. For example, a corpus where the primary speaker is the parent might have a workspace-level setting of `"talkbank.transcription.defaultSpeaker": "PAR"`, while your user-level default remains `"CHI"`.

## Related Chapters

- [Keyboard Shortcuts](keyboard-shortcuts.md) -- keybinding customization
- [Cache Management](cache.md) -- cache location and clearing
- [Troubleshooting: Common Issues](../troubleshooting/common-issues.md) -- resolving configuration problems
