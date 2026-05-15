/** Stable Tauri protocol contract for the dashboard desktop shell boundary.
 *
 * `lib/tauri.ts` owns the dynamic imports and browser fallbacks, but this
 * module owns the raw command/event identifiers and their paired transport
 * shapes so the shell/frontend seam stays inventoried in one place.
 */

/** Desktop-only user configuration mirrored from `~/.batchalign.ini`. */
export interface DesktopConfig {
  /** Selected ASR engine ("rev" or "whisper"). */
  engine: string;
  /** Optional Rev.AI API key persisted by the desktop setup flow. */
  rev_key: string | null;
}

/** Small acknowledgement payload returned by shell-side mutations. */
export interface DesktopCommandAck {
  /** Human-readable status message from the desktop shell. */
  message: string;
}

/** Minimal server-status payload returned by the desktop shell. */
export interface DesktopServerStatus {
  /** True when the managed batchalign server child is still alive. */
  running: boolean;
  /** Port the desktop shell manages for the local server. */
  port: number;
  /** Resolved `batchalign3` binary path, or null if unavailable. */
  binary_path: string | null;
  /** Child process ID when a managed server is running. */
  pid: number | null;
}

/** Event payload emitted when the desktop shell's server state changes. */
export interface DesktopServerStatusChangedEvent {
  /** Latest shell-side snapshot after one lifecycle transition. */
  status: DesktopServerStatus;
}

/** Default user config used when the browser has no desktop shell available. */
export const DEFAULT_DESKTOP_CONFIG: DesktopConfig = {
  engine: "whisper",
  rev_key: null,
};

/** Default status snapshot used when the browser has no desktop shell available. */
export const DEFAULT_DESKTOP_SERVER_STATUS: DesktopServerStatus = {
  running: false,
  port: 18000,
  binary_path: null,
  pid: null,
};

/** Compile-time inventory of Tauri command names and transport payloads. */
export interface DesktopCommandMap {
  discoverFiles: {
    name: "discover_files";
    request: { dir: string; extensions: string[] };
    response: string[];
  };
  getBatchalignPath: {
    name: "get_batchalign_path";
    request: void;
    response: string | null;
  };
  isFirstLaunch: {
    name: "is_first_launch";
    request: void;
    response: boolean;
  };
  readConfig: {
    name: "read_config";
    request: void;
    response: DesktopConfig;
  };
  writeConfig: {
    name: "write_config";
    request: { config: DesktopConfig };
    response: DesktopCommandAck;
  };
  serverStatus: {
    name: "server_status";
    request: void;
    response: DesktopServerStatus;
  };
  startServer: {
    name: "start_server";
    request: void;
    response: DesktopServerStatus;
  };
  stopServer: {
    name: "stop_server";
    request: void;
    response: DesktopServerStatus;
  };
}

/** Compile-time inventory of custom desktop events crossing the shell boundary. */
export interface DesktopEventMap {
  serverStatusChanged: {
    name: "desktop://server-status-changed";
    payload: DesktopServerStatusChangedEvent;
  };
}

/** Runtime command registry used by the low-level adapter. */
export const DESKTOP_COMMANDS = {
  discoverFiles: { name: "discover_files" },
  getBatchalignPath: { name: "get_batchalign_path" },
  isFirstLaunch: { name: "is_first_launch" },
  readConfig: { name: "read_config" },
  writeConfig: { name: "write_config" },
  serverStatus: { name: "server_status" },
  startServer: { name: "start_server" },
  stopServer: { name: "stop_server" },
} satisfies { [K in keyof DesktopCommandMap]: { name: DesktopCommandMap[K]["name"] } };

/** Runtime event registry used by the low-level adapter. */
export const DESKTOP_EVENTS = {
  serverStatusChanged: { name: "desktop://server-status-changed" },
} satisfies { [K in keyof DesktopEventMap]: { name: DesktopEventMap[K]["name"] } };

/** Valid keys for one known desktop command. */
export type DesktopCommandKey = keyof DesktopCommandMap;

/** Request payload type for one known desktop command. */
export type DesktopCommandRequest<K extends DesktopCommandKey> =
  DesktopCommandMap[K]["request"];

/** Response payload type for one known desktop command. */
export type DesktopCommandResponse<K extends DesktopCommandKey> =
  DesktopCommandMap[K]["response"];

/** Valid keys for one known desktop event. */
export type DesktopEventKey = keyof DesktopEventMap;

/** Payload type for one known desktop event. */
export type DesktopEventPayload<K extends DesktopEventKey> =
  DesktopEventMap[K]["payload"];
