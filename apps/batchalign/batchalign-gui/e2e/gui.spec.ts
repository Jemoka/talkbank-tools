// End-to-end GUI smoke test.
//
// Runs the React frontend (vite dev) inside a real Chromium against a
// real PyApp-bundled sidecar daemon. Tauri host APIs are stubbed (see
// tauri-stubs.ts) so the same frontend code that ships in the Tauri
// shell runs unmodified in the browser, with the stubs forwarding to
// the same daemon the production app would talk to.
//
// What this covers:
//   - app boots and finishes the daemon handshake
//   - capabilities loads with real recipe + backend names
//   - folder picker → BATCH_OPENED → file table populated
//   - settings view opens
//   - "start batch" submits a real recipe (or surfaces a daemon error
//     in a controlled, observable way — we accept either as long as
//     the request was issued and the UI reacted)

import { test, expect } from "@playwright/test";
import { spawn, ChildProcess } from "node:child_process";
import { mkdtempSync, copyFileSync, readFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, "..", "..", "..", "..");
const SIDECAR = join(REPO_ROOT, "bazel-bin", "python", "batchalign", "sidecar");

let sidecar: ChildProcess | null = null;
let daemonPort = 0;
let fixturesDir = "";

test.beforeAll(async () => {
  // Stage two small audio fixtures into a temp dir so the "files"
  // block has stable rows to display.
  fixturesDir = mkdtempSync(join(tmpdir(), "batchalign-e2e-"));
  const fixtureSrc = join(__dirname, "..", "fixtures");
  copyFileSync(join(fixtureSrc, "test1.wav"), join(fixturesDir, "test1.wav"));
  copyFileSync(join(fixtureSrc, "test2.wav"), join(fixturesDir, "test2.wav"));

  // Boot the real daemon. PyApp first-launch may take ~15s to unpack
  // CPython on a cold machine; we allow up to 30s.
  sidecar = spawn(
    SIDECAR,
    ["--port", "0", "--host", "127.0.0.1", "--log-level", "info", "--no-access-log"],
    { env: { ...process.env, BATCHALIGN_API_ALLOW_PATHS: "1" } },
  );

  daemonPort = await new Promise<number>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("daemon did not announce port within 60s")),
      60_000,
    );
    sidecar!.stdout!.on("data", (chunk: Buffer) => {
      const text = chunk.toString();
      const m = text.match(/DAEMON_PORT=(\d+)/);
      if (m) {
        clearTimeout(timer);
        resolve(Number(m[1]));
      }
    });
    sidecar!.stderr!.on("data", () => {});
    sidecar!.on("exit", (code) =>
      reject(new Error(`daemon exited (code=${code}) before announcing port`)),
    );
  });
});

test.afterAll(() => {
  if (sidecar) {
    sidecar.kill("SIGTERM");
  }
});

test("Batchalign GUI end-to-end (real daemon)", async ({ page }) => {
  test.setTimeout(120_000);

  // Build the fixture file list once so the stub returns deterministic
  // rows regardless of the user's filesystem.
  const files = [
    { name: "test1.wav", size: statSync(join(fixturesDir, "test1.wav")).size },
    { name: "test2.wav", size: statSync(join(fixturesDir, "test2.wav")).size },
  ].map((f) => ({
    source_id: f.name,
    stem: f.name.replace(/\.wav$/, ""),
    filename: f.name,
    size_bytes: f.size,
    duration_ms: null,
    kind: "media" as const,
  }));

  // Inject the Tauri stubs before any frontend code runs. The init
  // script also seeds the three e2e globals the stubs need.
  await page.addInitScript(
    ({ port, folder, fileList }) => {
      // @ts-expect-error: window typed strictly elsewhere
      window.__E2E_DAEMON_PORT__ = port;
      // @ts-expect-error
      window.__E2E_FOLDER__ = folder;
      // @ts-expect-error
      window.__E2E_FILES__ = fileList;
    },
    { port: daemonPort, folder: fixturesDir, fileList: files },
  );
  await page.addInitScript({ content: stubBundle() });

  // Surface frontend console errors so a broken bridge.ts fails the
  // test rather than silently rendering an empty shell.
  const consoleErrors: string[] = [];
  page.on("pageerror", (err) => consoleErrors.push(`pageerror: ${err.message}`));
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(`console.error: ${msg.text()}`);
    }
  });

  await page.goto("http://localhost:1421/");

  // --- 1. Cold-launch: daemon spawn + capabilities ----------------
  // While booting, the DaemonBootOverlay covers the app and captures
  // pointer events. In this test the daemon is already up (spawned
  // in beforeAll), so the overlay clears almost immediately — we
  // assert on the ready state directly rather than trying to catch
  // the overlay mid-flight.
  await expect(page.locator("text=open folder…")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator("text=warming up the engine")).toBeHidden({
    timeout: 15_000,
  });

  // /capabilities should populate the React store. We can't peek inside
  // zustand from outside without a hook, so verify indirectly: the dev
  // toolbar/header drops the "starting daemon…" subtitle when ready.
  // (Also implicitly covered by the overlay's `warming up` text being
  // hidden — both gate on daemon.ready && capabilities != null.)
  await expect(page.locator("text=starting daemon…")).toBeHidden({
    timeout: 15_000,
  });

  // --- 2. Open folder → BatchView mounts, placeholder visible ---
  // After BATCH_OPENED dispatches the right pane is a placeholder (no
  // pipeline picked yet, so the daemon would process nothing). The
  // empty-pipeline hint in PipelineBlock guides the user to add a step.
  await page.click("text=open folder…");
  await expect(page.locator("text=no pipeline steps yet")).toBeVisible({
    timeout: 5_000,
  });

  // --- 3. Add a pipeline step → file table populates -------------
  await page.click("button:has-text('+ add step')");
  await page.click("text=transcribe");
  // FileTable column shows file *stem* (no extension).
  await expect(page.locator(`text=${files[0].stem}`).first()).toBeVisible({
    timeout: 5_000,
  });
  await expect(page.locator(`text=${files[1].stem}`).first()).toBeVisible();

  // --- 4. Settings view opens ------------------------------------
  await page.click("button:has-text('settings')");
  await expect(page.locator("text=workers").first()).toBeVisible({
    timeout: 3_000,
  });
  await page.click("button:has-text('close')");

  // --- 5. Start batch → request is issued + UI reacts -----------
  const requestPromise = page.waitForRequest(
    (req) =>
      req.url().includes(`/recipes/transcribe`) && req.method() === "POST",
    { timeout: 20_000 },
  );
  await page.click("button:has-text('start batch')");
  const request = await requestPromise;
  expect(request).toBeTruthy();
  // The request body should carry the inputs + asr_backend kwargs the
  // FilesBlock.tsx mapper produces.
  const body = JSON.parse(request.postData() || "{}");
  expect(body).toHaveProperty("inputs");
  expect(body.inputs).toHaveLength(2);
  expect(body).toHaveProperty("asr_backend");
  expect(body.asr_backend).toHaveProperty("kind");

  // Allow any cascading store mutation to settle, then assert no
  // UNEXPECTED page errors surfaced during the whole run. We tolerate:
  //   - "start_batch failed" / "start transcribe:" — FilesBlock.tsx's
  //     catch logs these when the daemon returns 4xx (expected: there
  //     are no whisper models in the test env)
  //   - "Failed to load resource: 4xx" — browser console line for the
  //     same daemon 4xx response
  //   - "capabilities fetch failed" — only if it slipped through
  await page.waitForTimeout(500);
  const fatalErrors = consoleErrors.filter(
    (e) =>
      !/start_batch failed/.test(e) &&
      !/start transcribe:/.test(e) &&
      !/capabilities fetch failed/.test(e) &&
      !/Failed to load resource: the server responded with a status of 4/.test(e),
  );
  expect(fatalErrors).toEqual([]);

  // Screenshot the final state for visual review.
  await page.screenshot({ path: "e2e-final.png", fullPage: true });
});

test("DaemonBootOverlay blocks interaction during boot", async ({ page }) => {
  test.setTimeout(30_000);

  // Inject a stub variant that NEVER fires daemon-ready, so the overlay
  // is visible the entire run. Same shape as the main stubs but with
  // `ensure_daemon` returning the port without emitting the event.
  await page.addInitScript({
    content: `
      window.__E2E_DAEMON_PORT__ = 1;
      window.__E2E_FOLDER__ = '/tmp/never-opened';
      window.__E2E_FILES__ = [];
      (function() {
        const listeners = new Map();
        const callbacks = new Map();
        let nextCallbackId = 0;
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            switch (cmd) {
              case 'plugin:event|listen': {
                const id = (args && args.handler) || 0;
                const evt = (args && args.event) || '';
                const cb = callbacks.get(id);
                if (cb) {
                  const list = listeners.get(evt) || [];
                  list.push((payload) => cb({ event: evt, payload, id }));
                  listeners.set(evt, list);
                }
                return id;
              }
              case 'plugin:event|unlisten': return null;
              case 'ensure_daemon':
              case 'daemon_port':
                // Deliberately do NOT emit daemon-ready — keep the
                // overlay visible.
                return 1;
              default: return null;
            }
          },
          transformCallback: (cb) => {
            const id = ++nextCallbackId;
            callbacks.set(id, cb);
            return id;
          },
          metadata: { plugins: {} },
        };
        window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
          unregisterListener: () => {},
        };
      })();
    `,
  });

  await page.goto("http://localhost:1421/");

  // The overlay should be visible and stay visible.
  await expect(page.locator("text=warming up the engine")).toBeVisible({
    timeout: 5_000,
  });
  await page.screenshot({ path: "e2e-boot-overlay.png", fullPage: true });

  // The overlay must capture pointer events so the underlying
  // EmptyView CTA is unreachable. Use elementFromPoint at the center
  // of the viewport — where the dropzone "open folder…" button sits —
  // and assert the topmost element is the overlay (or one of its
  // descendants), not anything inside EmptyView.
  const topmost = await page.evaluate(() => {
    const w = window.innerWidth / 2;
    const h = window.innerHeight / 2;
    const el = document.elementFromPoint(w, h);
    if (!el) return { tag: "(none)", insideOverlay: false };
    const overlay = el.closest('[role="dialog"][aria-busy="true"]');
    return { tag: el.tagName, insideOverlay: overlay != null };
  });
  expect(topmost.insideOverlay).toBe(true);
});

/// Inline-load the plain-JS stubs file and return its content so we
/// can hand it to `page.addInitScript({ content: ... })`. The stubs
/// are written as a self-invoking IIFE in JS (not TS) so the browser
/// can parse them directly — no transpile step needed.
function stubBundle(): string {
  return readFileSync(join(__dirname, "tauri-stubs.js"), "utf-8");
}
