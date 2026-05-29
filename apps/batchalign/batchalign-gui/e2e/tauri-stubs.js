// Tauri API stubs — plain JS injected via page.addInitScript before
// any frontend module loads. The Vite-served bridge.ts pulls
// `@tauri-apps/api/{core,event}`, which read window.__TAURI_INTERNALS__
// at import time; if the global isn't there, the API throws and the
// React tree fails to mount. So we install the shim synchronously here.
//
// Globals expected on `window` (seeded by a sibling addInitScript):
//   __E2E_DAEMON_PORT__ : number, sidecar HTTP port
//   __E2E_FOLDER__      : string, fake "picked" folder path
//   __E2E_FILES__       : Array<{ source_id, stem, filename, size_bytes, duration_ms }>

(function installTauriStubs() {
  const listeners = new Map();
  function emit(event, payload) {
    const list = listeners.get(event) || [];
    for (const fn of list) fn(payload);
  }
  window.__E2E_EVENT_LISTENERS__ = listeners;
  window.__E2E_EMIT__ = emit;

  const callbacks = new Map();
  let nextCallbackId = 0;

  function port() {
    return window.__E2E_DAEMON_PORT__;
  }

  async function realInvoke(cmd, args) {
    switch (cmd) {
      case "ensure_daemon":
      case "daemon_port": {
        const p = port();
        // Fire daemon-ready BEFORE invoke resolves so the listener
        // (registered earlier by bridge.ts in the same call sequence)
        // sees it. Using setTimeout would let React's StrictMode tear
        // down the listener before the event lands.
        emit("daemon-ready", { port: p });
        return p;
      }
      case "list_folder_files":
        return { files: window.__E2E_FILES__ };
      case "daemon_request": {
        // In the browser test harness we run on http://localhost:1421
        // and CAN reach 127.0.0.1:<daemon_port> via window.fetch (no
        // WebKit ATS restriction). So the e2e routes daemon_request
        // through fetch instead of through Rust — same wire format.
        const method = (args && args.method) || "GET";
        const path = (args && args.path) || "/";
        const body = args && args.body;
        const url = `http://127.0.0.1:${port()}${path}`;
        const init = { method };
        if (body !== null && body !== undefined) {
          init.headers = { "content-type": "application/json" };
          init.body = JSON.stringify(body);
        }
        const resp = await fetch(url, init);
        if (!resp.ok) {
          const text = await resp.text().catch(() => "");
          throw new Error(`${method} ${path} → ${resp.status}: ${text}`);
        }
        if (resp.status === 204) return null;
        const text = await resp.text();
        return text ? JSON.parse(text) : null;
      }
      case "reveal_in_file_manager":
        return null;
      case "start_batch_pump": {
        const jobId = args && args.jobId;
        const batchId = args && args.batchId;
        if (!jobId || !batchId) return null;
        const es = new EventSource(
          `http://127.0.0.1:${port()}/jobs/${jobId}/events`,
        );
        es.addEventListener("progress", (e) => {
          try {
            const parsed = JSON.parse(e.data);
            emit("progress-v2", { batchId, jobId, event: parsed });
          } catch (err) {
            console.warn("e2e: bad progress payload", err);
          }
        });
        es.addEventListener("done", () => es.close());
        es.onerror = () => es.close();
        return null;
      }
      default:
        throw new Error(`e2e: unknown invoke '${cmd}'`);
    }
  }

  function transformCallback(cb) {
    const id = ++nextCallbackId;
    callbacks.set(id, cb);
    return id;
  }

  async function pluginListen(args) {
    const event = args.event;
    const handler = args.handler;
    const cb = callbacks.get(handler);
    if (!cb) return handler;
    const list = listeners.get(event) || [];
    const wrapper = (payload) => cb({ event, payload, id: handler });
    wrapper.__listenerId = handler;
    list.push(wrapper);
    listeners.set(event, list);
    return handler;
  }

  // The real @tauri-apps/api/event uses Channel-based listeners in v2:
  // listen() calls invoke('plugin:event|listen', { event, target, handler })
  // where handler is a transformCallback id. We forward the payload through
  // the callback registry.
  // The event plugin's _unlisten path reads
  // __TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, id) BEFORE
  // it awaits the invoke('plugin:event|unlisten') call. We stub it so
  // React-StrictMode-induced cleanup doesn't pageerror in the e2e run.
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event, id) => {
      const list = listeners.get(event);
      if (!list) return;
      // Listeners are wrapper closures; we can't match by id directly
      // because wrappers don't carry the id. So we clear the wrapper
      // whose underlying callback id matches. The wrapper closes over
      // its handler id (see pluginListen) — store the id with it.
      const next = list.filter((fn) => fn.__listenerId !== id);
      listeners.set(event, next);
      callbacks.delete(id);
    },
  };

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      switch (cmd) {
        case "plugin:event|listen":
          return pluginListen(args || {});
        case "plugin:event|unlisten":
          return null;
        case "plugin:event|emit":
          // The frontend doesn't emit; the daemon does.
          return null;
        case "plugin:dialog|open":
          return window.__E2E_FOLDER__;
        case "plugin:opener|open":
        case "plugin:opener|reveal_item_in_dir":
        case "plugin:fs|read_dir":
        case "plugin:fs|read_text_file":
          return null;
        default:
          return realInvoke(cmd, args);
      }
    },
    transformCallback,
    metadata: { plugins: {} },
  };
})();
