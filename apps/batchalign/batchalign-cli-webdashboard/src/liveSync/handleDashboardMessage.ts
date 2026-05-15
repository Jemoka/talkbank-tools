/** WebSocket-to-client-state reconciliation logic.
 *
 * The dashboard keeps two client-side views in sync:
 *
 * - Zustand for summary-oriented fleet state
 * - React Query for REST-shaped detail/query cache entries
 *
 * Each WebSocket message updates both views together so route-level components
 * can render a consistent picture regardless of whether they read from the
 * store, the query cache, or both.
 */
import { jobQueryKey, jobsQueryKey, queryClient } from "../query";
import { useStore } from "../state";
import type { JobInfo, JobListItem, WSMessage } from "../types";

/** Apply one fleet WebSocket message to the store and React Query cache. */
export function handleDashboardMessage(server: string, msg: WSMessage) {
  const { setJobsForServer, updateJob, deleteJob, setHealth } = useStore.getState();

  switch (msg.type) {
    case "snapshot": {
      // Replace the entire server slice with the authoritative snapshot from
      // the socket. This is the baseline that later incremental updates patch.
      const taggedJobs = msg.jobs.map((job) => ({ ...job, server }));
      setJobsForServer(server, taggedJobs);
      setHealth(server, msg.health);
      queryClient.setQueryData<JobListItem[]>(jobsQueryKey(server), taggedJobs);
      break;
    }
    case "job_update": {
      // Job-level updates patch summary rows and the per-server detail cache
      // entry if that route has already been fetched.
      const taggedJob = { ...msg.job, server };
      updateJob(taggedJob);
      queryClient.setQueryData<JobListItem[]>(
        jobsQueryKey(server),
        (previous) => {
          const next = [...(previous ?? [])];
          const index = next.findIndex((job) => job.job_id === taggedJob.job_id);
          if (index >= 0) {
            next[index] = taggedJob;
          } else {
            next.unshift(taggedJob);
          }
          return next;
        }
      );

      const patch = {
        status: msg.job.status,
        completed_files: msg.job.completed_files,
        duration_s: msg.job.duration_s,
        completed_at: msg.job.completed_at,
        num_workers: msg.job.num_workers,
      };
      queryClient.setQueryData<JobInfo>(
        jobQueryKey(server, msg.job.job_id),
        (previous) => (previous ? { ...previous, ...patch } : previous)
      );
      break;
    }
    case "file_update": {
      // File-level updates affect both list progress counters and any cached
      // detailed file-status table for the same `(server, jobId)` pair.
      const jobs = useStore.getState().jobs;
      const key = `${server}|${msg.job_id}`;
      const existing = jobs.get(key);
      if (existing) {
        updateJob({ ...existing, completed_files: msg.completed_files });
      }
      queryClient.setQueryData<JobListItem[]>(
        jobsQueryKey(server),
        (previous) =>
          (previous ?? []).map((job) =>
            job.job_id === msg.job_id
              ? { ...job, completed_files: msg.completed_files }
              : job
          )
      );

      queryClient.setQueryData<JobInfo>(
        jobQueryKey(server, msg.job_id),
        (previous) =>
          previous
            ? {
                ...previous,
                completed_files: msg.completed_files,
                file_statuses: previous.file_statuses.map((fileStatus) =>
                  fileStatus.filename === msg.file.filename
                    ? { ...fileStatus, ...msg.file }
                    : fileStatus
                ),
              }
            : previous
      );
      break;
    }
    case "job_deleted": {
      // Deletions must evict both the list entry and any stale cached detail
      // query so a future lookup does not resurrect removed data.
      deleteJob(server, msg.job_id);
      queryClient.setQueryData<JobListItem[]>(
        jobsQueryKey(server),
        (previous) => (previous ?? []).filter((job) => job.job_id !== msg.job_id)
      );
      queryClient.removeQueries({
        queryKey: jobQueryKey(server, msg.job_id),
        exact: true,
      });
      break;
    }
  }
}
