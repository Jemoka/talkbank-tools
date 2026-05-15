/** Focused desktop capabilities exposed to the shared React tree.
 *
 * The Tauri adapter still owns raw IPC and plugin transport in `lib/tauri.ts`.
 * This module defines the narrower capability interfaces that hooks and
 * components should depend on so the desktop seam stays explicit and small.
 */

import type { DesktopConfig, DesktopServerStatus } from "./protocol";

/** Runtime-only capability: answers whether the UI runs in the desktop shell. */
export interface DesktopEnvironmentCapability {
  /** True when the current UI is running inside the Tauri desktop shell. */
  isDesktop: boolean;
}

/** File-system and native-dialog helpers needed by the process flow. */
export interface DesktopFilesCapability {
  /** Open the native folder picker. */
  pickFolder: (title?: string) => Promise<string | null>;
  /** Open the native output-folder picker. */
  pickOutputFolder: () => Promise<string | null>;
  /** Enumerate files inside one selected directory. */
  discoverFiles: (dir: string, extensions: string[]) => Promise<string[]>;
  /** Reveal one local path in the native shell. */
  openPath: (path: string) => Promise<void>;
}

/** Config persistence helpers needed by the setup flow. */
export interface DesktopConfigCapability {
  /** Detect whether the desktop shell should show the first-run wizard. */
  isFirstLaunch: () => Promise<boolean>;
  /** Read the desktop shell's persisted user configuration. */
  readConfig: () => Promise<DesktopConfig>;
  /** Persist one user-configuration update through the desktop shell. */
  writeConfig: (config: DesktopConfig) => Promise<void>;
}

/** Callback fired when the shell emits one server-status update. */
export type DesktopServerStatusListener = (status: DesktopServerStatus) => void;

/** Unsubscribe function returned by desktop event subscriptions. */
export type DesktopCapabilityUnlisten = () => void;

/** Server lifecycle helpers needed by the process flow. */
export interface DesktopServerCapability {
  /** Query the desktop shell's managed server status. */
  serverStatus: () => Promise<DesktopServerStatus>;
  /** Ask the desktop shell to start the local server process. */
  startServer: () => Promise<DesktopServerStatus>;
  /** Ask the desktop shell to stop the local server process. */
  stopServer: () => Promise<DesktopServerStatus>;
  /** Subscribe to shell-side server lifecycle changes. */
  onServerStatusChange: (
    listener: DesktopServerStatusListener,
  ) => Promise<DesktopCapabilityUnlisten>;
}
