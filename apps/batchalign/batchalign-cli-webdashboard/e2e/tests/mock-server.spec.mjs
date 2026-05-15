// affects: apps/batchalign/cli-web-statuspage/src/**
// affects: apps/batchalign/cli-web-statuspage/dist/**
import { createServer } from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { WebSocketServer } from "ws";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendRoot = path.resolve(__dirname, "..", "..");
const buildDir = path.join(frontendRoot, "dist");

const contentTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".svg", "image/svg+xml"],
  [".ico", "image/x-icon"],
  [".png", "image/png"],
]);

function withCors(headers = {}) {
  return {
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET,POST,DELETE,OPTIONS",
    "access-control-allow-headers": "content-type",
    ...headers,
  };
}

function makeFileStatus(filename, status, overrides = {}) {
  return {
    filename,
    status,
    error: null,
    error_category: null,
    error_codes: [],
    started_at: status === "done" ? 1 : null,
    finished_at: status === "done" ? 2 : null,
    progress_current: status === "processing" ? 1 : null,
    progress_total: status === "processing" ? 4 : null,
    progress_stage: status === "processing" ? "analyzing_morphosyntax" : null,
    progress_label: status === "processing" ? "Analyzing morphosyntax" : null,
    error_line: null,
    bug_report_id: null,
    ...overrides,
  };
}

function buildFailedJobFileStatuses() {
  const files = [];

  for (let index = 0; index < 42; index += 1) {
    const dir = index < 24 ? "alpha" : "beta";
    files.push(makeFileStatus(`${dir}/sample-${String(index).padStart(2, "0")}.cha`, "done"));
  }

  for (let index = 42; index < 54; index += 1) {
    const sample = String(index).padStart(2, "0");
    files.push(
      makeFileStatus(`beta/sample-${sample}.cha`, "error", {
        error: [
          `Validation mismatch for sample-${sample}.cha: parser lost speaker alignment while rebuilding the CHAT AST after retokenization.`,
          `Second line detail for sample-${sample}.cha explains the exact mismatch and preserves the full diagnostic payload.`,
        ].join("\n"),
        error_category: "validation",
        error_codes: ["E362"],
        error_line: 2,
        bug_report_id: `report-${sample}`,
      })
    );
  }

  files.push(
    makeFileStatus("gamma/sample-54.cha", "error", {
      error: "CHAT declaration is missing an @ID tier for speaker PAR.",
      error_category: "input",
      error_codes: ["E101"],
      error_line: 3,
    }),
    makeFileStatus("gamma/sample-55.cha", "error", {
      error: "CHAT declaration is missing an @ID tier for speaker PAR.",
      error_category: "input",
      error_codes: ["E101"],
      error_line: 3,
    }),
    makeFileStatus("gamma/sample-56.cha", "error", {
      error: "Referenced media file missing from media root.",
      error_category: "media",
      error_line: 1,
    }),
    makeFileStatus("gamma/sample-57.cha", "error", {
      error: "Referenced media file missing from media root.",
      error_category: "media",
      error_line: 1,
    }),
    makeFileStatus("gamma/sample-58.cha", "error", {
      error: "SQLite store returned a synthetic write failure while flushing the result bundle.",
      error_category: "system",
    }),
    makeFileStatus("gamma/sample-59.cha", "error", {
      error: "Morphosyntax worker exited unexpectedly after returning a malformed payload.",
      error_category: "processing",
    })
  );

  return files;
}

function makeJob(jobId, status, { completed, total, submittedAt, fileStatuses, error, sourceDir }) {
  const resolvedFileStatuses =
    fileStatuses ??
    [
      makeFileStatus(
        `${jobId}.cha`,
        status === "completed"
          ? "done"
          : status === "failed"
            ? "error"
            : status === "running"
              ? "processing"
              : "queued",
        {
          error:
            status === "failed"
              ? "synthetic failure while analyzing this file: tokenization and morphosyntax pipeline both reported inconsistent state for dashboard testing."
              : null,
          error_category: status === "failed" ? "processing" : null,
          error_codes: status === "failed" ? ["E_SYNTH"] : [],
          progress_current: completed,
          progress_total: total,
          error_line: status === "failed" ? 12 : null,
        }
      ),
    ];
  const completedFiles =
    completed ?? resolvedFileStatuses.filter((file) => file.status === "done").length;
  const totalFiles = total ?? resolvedFileStatuses.length;
  const errorFiles = resolvedFileStatuses.filter((file) => file.status === "error").length;

  return {
    job_id: jobId,
    status,
    command: "morphotag",
    options: {
      command: "morphotag",
      retokenize: false,
      skipmultilang: false,
      merge_abbrev: false,
      override_cache: false,
    },
    lang: "eng",
    source_dir: sourceDir ?? "/tmp/source",
    total_files: totalFiles,
    completed_files: completedFiles,
    error_files: errorFiles,
    submitted_at: submittedAt,
    completed_at:
      status === "completed" || status === "failed" ? "2026-02-25T00:10:00Z" : null,
    duration_s: status === "completed" || status === "failed" ? 2.4 : null,
    num_workers: 2,
    error: error ?? (status === "failed" ? "synthetic failure" : null),
    file_statuses: resolvedFileStatuses,
  };
}

function listItem(job) {
  return {
    job_id: job.job_id,
    status: job.status,
    command: job.command,
    lang: job.lang,
    source_dir: job.source_dir,
    total_files: job.total_files,
    completed_files: job.completed_files,
    error_files: job.error_files,
    submitted_at: job.submitted_at,
    completed_at: job.completed_at,
    duration_s: job.duration_s,
    num_workers: job.num_workers,
    submitted_by: "tester",
    submitted_by_name: "Test User",
  };
}

function json(res, status, payload) {
  const body = Buffer.from(JSON.stringify(payload), "utf-8");
  res.writeHead(status, withCors({ "content-type": "application/json; charset=utf-8" }));
  res.end(body);
}

async function staticFileResponse(pathname) {
  const cleaned = pathname === "/" ? "/index.html" : pathname;
  const relative = decodeURIComponent(cleaned).replace(/^\/+/, "");
  const fullPath = path.resolve(buildDir, relative);
  if (!fullPath.startsWith(buildDir)) {
    return { status: 403, body: Buffer.from("forbidden"), type: "text/plain; charset=utf-8" };
  }

  try {
    const body = await fs.readFile(fullPath);
    const type = contentTypes.get(path.extname(fullPath)) ?? "application/octet-stream";
    return { status: 200, body, type };
  } catch {
    const indexPath = path.join(buildDir, "index.html");
    const body = await fs.readFile(indexPath);
    return { status: 200, body, type: "text/html; charset=utf-8" };
  }
}

async function startHarness() {
  await fs.access(path.join(buildDir, "index.html"));

  const jobs = new Map([
    [
      "runjob",
      makeJob("runjob", "running", {
        completed: 1,
        total: 4,
        submittedAt: "2026-02-25T00:00:00Z",
      }),
    ],
    [
      "donejob",
      makeJob("donejob", "completed", {
        completed: 2,
        total: 2,
        submittedAt: "2026-02-25T00:05:00Z",
      }),
    ],
    [
      "failjob",
      makeJob("failjob", "failed", {
        submittedAt: "2026-02-25T00:08:00Z",
        fileStatuses: buildFailedJobFileStatuses(),
        error: "18 files failed validation and processing checks.",
        sourceDir: "/tmp/failure-case",
      }),
    ],
  ]);

  let baseUrl = "";
  const sockets = new Set();
  const serverSockets = new Set();

  const server = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", "http://localhost");

    if (req.method === "OPTIONS") {
      res.writeHead(204, withCors());
      res.end();
      return;
    }

    if (url.pathname === "/health" && req.method === "GET") {
      json(res, 200, {
        status: "ok",
        version: "test",
        capabilities: ["morphotag"],
        workers_available: 2,
      });
      return;
    }

    if (url.pathname === "/fleet" && req.method === "GET") {
      json(res, 200, [{ name: "local", url: baseUrl }]);
      return;
    }

    if (url.pathname === "/jobs" && req.method === "GET") {
      json(res, 200, Array.from(jobs.values()).map(listItem));
      return;
    }

    const cancelMatch = url.pathname.match(/^\/jobs\/([^/]+)\/cancel$/);
    if (cancelMatch && req.method === "POST") {
      const job = jobs.get(cancelMatch[1]);
      if (!job) {
        json(res, 404, { status: "not_found", message: "job not found" });
        return;
      }
      job.status = "cancelled";
      job.error = "cancelled by user";
      broadcast({ type: "job_update", job: listItem(job) });
      json(res, 200, { status: "ok", message: "cancelled" });
      return;
    }

    const restartMatch = url.pathname.match(/^\/jobs\/([^/]+)\/restart$/);
    if (restartMatch && req.method === "POST") {
      const job = jobs.get(restartMatch[1]);
      if (!job) {
        json(res, 404, { status: "not_found", message: "job not found" });
        return;
      }
      job.status = "queued";
      job.error = null;
      job.completed_files = 0;
      job.file_statuses = [
        {
          ...job.file_statuses[0],
          status: "queued",
          error: null,
          error_category: null,
          error_codes: [],
          progress_current: 0,
          progress_stage: null,
          progress_label: null,
        },
      ];
      broadcast({ type: "job_update", job: listItem(job) });
      json(res, 200, job);
      return;
    }

    const jobMatch = url.pathname.match(/^\/jobs\/([^/]+)$/);
    if (jobMatch && req.method === "GET") {
      const job = jobs.get(jobMatch[1]);
      if (!job) {
        json(res, 404, { status: "not_found", message: "job not found" });
        return;
      }
      json(res, 200, job);
      return;
    }

    if (jobMatch && req.method === "DELETE") {
      const existed = jobs.delete(jobMatch[1]);
      if (!existed) {
        json(res, 404, { status: "not_found", message: "job not found" });
        return;
      }
      broadcast({ type: "job_deleted", job_id: jobMatch[1] });
      json(res, 200, { status: "ok", message: "deleted" });
      return;
    }

    const file = await staticFileResponse(url.pathname);
    res.writeHead(file.status, withCors({ "content-type": file.type }));
    res.end(file.body);
  });

  server.on("connection", (socket) => {
    serverSockets.add(socket);
    socket.on("close", () => serverSockets.delete(socket));
  });

  const wsServer = new WebSocketServer({ noServer: true });

  function broadcast(event) {
    const payload = JSON.stringify(event);
    for (const ws of sockets) {
      if (ws.readyState === 1) {
        ws.send(payload);
      }
    }
  }

  server.on("upgrade", (req, socket, head) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    if (url.pathname !== "/ws") {
      socket.destroy();
      return;
    }
    wsServer.handleUpgrade(req, socket, head, (ws) => {
      wsServer.emit("connection", ws);
    });
  });

  wsServer.on("connection", (ws) => {
    sockets.add(ws);
    ws.on("close", () => sockets.delete(ws));
    ws.on("message", (raw) => {
      if (raw.toString() === "ping") {
        try {
          ws.send("pong");
        } catch {
          // Ignore races with socket close.
        }
      }
    });

    ws.send(
      JSON.stringify({
        type: "snapshot",
        jobs: Array.from(jobs.values()).map(listItem),
        health: { status: "ok" },
      })
    );
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve());
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("failed to resolve server address");
  }
  baseUrl = `http://127.0.0.1:${address.port}`;

  return {
    baseUrl,
    close: async () => {
      for (const ws of sockets) {
        try {
          ws.terminate();
        } catch {
          // Ignore races with already-closed sockets.
        }
      }
      sockets.clear();
      await new Promise((resolve) => wsServer.close(resolve));
      await new Promise((resolve, reject) => {
        server.close((err) => {
          if (err) {
            reject(err);
          } else {
            resolve();
          }
        });
        for (const socket of serverSockets) {
          socket.destroy();
        }
      });
    },
  };
}

async function installDesktopTauriStub(page, commandResponses) {
  await page.addInitScript((responses) => {
    const queuedResponses = new Map(
      Object.entries(responses).map(([command, value]) => [
        command,
        Array.isArray(value) ? [...value] : [value],
      ]),
    );
    const invokeCalls = [];
    const callbacks = new Map();
    const eventListeners = new Map();
    let nextCallbackId = 1;
    let nextEventId = 1;

    function cloneResponse(value) {
      if (value === null || value === undefined) {
        return value;
      }
      if (typeof value === "object") {
        return JSON.parse(JSON.stringify(value));
      }
      return value;
    }

    function consumeResponse(command) {
      const queue = queuedResponses.get(command);
      if (!queue || queue.length === 0) {
        throw new Error(`Unexpected Tauri invoke: ${command}`);
      }
      if (queue.length > 1) {
        return cloneResponse(queue.shift());
      }
      return cloneResponse(queue[0]);
    }

    function removeEventListener(event, eventId) {
      const listeners = eventListeners.get(event) ?? [];
      eventListeners.set(
        event,
        listeners.filter((listener) => listener.eventId !== eventId),
      );
    }

    window.__desktopInvokeCalls = invokeCalls;
    window.__emitDesktopEvent = (event, payload) => {
      for (const listener of eventListeners.get(event) ?? []) {
        const callback = callbacks.get(listener.callbackId);
        callback?.({ event, id: listener.eventId, payload });
      }
    };
    window.__TAURI_INTERNALS__ = {
      invoke(command, args) {
        invokeCalls.push({ command, args: args ?? null });
        if (command === "plugin:event|listen") {
          const eventId = nextEventId++;
          const listeners = eventListeners.get(args.event) ?? [];
          listeners.push({ eventId, callbackId: args.handler });
          eventListeners.set(args.event, listeners);
          return eventId;
        }
        if (command === "plugin:event|unlisten") {
          removeEventListener(args.event, args.eventId);
          return null;
        }
        return consumeResponse(command);
      },
      transformCallback(callback) {
        const id = nextCallbackId++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id) {
        callbacks.delete(id);
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(event, eventId) {
        removeEventListener(event, eventId);
      },
    };
  }, commandResponses);
}

async function readDesktopInvokeCalls(page) {
  return page.evaluate(() => window.__desktopInvokeCalls);
}

async function emitDesktopEvent(page, event, payload) {
  await page.evaluate(
    ({ name, eventPayload }) => {
      window.__emitDesktopEvent(name, eventPayload);
    },
    { name: event, eventPayload: payload },
  );
}

test("dashboard list/detail/actions work against the mock harness", async ({ page, request }) => {
  const harness = await startHarness();
  try {
    await page.goto(`${harness.baseUrl}/dashboard`, { waitUntil: "domcontentloaded" });

    const runningCard = page.locator('a[href="/dashboard/jobs/runjob"]');
    const completedCard = page.locator('a[href="/dashboard/jobs/donejob"]');

    await expect(runningCard).toBeVisible();
    await expect(completedCard).toBeVisible();

    await runningCard.click();
    await expect(page).toHaveURL(new RegExp(`${harness.baseUrl}/dashboard/jobs/runjob$`));
    await expect(page.getByText("Options")).toBeVisible();
    await expect(page.getByText("morphotag")).toBeVisible();
    await expect(page.getByRole("button", { name: "Cancel" })).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("button", { name: "Restart" })).toBeVisible();

    const cancelled = await request.get(`${harness.baseUrl}/jobs/runjob`);
    expect(cancelled.ok()).toBeTruthy();
    expect((await cancelled.json()).status).toBe("cancelled");

    await page.getByRole("button", { name: "Restart" }).click();
    await expect(page.getByRole("button", { name: "Cancel" })).toBeVisible();

    const restarted = await request.get(`${harness.baseUrl}/jobs/runjob`);
    expect(restarted.ok()).toBeTruthy();
    expect((await restarted.json()).status).toBe("queued");

    await page.goto(`${harness.baseUrl}/dashboard/jobs/donejob`, {
      waitUntil: "domcontentloaded",
    });
    await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();
    await page.getByRole("button", { name: "Delete" }).click();

    await expect(page).toHaveURL(new RegExp(`${harness.baseUrl}/dashboard/?$`));
    await expect(page.locator('a[href="/dashboard/jobs/donejob"]')).toHaveCount(0);

    const deleted = await request.get(`${harness.baseUrl}/jobs/donejob`);
    expect(deleted.status()).toBe(404);
  } finally {
    await harness.close();
  }
});

test("web runtime keeps root navigation on the browser dashboard surface", async ({ page }) => {
  const harness = await startHarness();
  try {
    await page.goto(harness.baseUrl, { waitUntil: "domcontentloaded" });

    await expect(page.locator('a[href="/dashboard/jobs/runjob"]')).toBeVisible();
    await expect(page.locator('a[href="/process"]')).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Dashboard", exact: true })).toBeVisible();
  } finally {
    await harness.close();
  }
});

test("desktop runtime gates first launch through the config seam", async ({ page }) => {
  const harness = await startHarness();
  try {
    await installDesktopTauriStub(page, {
      is_first_launch: true,
      server_status: {
        running: true,
        port: 18000,
        binary_path: "/usr/local/bin/batchalign3",
        pid: 4321,
      },
      write_config: { message: "saved" },
    });

    await page.goto(`${harness.baseUrl}/?server=${encodeURIComponent(harness.baseUrl)}`, {
      waitUntil: "domcontentloaded",
    });

    await expect(page.getByRole("heading", { name: "Welcome to Batchalign" })).toBeVisible();
    await page.getByRole("button", { name: "Let's Get Started" }).click();
    await page.getByRole("button", { name: "Finish Setup" }).click();
    await page.getByRole("button", { name: "Get Started" }).click();

    await expect(page).toHaveURL(/\/process(?:\?|$)/);
    await expect(page.getByText("What would you like to do?")).toBeVisible();

    const calls = await readDesktopInvokeCalls(page);
    expect(calls).toEqual(
      expect.arrayContaining([
        { command: "is_first_launch", args: {} },
        {
          command: "write_config",
          args: {
            config: {
              engine: "whisper",
              rev_key: null,
            },
          },
        },
      ]),
    );
  } finally {
    await harness.close();
  }
});

test("desktop process flow routes folder discovery through the file seam", async ({ page }) => {
  const harness = await startHarness();
  try {
    await installDesktopTauriStub(page, {
      is_first_launch: false,
      server_status: {
        running: true,
        port: 18000,
        binary_path: "/usr/local/bin/batchalign3",
        pid: 4321,
      },
      "plugin:dialog|open": ["/Users/test/input", "/Users/test/output"],
      discover_files: [[
        "/Users/test/input/alpha.cha",
        "/Users/test/input/nested/beta.cha",
      ]],
    });

    await page.goto(`${harness.baseUrl}/process?server=${encodeURIComponent(harness.baseUrl)}`, {
      waitUntil: "domcontentloaded",
    });

    await expect(page.getByText("What would you like to do?")).toBeVisible();
    await page.getByRole("button", { name: /Add Grammar/i }).click();
    await expect(page.getByRole("button", { name: /Click to choose a folder/i })).toBeVisible();
    await page.getByRole("button", { name: /Click to choose a folder/i }).click();
    await expect(page.getByText("2 files found")).toBeVisible();
    await expect(page.getByText("/Users/test/input")).toBeVisible();

    await expect(page.getByRole("button", { name: /Click to choose output folder/i })).toBeVisible();
    await page.getByRole("button", { name: /Click to choose output folder/i }).click();
    await expect(page.getByText("/Users/test/output")).toBeVisible();

    const calls = await readDesktopInvokeCalls(page);
    expect(calls).toEqual(
      expect.arrayContaining([
        {
          command: "plugin:dialog|open",
          args: {
            options: {
              directory: true,
              multiple: false,
              title: "Select Add Grammar input folder",
            },
          },
        },
        {
          command: "discover_files",
          args: {
            dir: "/Users/test/input",
            extensions: ["cha"],
          },
        },
        {
          command: "plugin:dialog|open",
          args: {
            options: {
              directory: true,
              multiple: false,
              title: "Choose output folder",
            },
          },
        },
      ]),
    );
  } finally {
    await harness.close();
  }
});

test("desktop server lifecycle subscribes to shell status events", async ({ page }) => {
  const harness = await startHarness();
  try {
    await installDesktopTauriStub(page, {
      is_first_launch: false,
      server_status: {
        running: false,
        port: 18000,
        binary_path: "/usr/local/bin/batchalign3",
        pid: null,
      },
      start_server: {
        running: true,
        port: 18000,
        binary_path: "/usr/local/bin/batchalign3",
        pid: 999,
      },
    });

    await page.goto(
      `${harness.baseUrl}/process?server=${encodeURIComponent("http://127.0.0.1:9")}`,
      { waitUntil: "domcontentloaded" },
    );

    await expect(page.getByText("Server starting...")).toBeVisible();

    const calls = await readDesktopInvokeCalls(page);
    expect(calls).toEqual(
      expect.arrayContaining([
        {
          command: "plugin:event|listen",
          args: expect.objectContaining({
            event: "desktop://server-status-changed",
          }),
        },
        { command: "start_server", args: {} },
      ]),
    );

    await emitDesktopEvent(page, "desktop://server-status-changed", {
      status: {
        running: false,
        port: 18000,
        binary_path: "/usr/local/bin/batchalign3",
        pid: null,
      },
    });

    await expect(page.getByText("Server stopped")).toBeVisible();
    await expect(page.getByRole("button", { name: "Start Server" })).toBeVisible();
  } finally {
    await harness.close();
  }
});

test("dashboard groups errors, paginates files, and filters failed job details", async ({
  page,
}) => {
  const harness = await startHarness();
  try {
    await page.goto(`${harness.baseUrl}/dashboard`, { waitUntil: "domcontentloaded" });

    const failedCard = page.locator('a[href="/dashboard/jobs/failjob"]');
    await expect(failedCard).toBeVisible();
    await failedCard.click();

    await expect(page).toHaveURL(new RegExp(`${harness.baseUrl}/dashboard/jobs/failjob$`));
    await expect(page.getByRole("button", { name: "Restart" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();
    await expect(page.getByText("18 files failed validation and processing checks.")).toBeVisible();
    await expect(page.getByText("18 errors")).toBeVisible();
    await expect(
      page.getByText(
        "This is a pipeline bug, not your input. A diagnostic report has been filed automatically."
      )
    ).toBeVisible();

    const validationGroup = page.locator("button").filter({ hasText: "E362" }).first();
    await validationGroup.click();
    await expect(page.getByRole("button", { name: "sample-42.cha", exact: true })).toBeVisible();

    await expect(page.getByText("Showing 1–50 of 60")).toBeVisible();
    await page.getByRole("button", { name: "Next" }).first().click();
    await expect(page.getByText("Showing 51–60 of 60")).toBeVisible();
    await expect(page.getByText("sample-58.cha")).toBeVisible();

    const search = page.getByPlaceholder("Search files...");
    await search.fill("SAMPLE-03");
    await expect(page.getByText("sample-03.cha")).toBeVisible();
    await expect(page.getByRole("cell", { name: "sample-58.cha", exact: true })).toHaveCount(0);

    await search.fill("");
    // Wait for the full file list to re-render after clearing the search
    // filter, so the Errors tab count is non-zero and the button is enabled.
    await expect(page.getByText("Showing 1–50 of 60")).toBeVisible();
    await page.getByRole("tab", { name: /Errors/ }).click();
    await expect(page.getByText("sample-03.cha")).toHaveCount(0);
    await expect(page.getByRole("cell", { name: "sample-52.cha", exact: true })).toBeVisible();

    await page.getByRole("button", { name: "sample-52.cha", exact: true }).click();
    await expect(
      page.locator("pre").filter({
        hasText:
          "Second line detail for sample-52.cha explains the exact mismatch and preserves the full diagnostic payload.",
      }),
    ).toBeVisible();
  } finally {
    await harness.close();
  }
});
