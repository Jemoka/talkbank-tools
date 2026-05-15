/** React Query hook that resolves which server currently owns a job id.
 *
 * The dashboard route path contains only a bare `jobId`, but the fleet can
 * have multiple servers. This hook probes the preferred server first, then the
 * remaining configured servers, and seeds the resolved per-server detail cache
 * once it finds a match.
 */
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { fetchJob, serverBaseUrl } from "../api";
import {
  QUERY_POLICY,
  jobLookupQueryKey,
  jobQueryKey,
  queryClient,
  retryPolicy,
} from "../query";
import { controlPlaneHostPort } from "../runtime";
import { serverLabel } from "../ws";
import type { JobInfo } from "../types";

/** Result of resolving a bare job id to one specific server-owned detail record. */
type JobLookupResult = {
  job: JobInfo;
  server: string;
};

/** Probe configured servers until one returns a job-detail payload. */
export function useJobLookupQuery(
  jobId: string,
  preferredServer: string,
  servers: string[]
) {
  const serversKey = useMemo(() => servers.join(","), [servers]);
  const labelToHost = useMemo(
    () => new Map(servers.map((hostPort) => [serverLabel(hostPort), hostPort])),
    [servers]
  );

  return useQuery({
    queryKey: jobLookupQueryKey(jobId, preferredServer, serversKey),
    enabled: Boolean(jobId),
    staleTime: QUERY_POLICY.jobDetail.staleMs,
    retry: retryPolicy(QUERY_POLICY.jobDetail.retries),
    queryFn: async (): Promise<JobLookupResult> => {
      const seen = new Set<string>();

      if (preferredServer) {
        const hostPort = labelToHost.get(preferredServer) ?? controlPlaneHostPort();
        seen.add(hostPort);
        try {
          const job = { ...(await fetchJob(jobId, serverBaseUrl(hostPort))), server: preferredServer };
          queryClient.setQueryData(jobQueryKey(preferredServer, jobId), job);
          return { job, server: preferredServer };
        } catch {
          // Fall through to scan all servers.
        }
      }

      for (const hostPort of servers) {
        if (seen.has(hostPort)) continue;
        const label = serverLabel(hostPort);
        try {
          const job = { ...(await fetchJob(jobId, serverBaseUrl(hostPort))), server: label };
          queryClient.setQueryData(jobQueryKey(label, jobId), job);
          return { job, server: label };
        } catch {
          continue;
        }
      }

      throw new Error(`Job ${jobId} not found on configured servers`);
    },
  });
}
