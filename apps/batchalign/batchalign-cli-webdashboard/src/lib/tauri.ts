/** Low-level Tauri adapter for the shared desktop runtime seam.
 *
 * React components and hooks consume focused capability contexts from
 * `desktop/DesktopContext.tsx`. This module stays lower level: it owns the
 * dynamic Tauri imports, shell/browser fallbacks, and the raw protocol
 * registry from `desktop/protocol.ts`.
 */

import { isDesktop } from "../runtime";
import {
  DESKTOP_COMMANDS,
  DESKTOP_EVENTS,
  DEFAULT_DESKTOP_CONFIG,
  DEFAULT_DESKTOP_SERVER_STATUS,
  type DesktopCommandKey,
  type DesktopCommandRequest,
  type DesktopCommandResponse,
  type DesktopConfig,
  type DesktopEventKey,
  type DesktopEventPayload,
  type DesktopServerStatus,
} from "../desktop/protocol";

type DesktopCommandArgs<K extends DesktopCommandKey> =
  DesktopCommandRequest<K> extends void ? [] : [DesktopCommandRequest<K>];

function desktopOnlyError(action: string): Error {
  return new Error(`${action} requires the desktop app`);
}

/** Invoke one known desktop command through a dynamic import. */
async function tauriInvoke<K extends DesktopCommandKey>(
  command: K,
  ...args: DesktopCommandArgs<K>
): Promise<DesktopCommandResponse<K>> {
  const { invoke } = await import("@tauri-apps/api/core");
  const payload = args[0] as Record<string, unknown> | undefined;
  return invoke<DesktopCommandResponse<K>>(DESKTOP_COMMANDS[command].name, payload);
}

/** Subscribe to one known desktop event through a dynamic import. */
async function tauriListen<K extends DesktopEventKey>(
  event: K,
  onEvent: (payload: DesktopEventPayload<K>) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<DesktopEventPayload<K>>(
    DESKTOP_EVENTS[event].name,
    ({ payload }) => onEvent(payload),
  );
  return () => {
    void unlisten();
  };
}

/** Open a native folder picker dialog. Returns the selected path or null if cancelled. */
export async function pickFolder(title?: string): Promise<string | null> {
  if (!isDesktop()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({
    directory: true,
    multiple: false,
    title: title ?? "Choose a folder",
  });
  // open() returns string | string[] | null for directory picks
  if (Array.isArray(result)) return result[0] ?? null;
  return result;
}

/** Open a native folder picker for the output destination. */
export async function pickOutputFolder(): Promise<string | null> {
  return pickFolder("Choose output folder");
}

/** Invoke the Tauri `discover_files` command to enumerate files in a directory. */
export async function discoverFiles(
  dir: string,
  extensions: string[],
): Promise<string[]> {
  if (!isDesktop()) {
    throw desktopOnlyError("discoverFiles");
  }
  return tauriInvoke("discoverFiles", { dir, extensions });
}

/** Open a path in the native file manager (Finder, Explorer, etc.). */
export async function openPath(path: string): Promise<void> {
  if (!isDesktop()) {
    // Best-effort fallback — won't work for local paths in browsers
    return;
  }
  const { open } = await import("@tauri-apps/plugin-shell");
  await open(path);
}

/** Check whether the desktop shell should show the first-run setup wizard. */
export async function isFirstLaunch(): Promise<boolean> {
  if (!isDesktop()) return false;
  try {
    return await tauriInvoke("isFirstLaunch");
  } catch {
    return false;
  }
}

/** Read the desktop shell's persisted user configuration. */
export async function readConfig(): Promise<DesktopConfig> {
  if (!isDesktop()) return { ...DEFAULT_DESKTOP_CONFIG };
  try {
    return await tauriInvoke("readConfig");
  } catch {
    return { ...DEFAULT_DESKTOP_CONFIG };
  }
}

/** Persist one desktop user-configuration update. */
export async function writeConfig(config: DesktopConfig): Promise<void> {
  if (!isDesktop()) {
    throw desktopOnlyError("writeConfig");
  }
  await tauriInvoke("writeConfig", { config });
}

/** Resolve the local `batchalign3` binary path from the desktop shell. */
export async function getBatchalignPath(): Promise<string | null> {
  if (!isDesktop()) return null;
  return tauriInvoke("getBatchalignPath");
}

/** Query the desktop shell's managed server status. */
export async function serverStatus(): Promise<DesktopServerStatus> {
  if (!isDesktop()) {
    return { ...DEFAULT_DESKTOP_SERVER_STATUS };
  }
  return tauriInvoke("serverStatus");
}

/** Ask the desktop shell to start the managed local server. */
export async function startServer(): Promise<DesktopServerStatus> {
  if (!isDesktop()) {
    throw desktopOnlyError("startServer");
  }
  return tauriInvoke("startServer");
}

/** Ask the desktop shell to stop the managed local server. */
export async function stopServer(): Promise<DesktopServerStatus> {
  if (!isDesktop()) {
    throw desktopOnlyError("stopServer");
  }
  return tauriInvoke("stopServer");
}

/** Subscribe to shell-emitted server status snapshots. */
export async function onServerStatusChange(
  listener: (status: DesktopServerStatus) => void,
): Promise<() => void> {
  if (!isDesktop()) {
    return () => {};
  }
  return tauriListen("serverStatusChanged", ({ status }) => {
    listener(status);
  });
}
