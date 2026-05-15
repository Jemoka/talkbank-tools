# Desktop App Testing

**Status:** Current
**Last updated:** 2026-03-17

This document covers the testing strategy for the Chatter desktop app
(`apps/chatter/chatter-gui/`). Testing is split into three tiers by speed and scope.

## Testing Tiers

```text
┌─────────────────────────────────────────────────────────┐
│  Tier 3: E2E (WebdriverIO + tauri-driver)               │
│  Real app, real DOM, real IPC. Slow (~5-10s/test).       │
│  Catches: rendering bugs, IPC wiring, platform quirks.   │
│  Run: manually before releases, optionally in CI.        │
├─────────────────────────────────────────────────────────┤
│  Tier 2: Rust integration tests                          │
│  Real validation pipeline, real event bridge, no GUI.    │
│  Catches: serialization mismatches, event ordering,      │
│  stats consistency, single-file handling.                 │
│  Run: every commit, CI required.                         │
├─────────────────────────────────────────────────────────┤
│  Tier 1: Unit tests (Rust + TypeScript)                  │
│  Pure functions and thin runtime seams in isolation.     │
│  Catches: protocol drift, reducer bugs, CLAN math.       │
│  Run: every commit, CI required.                         │
└─────────────────────────────────────────────────────────┘
```

Most bugs will be caught by Tier 2. The Rust integration tests exercise the
exact same code path as the Tauri commands — they call
`validate_target_streaming()` and the frontend event bridge directly, then
verify the JSON shape, field names, event ordering, and stats consistency.

## Tier 1 & 2: Unit and integration tests

### Running

```bash
# TypeScript capability/seam tests
cd apps/chatter-gui && npm run test:unit

# Rust contract/integration tests
cargo nextest run -p chatter-gui --test validation_bridge
```

### What they cover

| Test | What it verifies |
|------|-----------------|
| `apps/chatter/chatter-gui/tests/unit/validationRunner.test.cjs` | Validation capability uses centralized command names, subscribes before invoke, and disposes listeners exactly once |
| `apps/chatter/chatter-gui/tests/unit/validationState.test.cjs` | Validation reducer computes relative file names and merges diagnostics/status immutably |
| `reference_corpus_no_hard_errors` | 74 reference files produce zero `Severity::Error` (warnings allowed) |
| `event_lifecycle_has_correct_sequence` | Discovering → Started → FileComplete×N → Finished ordering |
| `frontend_events_serialize_to_expected_json_shape` | Every event has `type` field; camelCase field names match TypeScript types; diagnostics include `renderedText` |
| `protocol_contracts_serialize_to_expected_json_shape` | Rust command/event constants and request payloads stay aligned with the TypeScript protocol module |
| `single_file_validation` | Single-file path validates exactly the selected file |
| `finished_stats_match_file_events` | `valid + invalid + parseErrors == totalFiles`; FileComplete count matches |
| `rendered_html_present_for_errors` | Every diagnostic carries non-empty miette HTML with box-drawing characters and `style=` attributes (ANSI colors converted to HTML) |

### Adding new tests

Test file: `apps/chatter/chatter-gui/src-tauri/tests/validation_bridge.rs`

The tests use `collect_events()` which runs the real validation pipeline and
collects all `FrontendEvent` values. To test a specific scenario:

```rust
#[test]
fn my_scenario() {
    let target = workspace_root().join("path/to/corpus");
    let events = collect_events(&target);
    let summary = summarize(&events);
    // assert on summary fields or individual events
}
```

### Miette rendering pipeline

Error rendering is server-side. Each `FrontendDiagnostic` carries two
renderings:

- **`rendered_html`** — `render_error_with_miette_with_source_colored()` produces
  ANSI-colored text, `ansi-to-html` converts it to HTML `<span style="...">`.
  The frontend displays it in a `<pre>` block via `dangerouslySetInnerHTML`.
  This guarantees identical output to the CLI.
- **`rendered_text`** — `render_error_with_miette_with_source()` produces plain
  text (no ANSI codes) for clean clipboard copy-paste.

The `rendered_html_present_for_errors` integration test verifies that every
error diagnostic includes non-empty HTML containing miette box-drawing
characters and `style=` attributes from ANSI color conversion.

### TypeScript seam tests

The TypeScript unit tests compile a focused subset of `apps/chatter/chatter-gui/src/` to a
temporary CommonJS directory, then run Node's built-in test runner against the
compiled output. This keeps the test toolchain small while still exercising the
runtime seam as real JavaScript.

- Runner script: `apps/chatter/chatter-gui/scripts/run-unit-tests.mjs`
- Compile config: `apps/chatter/chatter-gui/tsconfig.unit.json`
- Test files: `apps/chatter/chatter-gui/tests/unit/*.test.cjs`

### TypeScript ↔ Rust contract

The Rust integration tests verify that serialized JSON matches what the
TypeScript frontend expects. If you change a field name or event structure in
`events.rs`, the `frontend_events_serialize_to_expected_json_shape` test will
catch the mismatch before you discover it at runtime.

The key serde attributes:

- `#[serde(tag = "type", rename_all = "camelCase")]` on enums — variant names
  become camelCase tag values (`fileComplete`, not `FileComplete`)
- `#[serde(rename_all = "camelCase")]` on individual variants — field names
  become camelCase (`totalFiles`, not `total_files`)
- Both must be present: the enum-level `rename_all` only affects tag names,
  not field names within variants

## Tier 3: E2E Tests (WebdriverIO)

### Prerequisites

```bash
cargo install tauri-driver    # WebDriver backend for Tauri (Linux/Windows only)
cargo tauri build --debug     # Build the app binary
```

**Note:** `tauri-driver` only works on Linux and Windows. On macOS, WKWebView
does not support WebDriver. Run E2E tests in CI (Linux) or on a Windows machine.

### Running

```bash
# Terminal 1: start tauri-driver (WebDriver server on :4444)
tauri-driver

# Terminal 2: run the tests
cd desktop
npm run test:e2e
```

### What they cover

The smoke tests in `tests/e2e/smoke.spec.ts` verify that the app launches and
renders the expected UI elements:

- Drop zone with Choose File / Choose Folder buttons
- Empty file tree ("No files loaded")
- Empty error panel ("Select a file to view errors")
- Status bar showing "Ready"

### Limitations

**File dialogs cannot be driven via WebDriver.** The native file picker
(`@tauri-apps/plugin-dialog`) opens an OS-level dialog that WebDriver can't
interact with. Options for testing the validation flow:

1. **Test-only Tauri command** — add `validate_for_test(path)` behind
   `#[cfg(debug_assertions)]` that bypasses the file dialog
2. **Programmatic invoke** — use `driver.executeScript()` to call
    `window.__TAURI__.core.invoke("validate", { path })` directly
3. **Drag-and-drop simulation** — possible but platform-dependent and fragile

For now, the Rust integration tests cover the full validation pipeline. E2E
tests focus on UI rendering and user-visible layout.

### Adding E2E tests

Test file: `apps/chatter/chatter-gui/tests/e2e/*.spec.ts`

WebdriverIO provides `$()` and `$$()` for CSS selectors, plus Tauri-aware
capabilities:

```typescript
it("should show validation results", async () => {
  // Programmatically trigger validation (bypasses file dialog)
    await browser.executeAsync(async (path, done) => {
      await (window as any).__TAURI__.core.invoke("validate", {
        path,
      });
      // Wait for finished event
      setTimeout(done, 5000);
  }, "/path/to/corpus");

  const tree = await $(".file-tree-panel");
  const text = await tree.getText();
  expect(text).not.toContain("No files loaded");
});
```

### When to run E2E tests

- **Before releases** — manual run to verify the built app works end-to-end
- **Optionally in CI** — requires `tauri-driver` and a display server (Xvfb on
  Linux). Slow, so consider running only on release branches.
- **Not on every commit** — the Rust integration tests are fast and cover more
  ground

## Platform-Specific Considerations

| Platform | WebView engine | E2E support |
|----------|---------------|-------------|
| macOS | WKWebView | **Not supported** — `tauri-driver` does not work on macOS (WKWebView has no WebDriver API) |
| Windows | WebView2 (Chromium) | Full support via `tauri-driver` |
| Linux | WebKitGTK | Full support via `tauri-driver`; requires Xvfb for headless |

**macOS limitation:** Apple's WKWebView does not expose a WebDriver endpoint,
so `tauri-driver` cannot drive the app on macOS. E2E tests must run on Linux
(CI) or Windows. For local macOS development, rely on the Rust integration
tests (Tier 2) and manual smoke testing.

CSS rendering differs slightly between WebKit (Linux) and Chromium (Windows).
Visual regressions are possible — consider screenshot comparison tests if this
becomes a problem.

## Test Data

All tests use the reference corpus at `resources/corpus/reference/` (74 files). This
corpus is checked into the repo and must always pass validation. Two files
currently produce warnings (E534, E603) but zero hard errors.

Do not create ad-hoc `.cha` test files. Use existing reference corpus files
or ask the user to provide test data.

## CI Integration

Add to the existing CI workflow:

```yaml
# Rust integration tests (fast, always run)
- name: Desktop integration tests
  run: cargo nextest run -p chatter-gui --test validation_bridge

# E2E tests (slow, release branches only)
- name: Build desktop app
  if: startsWith(github.ref, 'refs/heads/release')
  run: cargo tauri build --debug
- name: E2E smoke tests
  if: startsWith(github.ref, 'refs/heads/release')
  run: |
    tauri-driver &
    sleep 2
    cd apps/chatter-gui && npm run test:e2e
```
