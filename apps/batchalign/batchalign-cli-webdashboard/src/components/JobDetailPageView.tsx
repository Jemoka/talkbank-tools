/** Presentational job-detail view.
 *
 * This component owns route-local view logic such as file filters, pagination,
 * and section layout. It deliberately does not fetch data or discover which
 * server owns the job; those concerns live in `useJobPageController`.
 */
import { useState } from "react";
import { Layout } from "./Layout";
import { ProgressBar } from "./ProgressBar";
import { ActionButtons } from "./ActionButtons";
import { TemporalPanel } from "./TemporalPanel";
import { BatchProgressPanel } from "./BatchProgressPanel";
import { StatusSummaryStrip } from "./StatusSummaryStrip";
import { ErrorPanel } from "./ErrorPanel";
import { FilterTabs } from "./FilterTabs";
import { PaginatedFileList } from "./PaginatedFileList";
import { useFileFilters } from "../hooks/useFileFilters";
import type { JobInfo, JobListItem } from "../types";

/** Tiny copy-to-clipboard button that shows a brief "Copied" tooltip. */
function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard API may be blocked in some contexts
    }
  }
  return (
    <button
      type="button"
      onClick={handleCopy}
      className="inline-flex items-center text-zinc-300 hover:text-zinc-500 transition-colors ml-1.5"
      aria-label={`Copy ${label}`}
      title={copied ? "Copied!" : `Copy ${label}`}
    >
      {copied ? (
        <svg className="w-3.5 h-3.5 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
        </svg>
      ) : (
        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
            d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
        </svg>
      )}
    </button>
  );
}
import {
  commandStyle,
  formatJsonDisplay,
  formatDuration,
  progressPercent,
  relativeTime,
  shortPath,
  statusDotColor,
  submitterName,
  displayLang,
  isDefaultLang,
} from "../utils";

// ---------------------------------------------------------------------------
// CommandOptionsPanel — readable display for the job's typed options
// ---------------------------------------------------------------------------

/** Human-readable labels for option fields that matter operationally.
 *
 * Fields not listed here are omitted from the readable view (they are
 * either internal, always-default, or not helpful for debugging).
 */
const OPTION_LABELS: Record<string, string> = {
  fa_engine: "FA engine",
  asr_engine: "ASR engine",
  utr_engine: "UTR engine",
  utr_overlap_strategy: "UTR overlap",
  retokenize: "Retokenize",
  skipmultilang: "Skip multi-lang",
  diarize: "Diarize",
  pauses: "Pauses",
  wor: "Wor tier",
  merge_abbrev: "Merge abbreviations",
  batch_size: "Batch size",
  batch_window: "Batch window",
  feature_set: "Feature set",
  text_cache: "Text cache",
  override_media_cache: "Override media cache",
  media_dir: "Media directory",
};

/** Fields to skip in the readable display because they are either always
 * present as a discriminator, always default, or opaque data blobs. */
const SKIP_FIELDS = new Set([
  "command",
  "engine_overrides",
  "mwt",
  "debug_dir",
  "override_media_cache_tasks",
  "utr_two_pass",
]);

/** Render a single option value as a readable string. */
function formatOptionValue(value: unknown): string {
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "number") return String(value);
  if (typeof value === "string") return value;
  if (value == null) return "none";
  return JSON.stringify(value);
}

/** Readable command options panel.
 *
 * Renders interesting (non-default, non-internal) option fields as a
 * two-column label/value list. Falls back to raw JSON for unrecognized
 * option shapes.
 */
function CommandOptionsPanel({ options }: { options: unknown }) {
  if (options == null) return null;
  if (typeof options !== "object" || Array.isArray(options)) {
    const raw = formatJsonDisplay(options);
    if (!raw) return null;
    return (
      <pre className="overflow-x-auto rounded-md border border-zinc-200 bg-zinc-50 px-3 py-2 text-[11px] leading-5 text-zinc-700">
        {raw}
      </pre>
    );
  }

  const obj = options as Record<string, unknown>;
  const entries: Array<{ label: string; value: string }> = [];

  // Engine overrides get special treatment: show each override inline
  const overrides = obj.engine_overrides;
  if (overrides && typeof overrides === "object" && !Array.isArray(overrides)) {
    for (const [key, val] of Object.entries(overrides as Record<string, unknown>)) {
      if (val != null && val !== "") {
        entries.push({
          label: `Engine override (${key})`,
          value: formatOptionValue(val),
        });
      }
    }
  }

  // Show all non-skipped fields with human labels
  for (const [key, val] of Object.entries(obj)) {
    if (SKIP_FIELDS.has(key)) continue;
    // Skip false booleans, zero-like defaults, and empty strings for cleaner display
    if (val === false || val === "" || val === null || val === undefined) continue;
    // Skip default-ish values that clutter the display
    if (key === "batch_window" && val === 25) continue;
    if (key === "batch_size" && val === 8) continue;

    const label = OPTION_LABELS[key] ?? key.replace(/_/g, " ");
    entries.push({ label, value: formatOptionValue(val) });
  }

  if (entries.length === 0) return null;

  return (
    <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
      {entries.map((e) => (
        <div key={e.label} className="contents">
          <span className="text-zinc-400 whitespace-nowrap">{e.label}</span>
          <span className="font-mono text-zinc-700 truncate" title={e.value}>
            {e.value}
          </span>
        </div>
      ))}
    </div>
  );
}

/** Collapsible options section with readable and raw JSON views.
 *
 * Shows the labeled readable view by default, with a toggle to switch
 * to the raw JSON for full-fidelity debugging and copy-paste.
 */
function OptionsSection({ options, rawJson }: { options: unknown; rawJson: string }) {
  const [showRaw, setShowRaw] = useState(false);

  return (
    <div className="mt-4">
      <div className="flex items-center gap-2 mb-1.5">
        <span className="text-[11px] text-zinc-400 uppercase tracking-wider">
          Options
        </span>
        {rawJson && (
          <>
            <button
              type="button"
              onClick={() => setShowRaw(!showRaw)}
              className="text-[11px] text-zinc-400 hover:text-zinc-600 transition-colors underline decoration-dotted"
            >
              {showRaw ? "readable" : "raw JSON"}
            </button>
            <CopyButton text={rawJson} label="options JSON" />
          </>
        )}
      </div>

      {showRaw ? (
        <pre className="overflow-x-auto rounded-md border border-zinc-200 bg-zinc-50 px-3 py-2 text-[11px] leading-5 text-zinc-700">
          {rawJson}
        </pre>
      ) : (
        <div className="rounded-md border border-zinc-200 bg-zinc-50 px-3 py-2.5">
          <CommandOptionsPanel options={options} />
        </div>
      )}
    </div>
  );
}

/** Inputs required to render a fully resolved job detail page. */
type JobDetailPageViewProps = {
  detail: JobInfo;
  wsJob: JobListItem | undefined;
  multiServer: boolean;
  effectiveServer: string;
  serverBase: string;
  onDeleted: () => void;
};

/** Render one job detail page from controller-supplied state plus live summary data. */
export function JobDetailPageView({
  detail,
  wsJob,
  multiServer,
  effectiveServer,
  serverBase,
  onDeleted,
}: JobDetailPageViewProps) {
  const fileStatuses = detail.file_statuses;
  const {
    activeTab,
    setActiveTab,
    searchQuery,
    setSearchQuery,
    page,
    setPage,
    counts,
    errorGroups,
    filteredFiles,
    pageFiles,
    totalPages,
    pageSize,
  } = useFileFilters(fileStatuses);

  const completedFiles = wsJob?.completed_files ?? detail.completed_files;
  const currentStatus = wsJob?.status ?? detail.status;
  const isActive = currentStatus === "queued" || currentStatus === "running";
  const isRunning = currentStatus === "running";
  const pct = progressPercent(completedFiles, detail.total_files);
  const [cmdBg, cmdText] = commandStyle(detail.command);
  const host = submitterName(detail.submitted_by_name, detail.submitted_by);
  const commandArgsJson = formatJsonDisplay(detail.options);
  const hasOptions = detail.options != null && (typeof detail.options !== "object" || Object.keys(detail.options as Record<string, unknown>).length > 0);

  return (
    <Layout>
      {/* Navigation stays in the view so route shells do not accumulate markup. */}
      <div className="mb-5">
        <a
          href="/dashboard"
          className="text-xs text-zinc-400 hover:text-zinc-600 transition-colors no-underline"
        >
          &larr; Back to jobs
        </a>
      </div>

      <div className="bg-white rounded-lg border border-zinc-200">
        {/* Header and job-scoped action controls. */}
        <div className="px-5 pt-5 pb-4 border-b border-zinc-100">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="flex items-center gap-3 mb-2">
                <span
                  className={`inline-block px-2.5 py-1 rounded text-xs font-mono font-semibold uppercase tracking-wider ${cmdBg} ${cmdText}`}
                >
                  {detail.command}
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span
                    className={`inline-block w-2 h-2 rounded-full ${statusDotColor(currentStatus)} ${
                      isRunning ? "status-dot-pulse" : ""
                    }`}
                  />
                  <span className="text-sm text-zinc-500 capitalize">
                    {currentStatus}
                  </span>
                </span>
                {multiServer && effectiveServer && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-zinc-100 text-zinc-400">
                    {effectiveServer}
                  </span>
                )}
              </div>

              <span className="font-mono text-xs text-zinc-400">
                {detail.job_id}
                <CopyButton text={detail.job_id} label="job ID" />
              </span>
            </div>

            <ActionButtons
              jobId={detail.job_id}
              status={currentStatus}
              serverBase={serverBase}
              onDeleted={onDeleted}
            />
          </div>
        </div>

        {/* Static metadata and summary counts. */}
        <div className="px-5 py-4 border-b border-zinc-100">
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-y-3 gap-x-6 text-sm">
            <div>
              <div className="text-[11px] text-zinc-400 uppercase tracking-wider mb-0.5">
                Files
              </div>
              <div className="font-mono text-zinc-700">
                {completedFiles}
                <span className="text-zinc-300"> / </span>
                {detail.total_files}
                {isActive && (
                  <span className="text-zinc-400 text-xs ml-1">({pct}%)</span>
                )}
              </div>
            </div>
            <div>
              <div className="text-[11px] text-zinc-400 uppercase tracking-wider mb-0.5">
                Submitted
              </div>
              <div className="text-zinc-700">{relativeTime(detail.submitted_at)}</div>
            </div>
            <div>
              <div className="text-[11px] text-zinc-400 uppercase tracking-wider mb-0.5">
                Duration
              </div>
              <div className="font-mono text-zinc-700">
                {formatDuration(detail.duration_s) || "\u2014"}
              </div>
            </div>
            <div>
              <div className="text-[11px] text-zinc-400 uppercase tracking-wider mb-0.5">
                Workers
              </div>
              <div className="font-mono text-zinc-700">{detail.num_workers ?? "\u2014"}</div>
            </div>
          </div>

          {detail.source_dir && (
            <div
              className="mt-3 text-xs text-zinc-500 font-mono truncate"
              title={detail.source_dir}
            >
              {shortPath(detail.source_dir)}
            </div>
          )}

          {(host || (detail.lang && !isDefaultLang(detail.lang))) && (
            <div className="flex items-center gap-3 mt-3 text-xs text-zinc-400">
              {host && (
                <span>
                  <span className="text-zinc-500">from</span>{" "}
                  <span className="font-mono">{host}</span>
                </span>
              )}
              {detail.lang && !isDefaultLang(detail.lang) && (
                <span className="font-mono uppercase">{displayLang(detail.lang)}</span>
              )}
            </div>
          )}

          {/* Original submission options are critical for debugging reruns and
              understanding exactly which engine/flags produced the job.
              The readable view extracts labeled option fields; the raw JSON
              toggle preserves full fidelity for copy-paste debugging. */}
          {hasOptions && (
            <OptionsSection options={detail.options} rawJson={commandArgsJson} />
          )}

          {/* Temporal workflow metadata (only when backend is temporal). */}
          <TemporalPanel controlPlane={detail.control_plane} />
        </div>

        {/* Progress is only meaningful for active jobs. */}
        {isActive && (
          <div className="px-5 py-3 border-b border-zinc-100">
            <ProgressBar
              completed={completedFiles}
              total={detail.total_files}
              animated={isRunning}
            />
            {/* Per-language batch progress for morphotag/utseg/translate/coref */}
            <BatchProgressPanel progress={(detail as Record<string, unknown>).batch_progress as never} />
          </div>
        )}

        {/* Job-level failures render above file-level breakdowns. */}
        {detail.error && (
          <div className="mx-5 mt-4 bg-red-50 border border-red-100 rounded-lg p-3 text-sm text-red-700">
            {detail.error}
          </div>
        )}

        {/* File-level summary and filtering controls. */}
        {fileStatuses.length > 0 && (
          <div className="px-5 pt-4">
            <StatusSummaryStrip counts={counts} onStatusClick={setActiveTab} />
          </div>
        )}

        {errorGroups.length > 0 && (
          <div className="px-5 pt-3">
            <ErrorPanel errorGroups={errorGroups} />
          </div>
        )}

        {/* Paginated file table plus tab/search controls. */}
        <div className="px-5 pt-4 pb-5">
          <div className="mb-3">
            <FilterTabs
              activeTab={activeTab}
              counts={counts}
              searchQuery={searchQuery}
              onTabChange={setActiveTab}
              onSearchChange={setSearchQuery}
            />
          </div>

          <PaginatedFileList
            pageFiles={pageFiles}
            page={page}
            totalPages={totalPages}
            totalFiltered={filteredFiles.length}
            pageSize={pageSize}
            onPageChange={setPage}
          />
        </div>
      </div>
    </Layout>
  );
}
