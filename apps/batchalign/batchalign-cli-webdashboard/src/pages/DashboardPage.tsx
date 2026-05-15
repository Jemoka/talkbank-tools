/** Dashboard route shell.
 *
 * Two-column layout: job list (left) + system panels stacked (right).
 * Collapses to single column on mobile. The right column shows worker
 * profiles, system memory, and operational counters — providing at-a-glance
 * server health alongside the job stream.
 */

import { StatsRow } from "../components/StatsRow";
import { JobList } from "../components/JobList";
import { ServerFilterTabs } from "../components/ServerFilterTabs";
import { WorkerProfilePanel } from "../components/WorkerProfilePanel";
import { MemoryPanel } from "../components/MemoryPanel";
import { VitalsRow } from "../components/VitalsRow";
import { CacheStatusPanel } from "../components/CacheStatusPanel";
import { useStore } from "../state";
import type { HealthResponse } from "../types";

/** Pick the first server's health from the health map.
 *
 * Single-server-first design: most deployments have one server. When
 * a server filter is active, prefer that server's health.
 */
function useHealth(): HealthResponse | undefined {
  return useStore((s) => {
    if (s.serverFilter) {
      return s.healthMap.get(s.serverFilter);
    }
    const first = s.healthMap.values().next();
    return first.done ? undefined : first.value;
  });
}

export function DashboardPage() {
  const serverFilter = useStore((s) => s.serverFilter);
  const setServerFilter = useStore((s) => s.setServerFilter);
  const health = useHealth();

  return (
    <div>
      <ServerFilterTabs selected={serverFilter} onSelect={setServerFilter} />
      <StatsRow />

      <div className="mt-4 grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-6 items-start">
        {/* Left column: job list */}
        <div className="min-w-0">
          <JobList />
        </div>

        {/* Right column: system panels */}
        <div className="space-y-4">
          <WorkerProfilePanel health={health} />
          <MemoryPanel health={health} />
          <VitalsRow health={health} />
          <CacheStatusPanel health={health} />
        </div>
      </div>
    </div>
  );
}
