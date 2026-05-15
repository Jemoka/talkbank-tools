/** Controller hook for the job-detail route.
 *
 * `JobPage` should not know how to discover the owning server, fetch detail
 * data, reconcile live WebSocket state, or derive the correct REST base URL.
 * This hook owns that control-plane work and returns a view-ready snapshot for
 * the presentational detail component.
 *
 * Summary rows remain in Zustand because they drive fleet-wide filtering and
 * dashboard metrics. Detailed job payloads now live entirely in React Query,
 * keyed by `(server, jobId)`, so the live WebSocket patch path and the detail
 * page share one source of truth.
 */
import { useMemo } from "react";
import { serverBaseUrl } from "../api";
import { controlPlaneHostPort } from "../runtime";
import { useJobById, useStore } from "../state";
import type { JobInfo, JobListItem } from "../types";
import { getServerList, serverLabel } from "../ws";
import { useJobDetailQuery } from "./useJobDetailQuery";
import { useJobLookupQuery } from "./useJobLookupQuery";

/** Route-level state needed by the job-detail page view. */
type JobPageControllerState = {
  loading: boolean;
  detail: JobInfo | null;
  wsJob: JobListItem | undefined;
  multiServer: boolean;
  effectiveServer: string;
  serverBase: string;
};

/** Resolve job ownership, fetch detail state, and expose the correct action base URL. */
export function useJobPageController(jobId: string): JobPageControllerState {
  const wsJob = useJobById(jobId);
  const multiServer = useStore((state) => state.wsConnectedMap.size > 1);
  const servers = useMemo(() => getServerList(), []);
  const labelToHost = useMemo(
    () => new Map(servers.map((hostPort) => [serverLabel(hostPort), hostPort])),
    [servers]
  );

  const lookupQuery = useJobLookupQuery(jobId, wsJob?.server ?? "", servers);

  // Prefer the server label resolved from the detail lookup query. The live job
  // summary remains a fallback so the page can render immediately from socket
  // state while the lookup query resolves.
  const effectiveServer = lookupQuery.data?.server ?? wsJob?.server ?? "";
  const hostPort = effectiveServer
    ? labelToHost.get(effectiveServer) ?? controlPlaneHostPort()
    : controlPlaneHostPort();
  const detailQuery = useJobDetailQuery(
    jobId,
    effectiveServer,
    hostPort,
    lookupQuery.data?.job ?? null
  );
  const detail = detailQuery.data ?? lookupQuery.data?.job ?? null;
  const loading =
    lookupQuery.isPending ||
    (Boolean(effectiveServer) && detailQuery.isPending && !detail);

  return {
    loading,
    detail,
    wsJob,
    multiServer,
    effectiveServer,
    serverBase: serverBaseUrl(hostPort),
  };
}
