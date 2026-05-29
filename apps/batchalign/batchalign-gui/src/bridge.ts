// The one event loop. Registers a single Tauri listener per event channel
// and converts each payload into a discriminated store action. Nothing
// else in the app should call `listen()` — components subscribe to the
// store, the store mutates only via `dispatch()`, and Tauri events
// always flow through here.
//
// Lifecycle: `bootBridge()` is called once from `<App>` on mount. It
// returns an `unlisten()` that the component must invoke on unmount to
// drop the subscriptions. Idempotent — re-calling `bootBridge()` is a
// no-op (guarded by `booted`).

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  TauriEvents,
  type DaemonFailedPayload,
  type DaemonProgressPayload,
  type DaemonReadyPayload,
  type ProgressV2Payload,
} from "./protocol/events";
import { dispatch } from "./store";
import { fetchCapabilities, setBaseUrl } from "./api";

let booted = false;
let unlisteners: Array<() => void> = [];

export async function bootBridge(): Promise<() => void> {
  if (booted) return cleanup;
  booted = true;

  // Daemon-ready: store the port so api.ts can route HTTP requests, then
  // load /capabilities. The `daemon-ready` Tauri event is emitted by
  // src-tauri/src/daemon.rs once it parsed the "DAEMON_PORT=<n>" line.
  const offReady = await listen<DaemonReadyPayload>(
    TauriEvents.daemonReady,
    async ({ payload }) => {
      setBaseUrl(`http://127.0.0.1:${payload.port}`);
      dispatch({ type: "DAEMON_READY", port: payload.port });
      try {
        const caps = await fetchCapabilities();
        dispatch({ type: "CAPABILITIES_LOADED", capabilities: caps });
      } catch (err) {
        dispatch({
          type: "DAEMON_FAILED",
          reason: `capabilities fetch failed: ${err}`,
        });
      }
    },
  );
  unlisteners.push(offReady);

  // Daemon failed to start or crashed before binding.
  const offFailed = await listen<DaemonFailedPayload>(
    TauriEvents.daemonFailed,
    ({ payload }) => {
      dispatch({ type: "DAEMON_FAILED", reason: payload.reason });
    },
  );
  unlisteners.push(offFailed);

  // One stdout/stderr line from the booting sidecar — the boot overlay
  // surfaces the most recent line so cold installs (pip downloading
  // torch / transformers / stanza / ...) don't look frozen.
  const offProgressLine = await listen<DaemonProgressPayload>(
    TauriEvents.daemonProgress,
    ({ payload }) => {
      dispatch({ type: "DAEMON_PROGRESS", line: payload.line });
    },
  );
  unlisteners.push(offProgressLine);

  // Per-progress-event push from the sidecar. The Rust side owns one SSE
  // stream per active batch and tags every emit with `batchId` so the
  // reducer can route to the right tab.
  const offProgress = await listen<ProgressV2Payload>(
    TauriEvents.progressV2,
    ({ payload }) => {
      dispatch({
        type: "PROGRESS_V2",
        batchId: payload.batchId,
        event: payload.event,
      });
    },
  );
  unlisteners.push(offProgress);

  // Ask the Rust side to spawn the daemon now that listeners are armed.
  // If the daemon was already spawned by the setup() hook, this is a
  // no-op (the Rust command checks state.daemon.handle.load()).
  //
  // `ensure_daemon` polls for up to 5s and then returns Err with the
  // sentinel "daemon still starting; listen for `daemon-ready`". That
  // is NOT a failure — PyApp cold-starts routinely take 15–25s on a
  // fresh install, so the daemon may simply not have announced its
  // port yet. We swallow that specific sentinel and rely on the
  // `daemon-ready` / `daemon-failed` listeners (armed above) for the
  // authoritative resolution. Any other error IS a real failure.
  try {
    await invoke("ensure_daemon");
  } catch (err) {
    const msg = String(err);
    if (!msg.includes("daemon still starting")) {
      dispatch({
        type: "DAEMON_FAILED",
        reason: `ensure_daemon invoke failed: ${err}`,
      });
    }
  }

  return cleanup;
}

function cleanup() {
  for (const u of unlisteners) {
    try {
      u();
    } catch {
      // listener may already be dropped; ignore.
    }
  }
  unlisteners = [];
  booted = false;
}
