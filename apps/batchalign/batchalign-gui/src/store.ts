// The single zustand store + reducer for the entire app.
//
// Design rules (see /Users/houjun/.claude/plans/atomic-chasing-castle.md §3):
//   1. One reducer; every mutation is a discriminated `Action`.
//   2. One store; no per-tab state lives in components.
//   3. Components subscribe via `useStore(selector)`; nothing calls
//      `listen()` directly — that's `bridge.ts`'s job.

import { create } from "zustand";
import type { ProgressEvent, ProgressTask } from "./protocol/events";

export type VerbStep =
  | "transcribe"
  | "align"
  | "morphotag"
  | "translate"
  | "compare";

/** Verb → daemon task (used to route ProgressEvent into the right stage). */
export const VERB_TO_TASK: Record<VerbStep, ProgressTask> = {
  transcribe: "Asr",
  align: "Fa",
  morphotag: "Morphosyntax",
  translate: "Translate",
  compare: "Compare",
};

export type StageState = "queued" | "running" | "done" | "fail";
export type FileStatus = "queued" | "running" | "done" | "failed";
export type BatchState = "idle" | "running" | "done" | "failed";

export interface StageRow {
  verb: VerbStep;
  state: StageState;
  pct: number; // 0–100
}

export interface LogEntry {
  ts: number;
  level: "info" | "warn" | "error";
  text: string;
}

/** Which discovery bucket a file belongs to (matches the daemon's
 *  `InputKind`). Drives kind-based filtering in the right-pane file
 *  table — only files whose `kind` is compatible with the first verb
 *  in the active pipeline get rendered. */
export type FileKind = "media" | "chat";

export interface FileRow {
  source_id: string;
  stem: string; // "P_1025_baseline"
  filename: string; // "P_1025_baseline.wav"
  sizeBytes: number;
  durationMs: number | null;
  kind: FileKind;
  status: FileStatus;
  stages: StageRow[];
  log: LogEntry[];
}

/**
 * Per-verb config blob. The shape is intentionally loose here because
 * the daemon's recipe request schemas are introspected at runtime
 * (see protocol/openapi.gen.ts once generated). Panels read
 * `state.capabilities.recipes[verb].params` to decide what fields to
 * render, write back via `VERB_CONFIG_CHANGED` actions.
 */
export type VerbConfig = Record<string, unknown>;

export interface Batch {
  id: string;
  name: string;
  folderPath: string;
  inPlace: boolean;
  outputPath: string | null;
  pipeline: VerbStep[];
  config: Record<VerbStep, VerbConfig>;
  files: Record<string, FileRow>;
  fileOrder: string[];
  state: BatchState;
  jobId: string | null;
  startedAt: number | null;
  finishedAt: number | null;
  expandedFileId: string | null;
}

export interface SettingsState {
  defaultWorkers: number;
  forceCpu: boolean;
  memoryGuard: boolean;
  adaptiveWorkers: boolean;
  verbosity: "quiet" | "v" | "vv";
  revAiKey: string | null;
}

/** /capabilities JSON shape. Loosened to `unknown` for forward-compat. */
export interface CapabilitiesJson {
  api_version: string;
  recipes: Record<string, { doc: string; params: ParamInfo[] }>;
  backends: Record<
    string,
    { doc: string; tasks: string[]; kwargs: ParamInfo[] }
  >;
  backends_by_task: Record<string, string[]>;
  input_kinds: string[];
  job_states: string[];
  endpoints: Record<string, { method: string; path: string }>;
}

export interface ParamInfo {
  name: string;
  required: boolean;
  default: string | null;
  annotation: string | null;
  is_backend?: boolean;
}

export interface AppState {
  batches: Record<string, Batch>;
  tabOrder: string[];
  activeBatchId: string | null;
  daemon: {
    port: number | null;
    ready: boolean;
    error: string | null;
    /** Last stdout/stderr line from the booting sidecar — surfaced by
     *  the boot overlay so a multi-minute first-launch install looks
     *  alive instead of frozen. Cleared once the daemon is ready. */
    progressLine: string | null;
  };
  capabilities: CapabilitiesJson | null;
  settings: SettingsState;
  showSettings: boolean;
}

export type Action =
  | { type: "DAEMON_READY"; port: number }
  | { type: "DAEMON_FAILED"; reason: string }
  | { type: "DAEMON_PROGRESS"; line: string }
  | { type: "CAPABILITIES_LOADED"; capabilities: CapabilitiesJson }
  | { type: "BATCH_OPENED"; batch: Batch }
  | { type: "BATCH_CLOSED"; batchId: string }
  | { type: "BATCH_ACTIVATED"; batchId: string }
  | { type: "BATCH_INPLACE_CHANGED"; batchId: string; inPlace: boolean }
  | { type: "BATCH_OUTPUT_CHANGED"; batchId: string; outputPath: string | null }
  | { type: "PIPELINE_CHANGED"; batchId: string; pipeline: VerbStep[] }
  | {
      type: "VERB_CONFIG_CHANGED";
      batchId: string;
      verb: VerbStep;
      patch: VerbConfig;
    }
  | {
      type: "BATCH_STARTED";
      batchId: string;
      jobId: string;
      files: FileRow[];
    }
  | { type: "PROGRESS_V2"; batchId: string; event: ProgressEvent }
  | {
      type: "FILE_EXPANDED";
      batchId: string;
      sourceId: string | null;
    }
  | { type: "SETTINGS_TOGGLED"; show: boolean }
  | { type: "SETTINGS_UPDATED"; patch: Partial<SettingsState> };

const defaultSettings: SettingsState = {
  defaultWorkers: 4,
  forceCpu: false,
  memoryGuard: false,
  adaptiveWorkers: true,
  verbosity: "quiet",
  revAiKey: null,
};

const initial: AppState = {
  batches: {},
  tabOrder: [],
  activeBatchId: null,
  daemon: { port: null, ready: false, error: null, progressLine: null },
  capabilities: null,
  settings: defaultSettings,
  showSettings: false,
};

// --- helpers --------------------------------------------------------

function makeStages(pipeline: VerbStep[]): StageRow[] {
  return pipeline.map((verb) => ({ verb, state: "queued", pct: 0 }));
}

function findStageIndex(file: FileRow, task: ProgressTask | null): number {
  if (!task) return -1;
  return file.stages.findIndex((s) => VERB_TO_TASK[s.verb] === task);
}

function recomputeFileStatus(file: FileRow): FileStatus {
  if (file.stages.every((s) => s.state === "done")) return "done";
  if (file.stages.some((s) => s.state === "fail")) return "failed";
  if (file.stages.some((s) => s.state === "running")) return "running";
  return "queued";
}

function recomputeBatchState(batch: Batch): BatchState {
  const files = Object.values(batch.files);
  if (files.length === 0) return "idle";
  if (files.every((f) => f.status === "done")) return "done";
  if (files.some((f) => f.status === "failed")) {
    if (files.every((f) => f.status === "done" || f.status === "failed")) {
      return "failed";
    }
  }
  if (files.some((f) => f.status === "running" || f.status === "queued")) {
    return "running";
  }
  return batch.state;
}

// --- reducer --------------------------------------------------------

export function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "DAEMON_READY":
      return {
        ...state,
        daemon: {
          port: action.port,
          ready: true,
          error: null,
          progressLine: null,
        },
      };
    case "DAEMON_FAILED":
      return {
        ...state,
        daemon: {
          port: null,
          ready: false,
          error: action.reason,
          progressLine: null,
        },
      };
    case "DAEMON_PROGRESS":
      return {
        ...state,
        daemon: { ...state.daemon, progressLine: action.line },
      };
    case "CAPABILITIES_LOADED":
      return { ...state, capabilities: action.capabilities };

    case "BATCH_OPENED": {
      const { batch } = action;
      return {
        ...state,
        batches: { ...state.batches, [batch.id]: batch },
        tabOrder: state.tabOrder.includes(batch.id)
          ? state.tabOrder
          : [...state.tabOrder, batch.id],
        activeBatchId: batch.id,
        showSettings: false,
      };
    }

    case "BATCH_CLOSED": {
      const next = { ...state.batches };
      delete next[action.batchId];
      const tabOrder = state.tabOrder.filter((id) => id !== action.batchId);
      const activeBatchId =
        state.activeBatchId === action.batchId
          ? (tabOrder[tabOrder.length - 1] ?? null)
          : state.activeBatchId;
      return { ...state, batches: next, tabOrder, activeBatchId };
    }

    case "BATCH_ACTIVATED":
      return { ...state, activeBatchId: action.batchId, showSettings: false };

    case "BATCH_INPLACE_CHANGED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: { ...batch, inPlace: action.inPlace },
        },
      };
    }

    case "BATCH_OUTPUT_CHANGED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: { ...batch, outputPath: action.outputPath },
        },
      };
    }

    case "PIPELINE_CHANGED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      // Reset stage rows on every file to mirror the new pipeline.
      const files: Record<string, FileRow> = {};
      for (const id of batch.fileOrder) {
        files[id] = { ...batch.files[id], stages: makeStages(action.pipeline) };
      }
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: {
            ...batch,
            pipeline: action.pipeline,
            files,
          },
        },
      };
    }

    case "VERB_CONFIG_CHANGED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: {
            ...batch,
            config: {
              ...batch.config,
              [action.verb]: { ...batch.config[action.verb], ...action.patch },
            },
          },
        },
      };
    }

    case "BATCH_STARTED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      const files: Record<string, FileRow> = {};
      const fileOrder: string[] = [];
      for (const f of action.files) {
        files[f.source_id] = f;
        fileOrder.push(f.source_id);
      }
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: {
            ...batch,
            jobId: action.jobId,
            files,
            fileOrder,
            state: "running",
            startedAt: Date.now(),
            finishedAt: null,
          },
        },
      };
    }

    case "PROGRESS_V2": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      const file = batch.files[action.event.source_id];
      if (!file) return state;
      const stageIdx = findStageIndex(file, action.event.task);
      const stages = file.stages.map((s) => ({ ...s }));
      const log: LogEntry[] = [...file.log];

      switch (action.event.kind) {
        case "StageStarted":
          if (stageIdx >= 0) {
            for (let i = 0; i < stageIdx; i++) stages[i].state = "done";
            stages[stageIdx].state = "running";
            stages[stageIdx].pct = 0;
          }
          if (action.event.label) {
            log.push({
              ts: Date.now(),
              level: "info",
              text: action.event.label,
            });
          }
          break;
        case "StageInjected":
          if (stageIdx >= 0) {
            stages[stageIdx].state = "running";
            stages[stageIdx].pct =
              action.event.total > 0
                ? Math.min(
                    100,
                    Math.round(
                      (action.event.completed / action.event.total) * 100,
                    ),
                  )
                : stages[stageIdx].pct;
          }
          if (action.event.label) {
            log.push({
              ts: Date.now(),
              level: "info",
              text: action.event.label,
            });
          }
          break;
        case "StageSkipped":
          if (stageIdx >= 0) {
            stages[stageIdx].state = "done";
            stages[stageIdx].pct = 100;
          }
          break;
        case "StageFailed":
          if (stageIdx >= 0) {
            stages[stageIdx].state = "fail";
          }
          log.push({
            ts: Date.now(),
            level: "error",
            text: action.event.label ?? "stage failed",
          });
          break;
        case "SourceCompleted":
          for (const s of stages) {
            if (s.state !== "fail") s.state = "done";
            if (s.state === "done") s.pct = 100;
          }
          break;
      }

      const newFile: FileRow = {
        ...file,
        stages,
        log: log.slice(-200), // cap at 200 to bound memory
        status: recomputeFileStatus({ ...file, stages }),
      };
      const newFiles = { ...batch.files, [file.source_id]: newFile };
      const newBatch: Batch = {
        ...batch,
        files: newFiles,
        state: recomputeBatchState({ ...batch, files: newFiles }),
        finishedAt:
          recomputeBatchState({ ...batch, files: newFiles }) === "done" ||
          recomputeBatchState({ ...batch, files: newFiles }) === "failed"
            ? (batch.finishedAt ?? Date.now())
            : batch.finishedAt,
      };
      return {
        ...state,
        batches: { ...state.batches, [batch.id]: newBatch },
      };
    }

    case "FILE_EXPANDED": {
      const batch = state.batches[action.batchId];
      if (!batch) return state;
      return {
        ...state,
        batches: {
          ...state.batches,
          [action.batchId]: { ...batch, expandedFileId: action.sourceId },
        },
      };
    }

    case "SETTINGS_TOGGLED":
      return { ...state, showSettings: action.show };

    case "SETTINGS_UPDATED":
      return { ...state, settings: { ...state.settings, ...action.patch } };
  }
}

// --- store ----------------------------------------------------------

interface AppStore extends AppState {
  dispatch: (action: Action) => void;
}

export const useStore = create<AppStore>((set) => ({
  ...initial,
  dispatch: (action) => set((state) => reducer(state, action)),
}));

/** Imperative dispatch (for bridge.ts where we're outside React). */
export const dispatch = (action: Action) =>
  useStore.getState().dispatch(action);

/** Snapshot getter, e.g. for HTTP request bodies. */
export const getAppState = (): AppState => useStore.getState();
