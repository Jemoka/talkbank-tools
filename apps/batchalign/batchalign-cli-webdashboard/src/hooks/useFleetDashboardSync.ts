/** Fleet bootstrap and live-sync hook for the dashboard shell.
 *
 * This hook is the frontend control-plane seam for multi-server dashboard
 * state. It owns three jobs:
 *
 * 1. discover which servers the dashboard should talk to
 * 2. issue the per-server list queries used for initial snapshots
 * 3. keep Zustand and React Query aligned with WebSocket updates
 *
 * The rest of the app consumes those synchronized views rather than reasoning
 * about fleet discovery or socket lifecycle directly.
 */
import { useEffect, useState } from "react";
import { useJobsQueries } from "./useJobsQueries";
import { useStore } from "../state";
import { createMultiWS, discoverFleet, serverLabel } from "../ws";
import { handleDashboardMessage } from "../liveSync/handleDashboardMessage";

/** Keep fleet discovery, job queries, and WebSocket live-sync aligned. */
export function useFleetDashboardSync() {
  const [servers, setServers] = useState<string[]>([]);
  const setJobsForServer = useStore((state) => state.setJobsForServer);
  const setWsConnected = useStore((state) => state.setWsConnected);
  const jobListQueries = useJobsQueries(servers);

  useEffect(() => {
    // Query results are authoritative snapshots. As each per-server query
    // resolves, mirror it into the store under that server's label so the
    // UI has a single, server-qualified source of truth.
    for (let index = 0; index < jobListQueries.length; index += 1) {
      const query = jobListQueries[index];
      const hostPort = servers[index];
      if (!query?.data || !hostPort) {
        continue;
      }
      setJobsForServer(serverLabel(hostPort), query.data);
    }
  }, [jobListQueries, servers, setJobsForServer]);

  useEffect(() => {
    // Fleet discovery is async and may be cancelled if the root component
    // unmounts before the server list resolves. The cleanup closure owns the
    // socket tree so the rest of the app never handles raw WebSocket lifecycles.
    let cancelled = false;
    let cleanup: (() => void) | null = null;

    discoverFleet().then((resolvedServers) => {
      if (cancelled) {
        return;
      }
      setServers(resolvedServers);
      const ws = createMultiWS(
        resolvedServers,
        handleDashboardMessage,
        setWsConnected
      );
      cleanup = () => ws.close();
    });

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [setWsConnected]);
}
