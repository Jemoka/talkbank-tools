// Tauri event payload contracts. These mirror the Rust side
// (`src-tauri/src/protocol.rs`); when one moves, the other must follow.
// The Rust integration tests serialize sample structs to JSON and the
// frontend tests deserialize them to catch drift early.

/**
 * Daemon → frontend events. The Rust sidecar manager (`daemon.rs`)
 * spawns the bundled `sidecar` PyApp binary, reads its stdout for
 * `DAEMON_PORT=<n>`, then proxies the daemon's SSE progress stream into
 * the Tauri event bus. The webview never opens HTTP sockets itself.
 */
export const TauriEvents = {
  /** Daemon has bound and printed its port. Payload: { port }. */
  daemonReady: "daemon-ready",
  /** Daemon could not be started or crashed before binding. */
  daemonFailed: "daemon-failed",
  /** One stdout/stderr line from the booting daemon — drives the boot
   *  overlay's status text so multi-minute cold installs don't look
   *  frozen. */
  daemonProgress: "daemon-progress",
  /**
   * A single `progress_v2` event from the daemon (one of StageStarted /
   * StageInjected / StageFailed / StageSkipped / SourceCompleted).
   * Tagged with `batchId` so the reducer routes it to the right tab.
   */
  progressV2: "progress-v2",
} as const;

export interface DaemonReadyPayload {
  port: number;
}
export interface DaemonFailedPayload {
  reason: string;
}
export interface DaemonProgressPayload {
  line: string;
}

/** ProgressEvent from python/batchalign/api.py:_event_to_dict. */
export type ProgressKind =
  | "StageStarted"
  | "StageInjected"
  | "StageFailed"
  | "StageSkipped"
  | "SourceCompleted";

/** Pipeline stages, matching `Task` enum on the Python side. */
export type ProgressTask =
  | "Asr"
  | "Fa"
  | "Speaker"
  | "UtSeg"
  | "Morphosyntax"
  | "Translate"
  | "Coref"
  | "Compare"
  | "OpenSmile"
  | "Avqi";

export interface ProgressEvent {
  source_id: string;
  kind: ProgressKind;
  task: ProgressTask | null;
  completed: number;
  total: number;
  label: string | null;
}

export interface ProgressV2Payload {
  batchId: string;
  jobId: string;
  event: ProgressEvent;
}
