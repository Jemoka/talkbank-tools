import type { JobListItem } from "../types";
import { useStore } from "../state";
import { ProgressBar } from "./ProgressBar";
import {
  commandStyle,
  statusDotColor,
  relativeTime,
  formatDuration,
  progressPercent,
  submitterName,
  shortPath,
  displayLang,
  isDefaultLang,
} from "../utils";

export function JobCard({ job }: { job: JobListItem }) {
  const isActive = job.status === "queued" || job.status === "running";
  const isRunning = job.status === "running";
  const errorFiles = job.error_files ?? 0;
  const hasErrors = errorFiles > 0;
  const multiServer = useStore((s) => s.wsConnectedMap.size > 1);
  const pct = progressPercent(job.completed_files, job.total_files);
  const [cmdBg, cmdText] = commandStyle(job.command);
  const host = submitterName(job.submitted_by_name, job.submitted_by);

  return (
    <a
      href={`/dashboard/jobs/${job.job_id}`}
      className="group block bg-white rounded-lg border border-zinc-200 hover:border-zinc-300 hover:shadow-sm transition-all no-underline"
    >
      {/* Top row: command badge + status + view button */}
      <div className="flex items-center gap-3 px-4 pt-3.5 pb-2">
        <span
          className={`inline-block px-2 py-0.5 rounded text-[11px] font-mono font-semibold uppercase tracking-wider ${cmdBg} ${cmdText}`}
        >
          {job.command}
        </span>

        <span className="inline-flex items-center gap-1.5">
          <span
            className={`inline-block w-1.5 h-1.5 rounded-full ${statusDotColor(job.status)} ${
              isRunning ? "status-dot-pulse" : ""
            }`}
          />
          <span className="text-xs text-zinc-400 capitalize">{job.status}</span>
        </span>

        {hasErrors && (
          <span className="text-[11px] text-red-500 font-medium">
            {errorFiles} failed
          </span>
        )}

        <span className="ml-auto text-xs text-zinc-400 opacity-0 group-hover:opacity-100 transition-opacity">
          View &rarr;
        </span>
      </div>

      {/* Source directory (if present) */}
      {job.source_dir && (
        <div className="px-4 pb-1 text-[11px] text-zinc-500 font-mono truncate" title={job.source_dir}>
          {shortPath(job.source_dir)}
        </div>
      )}

      {/* Metadata line */}
      <div className="flex items-center gap-2 px-4 pb-2 text-[11px] text-zinc-400 font-mono">
        <span className="text-zinc-500">{job.job_id.slice(0, 8)}</span>
        {host && (
          <>
            <span className="text-zinc-300">&middot;</span>
            <span>{host}</span>
          </>
        )}
        {job.lang && !isDefaultLang(job.lang) && (
          <>
            <span className="text-zinc-300">&middot;</span>
            <span>{displayLang(job.lang)}</span>
          </>
        )}
        {multiServer && job.server && (
          <>
            <span className="text-zinc-300">&middot;</span>
            <span>{job.server}</span>
          </>
        )}
        <span className="text-zinc-300">&middot;</span>
        <span>
          {job.completed_files}/{job.total_files} files
          {isActive && ` (${pct}%)`}
        </span>
        {job.num_workers != null && (
          <>
            <span className="text-zinc-300">&middot;</span>
            <span>{job.num_workers}w</span>
          </>
        )}
        <span className="ml-auto text-zinc-300">
          {job.duration_s != null
            ? formatDuration(job.duration_s)
            : relativeTime(job.submitted_at)}
        </span>
      </div>

      {/* Progress bar for active jobs */}
      {isActive && (
        <div className="px-4 pb-3">
          <ProgressBar
            completed={job.completed_files}
            total={job.total_files}
            animated={isRunning}
          />
        </div>
      )}

      {/* Completed jobs get a thin bottom accent */}
      {!isActive && (
        <div className="h-px bg-zinc-100" />
      )}
    </a>
  );
}
