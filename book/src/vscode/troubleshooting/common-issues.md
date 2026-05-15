# Common Issues

**Status:** Current
**Last updated:** 2026-03-30 13:40 EDT

This chapter covers the most frequently encountered problems and their solutions.

## No Diagnostics Appearing

**Symptom:** You open a `.cha` file but see no error squiggles, no entries in the Problems panel, and no inlay hints.

**Possible causes and fixes:**

1. **The language server is not running.** Check the Output panel: click the dropdown and select "TalkBank Language Server". If there is no such entry, the server failed to start. See [LSP Connection](lsp.md) for troubleshooting.

2. **The file is not recognized as CHAT.** Check the language mode indicator in the bottom-right of the status bar. It should show "CHAT". If it shows "Plain Text" or another language, click on it and select "CHAT" from the list, or ensure the file has a `.cha` extension.

3. **Validation severity is set to filter out all diagnostics.** Check `talkbank.validation.severity` in your settings. If set to `"errorsOnly"`, warnings and information-level diagnostics are hidden. Try setting it to `"all"`.

4. **The file is actually valid.** If the file has no errors, no diagnostics will appear. This is correct behavior.

## Extension Not Activating

**Symptom:** Opening a `.cha` file does not trigger syntax highlighting or any extension features.

**Fixes:**

1. **Check the file extension.** The extension activates only on `.cha` files. Files with other extensions (`.txt`, `.chat`) will not trigger activation.

2. **Check that the extension is installed and enabled.** Open the Extensions sidebar (`Cmd+Shift+X`) and search for "TalkBank CHAT". The extension should show as installed and enabled.

3. **Check the development host.** If running via `code --extensionDevelopmentPath=.`, ensure you are running the command from the `apps/vscode-extension/` directory and that `npm run compile` has been run.

## Formatting Does Nothing

**Symptom:** Running Format Document (`Shift+Alt+F`) produces no changes.

**This is usually correct.** The formatter re-serializes the document through the canonical CHAT serializer. If the document is already in canonical form, no changes are needed and the formatter correctly reports no edits.

If you expected changes (e.g., fixing indentation):

1. Ensure the file has `@UTF8` and `@Begin` headers -- the formatter requires a structurally valid CHAT document.
2. Check that the language server is running (see above).

## Dependency Graph Not Rendering

**Symptom:** The dependency graph panel shows "Failed to load Graphviz renderer" or a blank panel.

**Fixes:**

1. **Check that `node_modules` is present.** The Graphviz WASM renderer (`@hpcc-js/wasm`) is bundled with the extension. If `node_modules/@hpcc-js/wasm/` is missing, run `npm install` in the `apps/vscode-extension/` directory.

2. **Ensure the utterance has a `%gra` tier.** The dependency graph requires a `%gra` (grammatical relations) tier on the current utterance. Position your cursor on an utterance that has `%gra` data and try again.

3. **Offline usage.** The Graphviz renderer works fully offline -- no internet connection is required. If it worked before and stopped, try reinstalling the extension.

## CLAN Integration Not Working

**Symptom:** "Open in CLAN" does nothing or shows an error.

**Platform support:**

| Platform | Status |
|----------|--------|
| macOS | Supported via Apple Events |
| Windows | Supported via Windows messaging |
| Linux | Not supported (CLAN is macOS/Windows only) |

**macOS fixes:**
- Ensure CLAN is installed and has been launched at least once
- The extension uses Apple Events IPC -- check that CLAN is not blocked by macOS security settings

**Windows fixes:**
- Ensure CLAN is installed and running
- The extension uses Windows messaging (`WM_APP`) -- CLAN must be running to receive the message

This feature is entirely optional. All editing, validation, and analysis features work without CLAN installed.

## Hover Shows No Alignment Data

**Symptom:** Hovering over a word on the main tier shows no cross-tier alignment information.

**Requirements for alignment:**

1. The utterance must have at least one dependent tier (`%mor`, `%gra`, `%pho`, or `%sin`)
2. Both the main tier and the dependent tier must be syntactically valid
3. The alignment counts must match (or the extension shows a mismatch hint instead)

If the main tier has words but no `%mor` tier is present, there is nothing to align.

## Scoped Find Returns No Results

**Symptom:** "Find in Tier" finds nothing even though you know the term exists.

**Check:**
- Are you searching the right tier? The scoped find restricts to the selected tier type only.
- Regex mode is activated by prefixing the query with `/`. If your search term contains `/`, it will be interpreted as a regex pattern.
- Speaker filtering is applied. If you selected specific speakers, results from other speakers are excluded.

## Related Chapters

- [LSP Connection](lsp.md) -- language server startup and debugging
- [Media Not Found](media.md) -- media playback issues
- [Settings Reference](../configuration/settings.md) -- all configurable settings
