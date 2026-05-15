/** Global dashboard state using Zustand.
 *
 * The store owns summary-oriented shared UI state:
 *
 * - per-server job rows
 * - per-server health
 * - WebSocket connection indicators
 * - dashboard-level filters
 *
 * Detailed job payloads intentionally do not live here anymore. Those records
 * now belong to React Query so the live WebSocket reconciliation layer and the
 * job-detail page read and write the same cache entries.
 */

import { create } from "zustand";
import { useShallow } from "zustand/shallow";
import type { HealthResponse, JobListItem } from "./types";

interface AppState {
  /** Jobs keyed by `serverHost|job_id` (avoids collisions across servers). */
  jobs: Map<string, JobListItem>;
  /** Per-server health. Key = short hostname. */
  healthMap: Map<string, HealthResponse>;
  /** Per-server WebSocket connection status. Key = short hostname. */
  wsConnectedMap: Map<string, boolean>;
  /** Filter jobs by server (null = show all). */
  serverFilter: string | null;

  setJobsForServer: (server: string, jobs: JobListItem[]) => void;
  updateJob: (job: JobListItem) => void;
  deleteJob: (server: string, jobId: string) => void;
  setHealth: (server: string, h: HealthResponse) => void;
  setWsConnected: (server: string, v: boolean) => void;
  setServerFilter: (server: string | null) => void;
}

function jobKey(server: string, jobId: string): string {
  return `${server}|${jobId}`;
}

export function findJobById(
  jobs: ReadonlyMap<string, JobListItem>,
  jobId: string
): JobListItem | undefined {
  // The store is keyed by `server|job_id` for collision safety, but detail
  // routes still begin with a bare job id. This helper is the one place that
  // knows how to bridge those two lookup shapes.
  for (const [, job] of jobs) {
    if (job.job_id === jobId) return job;
  }
  return undefined;
}

export const useStore = create<AppState>((set, get) => ({
  jobs: new Map(),
  healthMap: new Map(),
  wsConnectedMap: new Map(),
  serverFilter: null,

  setJobsForServer: (server, jobs) => {
    const next = new Map(get().jobs);
    // Remove old entries for this server
    for (const [k] of next) {
      if (k.startsWith(`${server}|`)) next.delete(k);
    }
    // Add new entries
    for (const j of jobs) {
      next.set(jobKey(server, j.job_id), { ...j, server });
    }
    set({ jobs: next });
  },

  updateJob: (job) => {
    const server = job.server ?? "";
    const next = new Map(get().jobs);
    next.set(jobKey(server, job.job_id), job);
    set({ jobs: next });
  },

  deleteJob: (server, jobId) => {
    const next = new Map(get().jobs);
    next.delete(jobKey(server, jobId));
    set({ jobs: next });
  },

  setHealth: (server, health) => {
    const next = new Map(get().healthMap);
    next.set(server, health);
    set({ healthMap: next });
  },

  setWsConnected: (server, connected) => {
    const next = new Map(get().wsConnectedMap);
    next.set(server, connected);
    set({ wsConnectedMap: next });
  },

  setServerFilter: (serverFilter) => set({ serverFilter }),
}));

/** Derived: jobs sorted newest-first. */
export function useSortedJobs(): JobListItem[] {
  return useStore(
    useShallow((s) =>
      [...s.jobs.values()].sort((a, b) =>
        (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "")
      )
    )
  );
}

/** Derived: jobs sorted newest-first, filtered by serverFilter. */
export function useFilteredJobs(): JobListItem[] {
  return useStore(
    useShallow((s) => {
      const filter = s.serverFilter;
      const all = [...s.jobs.values()];
      const filtered = filter === null ? all : all.filter((j) => j.server === filter);
      return filtered.sort((a, b) =>
        (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "")
      );
    })
  );
}

/** Derived: one job by id, regardless of which server currently owns it. */
export function useJobById(jobId: string): JobListItem | undefined {
  return useStore((state) => findJobById(state.jobs, jobId));
}

/** Derived: aggregate stats across all jobs. */
export function useStats() {
  return useStore(
    useShallow((s) => {
      const all = [...s.jobs.values()];
      const active = all.filter(
        (j) => j.status === "queued" || j.status === "running"
      ).length;
      const completed = all.filter((j) => j.status === "completed").length;
      const failed = all.filter(
        (j) => j.status === "failed" || j.status === "cancelled"
      ).length;
      const totalFiles = all.reduce((sum, j) => sum + j.total_files, 0);
      return { active, completed, failed, totalFiles };
    })
  );
}

/** Derived: stats filtered by serverFilter (or aggregate if null). */
export function useFilteredStats() {
  return useStore(
    useShallow((s) => {
      const filter = s.serverFilter;
      const all = [...s.jobs.values()];
      const filtered = filter === null ? all : all.filter((j) => j.server === filter);
      const active = filtered.filter(
        (j) => j.status === "queued" || j.status === "running"
      ).length;
      const completed = filtered.filter((j) => j.status === "completed").length;
      const failed = filtered.filter(
        (j) => j.status === "failed" || j.status === "cancelled"
      ).length;
      const totalFiles = filtered.reduce((sum, j) => sum + j.total_files, 0);
      return { active, completed, failed, totalFiles };
    })
  );
}

/** Derived: true if ANY server is connected. */
export function useAnyConnected(): boolean {
  return useStore((s) => [...s.wsConnectedMap.values()].some(Boolean));
}

/** Derived: per-server connection entries for the header. */
export function useServerStatuses(): Array<{ server: string; connected: boolean }> {
  const map = useStore((s) => s.wsConnectedMap);
  return [...map.entries()].map(([server, connected]) => ({ server, connected }));
}
