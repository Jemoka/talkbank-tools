/** Compact recent jobs list for the process home screen.
 *
 * Shows the last few jobs from the Zustand store with their status,
 * command, file count, and time. Clicking a job navigates to its detail page.
 */

import { useSortedJobs } from "../../state";
import { commandStyle, statusDotColor, relativeTime } from "../../utils";

const MAX_RECENT = 5;

export function RecentJobs() {
  const jobs = useSortedJobs();
  const recent = jobs.slice(0, MAX_RECENT);

  if (recent.length === 0) return null;

  return (
    <div>
      <h2 className="text-lg font-semibold text-gray-800 mb-3">
        Recent Tasks
      </h2>
      <div className="space-y-2">
        {recent.map((job) => {
          const [bg, text] = commandStyle(job.command);
          return (
            <a
              key={job.job_id}
              href={`/dashboard/jobs/${job.job_id}`}
              className="flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg
                hover:border-gray-300 hover:shadow-sm transition-all no-underline"
            >
              <span
                className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${statusDotColor(job.status)} ${
                  job.status === "running" || job.status === "queued"
                    ? "status-dot-pulse"
                    : ""
                }`}
              />
              <span
                className={`text-xs font-medium px-1.5 py-0.5 rounded ${bg} ${text}`}
              >
                {job.command}
              </span>
              <span className="flex-1 text-sm text-gray-600 truncate">
                {job.completed_files}/{job.total_files} files
              </span>
              <span className="text-xs text-gray-400 flex-shrink-0">
                {relativeTime(job.submitted_at)}
              </span>
            </a>
          );
        })}
      </div>
    </div>
  );
}
