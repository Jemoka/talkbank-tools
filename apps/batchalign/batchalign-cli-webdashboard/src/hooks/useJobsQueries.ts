import { useQueries } from "@tanstack/react-query";
import { fetchJobs, serverBaseUrl } from "../api";
import { jobsQueryKey, QUERY_POLICY, retryPolicy } from "../query";
import { serverLabel } from "../ws";
import type { JobListItem } from "../types";

export function useJobsQueries(servers: string[]) {
  return useQueries({
    queries: servers.map((hostPort) => {
      const server = serverLabel(hostPort);
      return {
        queryKey: jobsQueryKey(server),
        queryFn: async (): Promise<JobListItem[]> => {
          const jobs = await fetchJobs(serverBaseUrl(hostPort));
          return jobs.map((job) => ({ ...job, server }));
        },
        staleTime: QUERY_POLICY.jobs.staleMs,
        retry: retryPolicy(QUERY_POLICY.jobs.retries),
      };
    }),
  });
}
