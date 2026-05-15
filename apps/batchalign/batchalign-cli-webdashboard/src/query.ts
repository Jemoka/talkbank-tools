import { QueryClient } from "@tanstack/react-query";
import { ApiError } from "./api";

export const QUERY_POLICY = {
  jobs: {
    staleMs: 15_000,
    retries: 2,
  },
  jobDetail: {
    staleMs: 5_000,
    retries: 1,
  },
} as const;

export const QUERY_STALE = {
  jobsMs: QUERY_POLICY.jobs.staleMs,
  jobDetailMs: QUERY_POLICY.jobDetail.staleMs,
} as const;

function shouldRetry(error: unknown): boolean {
  if (!(error instanceof ApiError)) return true;
  // Treat most client errors as terminal (except 429 throttling).
  return error.status >= 500 || error.status === 429;
}

export function retryPolicy(maxRetries: number) {
  return (failureCount: number, error: unknown): boolean =>
    shouldRetry(error) && failureCount < maxRetries;
}

export function jobsQueryKey(server: string): readonly ["jobs", string] {
  return ["jobs", server] as const;
}

export function jobQueryKey(
  server: string,
  jobId: string
): readonly ["job", string, string] {
  return ["job", server, jobId] as const;
}

export function jobLookupQueryKey(
  jobId: string,
  preferredServer: string,
  serversKey: string
): readonly ["job-lookup", string, string, string] {
  return ["job-lookup", jobId, preferredServer, serversKey] as const;
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
    },
  },
});
