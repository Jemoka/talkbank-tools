/** React Query hook for one fully resolved job-detail payload.
 *
 * The detail route first discovers which server owns the requested job id, and
 * then this hook owns the stable per-server cache entry for the detailed job
 * payload. WebSocket reconciliation patches the same `jobQueryKey`, so the
 * page no longer needs a separate Zustand-only detail slot.
 */
import { useQuery } from "@tanstack/react-query";
import { fetchJob, serverBaseUrl } from "../api";
import { QUERY_POLICY, jobQueryKey, retryPolicy } from "../query";
import type { JobInfo } from "../types";

/** Fetch and cache one detailed job record for the resolved owning server. */
export function useJobDetailQuery(
  jobId: string,
  server: string,
  hostPort: string,
  initialJob: JobInfo | null
) {
  return useQuery({
    queryKey: jobQueryKey(server, jobId),
    enabled: Boolean(jobId && server),
    staleTime: QUERY_POLICY.jobDetail.staleMs,
    retry: retryPolicy(QUERY_POLICY.jobDetail.retries),
    initialData:
      initialJob && initialJob.server === server ? initialJob : undefined,
    queryFn: async (): Promise<JobInfo> => ({
      ...(await fetchJob(jobId, serverBaseUrl(hostPort))),
      server,
    }),
  });
}
