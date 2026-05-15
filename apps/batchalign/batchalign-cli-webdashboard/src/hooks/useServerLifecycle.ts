/** Manage the batchalign3 server lifecycle from the desktop app.
 *
 * In desktop mode (Tauri), this hook subscribes to shell-emitted server status
 * updates, reconciles an initial shell snapshot, and auto-starts the managed
 * local server when the binary is present but not yet running. It then combines
 * that shell-owned process state with HTTP health checks to expose a small UI
 * lifecycle model.
 *
 * In web mode, this hook is a no-op — the server is assumed to be externally
 * managed.
 */

import { useCallback, useEffect, useState } from "react";
import type { DesktopServerStatus } from "../desktop/protocol";
import {
  useDesktopEnvironment,
  useDesktopServer,
} from "../desktop/DesktopContext";
import { useServerHealth } from "./useServerHealth";

export type ServerLifecycleStatus =
  | "starting"     // Server process spawned, waiting for health
  | "running"      // Health endpoint responding
  | "stopped"      // Process not running (user stopped or crashed)
  | "not-found"    // batchalign3 binary not on PATH
  | "external";    // Web mode — server managed externally

export interface ServerLifecycleState {
  /** Current lifecycle status. */
  status: ServerLifecycleStatus;
  /** Whether the server is ready to accept jobs. */
  isReady: boolean;
  /** Start the server manually (desktop only). */
  start: () => Promise<void>;
  /** Stop the server manually (desktop only). */
  stop: () => Promise<void>;
  /** Last error message from start/stop attempts. */
  error: string | null;
  /** Underlying health state for detailed info. */
  health: ReturnType<typeof useServerHealth>;
}

export function useServerLifecycle(): ServerLifecycleState {
  const environment = useDesktopEnvironment();
  const server = useDesktopServer();
  const health = useServerHealth();
  const [shellStatus, setShellStatus] = useState<DesktopServerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!environment.isDesktop) {
      setShellStatus(null);
      setError(null);
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    async function connectToShell(): Promise<void> {
      try {
        unlisten = await server.onServerStatusChange((nextStatus) => {
          if (!cancelled) {
            setShellStatus(nextStatus);
          }
        });
        if (cancelled) return;

        const status = await server.serverStatus();
        if (cancelled) return;
        setShellStatus(status);

        if (!status.running && status.binary_path) {
          const started = await server.startServer();
          if (!cancelled) {
            setShellStatus(started);
          }
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
        }
      }
    }

    void connectToShell();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [environment, server]);

  const binaryMissing = environment.isDesktop && shellStatus?.binary_path === null;

  let status: ServerLifecycleStatus;
  if (!environment.isDesktop) {
    status = "external";
  } else if (binaryMissing) {
    status = "not-found";
  } else if (health.isReachable) {
    status = "running";
  } else if (shellStatus?.running) {
    status = "starting";
  } else {
    status = "stopped";
  }

  const start = useCallback(async () => {
    if (!environment.isDesktop) return;
    setError(null);
    try {
      const nextStatus = await server.startServer();
      setShellStatus(nextStatus);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [environment, server]);

  const stop = useCallback(async () => {
    if (!environment.isDesktop) return;
    setError(null);
    try {
      const nextStatus = await server.stopServer();
      setShellStatus(nextStatus);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [environment, server]);

  return {
    status,
    isReady: health.isReachable,
    start,
    stop,
    error,
    health,
  };
}
