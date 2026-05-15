/** Job list with sorting controls.
 *
 * Supports sorting by newest (default), status, progress, and duration.
 */

import { useState } from "react";
import { useFilteredJobs } from "../state";
import { JobCard } from "./JobCard";
import { EmptyState } from "./EmptyState";
import type { JobListItem } from "../types";

type SortKey = "newest" | "status" | "progress" | "duration";

const SORT_LABELS: Record<SortKey, string> = {
  newest: "Newest",
  status: "Status",
  progress: "Progress",
  duration: "Duration",
};

const STATUS_ORDER: Record<string, number> = {
  running: 0,
  queued: 1,
  failed: 2,
  cancelled: 3,
  completed: 4,
};

function compareJobs(a: JobListItem, b: JobListItem, key: SortKey): number {
  switch (key) {
    case "newest":
      return (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "");
    case "status": {
      const sa = STATUS_ORDER[a.status] ?? 9;
      const sb = STATUS_ORDER[b.status] ?? 9;
      if (sa !== sb) return sa - sb;
      return (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "");
    }
    case "progress": {
      const pa = a.total_files > 0 ? a.completed_files / a.total_files : 0;
      const pb = b.total_files > 0 ? b.completed_files / b.total_files : 0;
      if (pa !== pb) return pa - pb; // least progress first
      return (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "");
    }
    case "duration": {
      const da = a.duration_s ?? 0;
      const db = b.duration_s ?? 0;
      if (da !== db) return db - da; // longest first
      return (b.submitted_at ?? "").localeCompare(a.submitted_at ?? "");
    }
  }
}

export function JobList() {
  const items = useFilteredJobs();
  const [sortKey, setSortKey] = useState<SortKey>("newest");

  if (items.length === 0) return <EmptyState />;

  const sorted = [...items].sort((a, b) => compareJobs(a, b, sortKey));

  return (
    <div className="space-y-3">
      {/* Sort control */}
      <div className="flex items-center gap-2">
        <label htmlFor="job-sort" className="text-xs text-zinc-400">
          Sort:
        </label>
        <select
          id="job-sort"
          value={sortKey}
          onChange={(e) => setSortKey(e.target.value as SortKey)}
          className="text-xs border border-zinc-200 rounded-md px-2 py-1 bg-white text-zinc-600 focus:outline-none focus:ring-1 focus:ring-zinc-300"
        >
          {(Object.keys(SORT_LABELS) as SortKey[]).map((k) => (
            <option key={k} value={k}>
              {SORT_LABELS[k]}
            </option>
          ))}
        </select>
      </div>

      {/* Job cards */}
      <div className="flex flex-col gap-3">
        {sorted.map((job) => (
          <JobCard key={job.job_id} job={job} />
        ))}
      </div>
    </div>
  );
}
