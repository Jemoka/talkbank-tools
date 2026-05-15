/** WebSocket client with auto-reconnect (exponential backoff).
 *
 * Supports connecting to multiple servers simultaneously via
 * `createMultiWS`. Each connection auto-reconnects independently.
 */

import type { WSMessage } from "./types";
import {
  controlPlaneHostPort,
  controlPlaneOrigin,
  controlPlaneWsScheme,
} from "./runtime";

type MessageHandler = (server: string, msg: WSMessage) => void;
type StatusHandler = (server: string, connected: boolean) => void;

const MIN_DELAY = 1000;
const MAX_DELAY = 30000;

function normalizeHostPort(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  try {
    if (trimmed.includes("://")) return new URL(trimmed).host;
    return trimmed;
  } catch {
    return null;
  }
}

/** Parse `?servers=` from the URL, always including the active control-plane host. */
export function getServerList(): string[] {
  const params = new URLSearchParams(window.location.search);
  const raw = params.get("servers") ?? "";
  const extra = raw
    .split(",")
    .map((s) => normalizeHostPort(s))
    .filter((s): s is string => Boolean(s));
  const primary = controlPlaneHostPort();
  const all = [primary, ...extra.filter((s) => s !== primary)];
  return all;
}

/**
 * Discover fleet servers from `GET /fleet` on the connected server.
 * Falls back to `getServerList()` if the endpoint is unavailable.
 */
export async function discoverFleet(): Promise<string[]> {
  const params = new URLSearchParams(window.location.search);
  if (params.get("servers")) {
    // Explicit servers in URL — don't override with fleet discovery
    return getServerList();
  }

  try {
    const resp = await fetch(`${controlPlaneOrigin()}/fleet`);
    if (!resp.ok) return getServerList();
    const fleet: Array<{ name: string; url: string }> = await resp.json();
    if (!fleet.length) return getServerList();

    // Extract host:port from each fleet URL, deduplicate with current control-plane host.
    const primary = controlPlaneHostPort();
    const fleetHosts = fleet
      .map((s) => normalizeHostPort(s.url))
      .filter((s): s is string => Boolean(s));
    const unique = [primary, ...fleetHosts.filter((h) => h !== primary)];
    return unique;
  } catch {
    return getServerList();
  }
}

/** Short hostname label (e.g. "<host>" from "<host>:8000"). */
export function serverLabel(hostPort: string): string {
  return hostPort.split(":")[0].split(".")[0];
}

function createSingleWS(
  hostPort: string,
  label: string,
  onMessage: MessageHandler,
  onStatus: StatusHandler
): { close: () => void } {
  let ws: WebSocket | null = null;
  let delay = MIN_DELAY;
  let closed = false;
  let pingInterval: ReturnType<typeof setInterval> | null = null;

  function connect() {
    if (closed) return;

    const proto = controlPlaneWsScheme();
    ws = new WebSocket(`${proto}//${hostPort}/ws`);

    ws.onopen = () => {
      delay = MIN_DELAY;
      onStatus(label, true);
      pingInterval = setInterval(() => {
        if (ws?.readyState === WebSocket.OPEN) ws.send("ping");
      }, 30000);
    };

    ws.onmessage = (ev) => {
      if (ev.data === "pong") return;
      try {
        const msg: WSMessage = JSON.parse(ev.data);
        onMessage(label, msg);
      } catch {
        /* ignore malformed messages */
      }
    };

    ws.onclose = () => {
      if (pingInterval) clearInterval(pingInterval);
      pingInterval = null;
      onStatus(label, false);
      if (!closed) {
        setTimeout(connect, delay);
        delay = Math.min(delay * 2, MAX_DELAY);
      }
    };

    ws.onerror = () => {
      ws?.close();
    };
  }

  connect();

  return {
    close() {
      closed = true;
      if (pingInterval) clearInterval(pingInterval);
      ws?.close();
    },
  };
}

/** Open one WebSocket per server in the list. */
export function createMultiWS(
  servers: string[],
  onMessage: MessageHandler,
  onStatus: StatusHandler
): { close: () => void } {
  const connections = servers.map((hp) =>
    createSingleWS(hp, serverLabel(hp), onMessage, onStatus)
  );

  return {
    close() {
      for (const c of connections) c.close();
    },
  };
}
