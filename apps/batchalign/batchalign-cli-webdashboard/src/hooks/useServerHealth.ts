/** Poll the batchalign server's `/health` endpoint on a regular interval.
 *
 * Exposes the server's reachability, capability list, and version so the
 * process form can disable submission when the server is down and show
 * which commands are available.
 */

import { useQuery } from "@tanstack/react-query";
import { fetchHealth } from "../api";
import { controlPlaneOrigin } from "../runtime";
import type { HealthResponse } from "../types";

export interface ServerHealthState {
  /** Latest health response, or undefined if unreachable. */
  health: HealthResponse | undefined;
  /** True while the first fetch is in flight. */
  isLoading: boolean;
  /** True if the server responded to the last health check. */
  isReachable: boolean;
  /** Human-readable connection status. */
  statusLabel: "connected" | "connecting" | "unreachable";
}

/** Poll server health every 5 seconds. */
export function useServerHealth(): ServerHealthState {
  const base = controlPlaneOrigin();

  const { data, isLoading, isError } = useQuery<HealthResponse>({
    queryKey: ["server-health", base],
    queryFn: () => fetchHealth(base),
    refetchInterval: 5_000,
    retry: 1,
    staleTime: 4_000,
  });

  const isReachable = !isError && data != null;

  let statusLabel: ServerHealthState["statusLabel"];
  if (isLoading) statusLabel = "connecting";
  else if (isReachable) statusLabel = "connected";
  else statusLabel = "unreachable";

  return { health: data, isLoading, isReachable, statusLabel };
}
