import { useState, useCallback } from "react";
import type { FileStatusEntry } from "../types";
import { displayProgressLabel, statusDotColor } from "../utils";
import { PipelineStageBar } from "./PipelineStageBar";

type SortCol = "file" | "status" | "duration";
type SortDir = "asc" | "desc";

const STATUS_RANK: Record<string, number> = {
  processing: 0,
  queued: 1,
  error: 2,
  done: 3,
  interrupted: 4,
};

function fileDuration(f: FileStatusEntry): number {
  if (f.started_at != null && f.finished_at != null) return f.finished_at - f.started_at;
  return 0;
}

function compareFiles(a: FileStatusEntry, b: FileStatusEntry, col: SortCol, dir: SortDir): number {
  let cmp = 0;
  switch (col) {
    case "file":
      cmp = a.filename.localeCompare(b.filename);
      break;
    case "status":
      cmp = (STATUS_RANK[a.status] ?? 9) - (STATUS_RANK[b.status] ?? 9);
      break;
    case "duration":
      cmp = fileDuration(a) - fileDuration(b);
      break;
  }
  return dir === "desc" ? -cmp : cmp;
}

const CATEGORY_LABELS: Record<string, string> = {
  input: "Parse",
  media: "Media",
  system: "System",
  processing: "Engine",
  validation: "Pipeline Bug",
};

const CATEGORY_COLORS: Record<string, string> = {
  input: "bg-amber-50 text-amber-600",
  media: "bg-purple-50 text-purple-600",
  system: "bg-red-50 text-red-600",
  processing: "bg-orange-50 text-orange-600",
  validation: "bg-rose-50 text-rose-600",
};

function errorSnippet(error: string | null | undefined): string {
  if (!error) return "Unknown error";
  const first = error.split("\n")[0];
  return first.length > 80 ? first.slice(0, 80) + "\u2026" : first;
}

/** Split a filename into (directory, basename). */
function splitPath(filename: string): [string, string] {
  const idx = filename.lastIndexOf("/");
  if (idx === -1) return ["", filename];
  return [filename.slice(0, idx), filename.slice(idx + 1)];
}

/** Group files by directory, sorted alphabetically within each group. */
function groupByDirectory(
  files: FileStatusEntry[],
): Array<{ dir: string; files: FileStatusEntry[] }> {
  const map = new Map<string, FileStatusEntry[]>();
  for (const f of files) {
    const [dir] = splitPath(f.filename);
    const group = map.get(dir);
    if (group) {
      group.push(f);
    } else {
      map.set(dir, [f]);
    }
  }

  // Sort groups: root first, then alphabetically. Sort files within each group alphabetically.
  return [...map.entries()]
    .sort(([a], [b]) => {
      if (a === "" && b !== "") return -1;
      if (a !== "" && b === "") return 1;
      return a.localeCompare(b);
    })
    .map(([dir, files]) => ({
      dir,
      files: [...files].sort((a, b) => a.filename.localeCompare(b.filename)),
    }));
}

function DirStats({ files }: { files: FileStatusEntry[] }) {
  const done = files.filter((f) => f.status === "done").length;
  const errs = files.filter((f) => f.status === "error").length;
  const active = files.filter(
    (f) => f.status === "processing" || f.status === "queued",
  ).length;
  return (
    <span className="text-[11px] text-zinc-400 font-mono ml-2">
      {done > 0 && <span className="text-emerald-500">{done} done</span>}
      {errs > 0 && (
        <span className={done > 0 ? "ml-2 text-red-500" : "text-red-500"}>
          {errs} err
        </span>
      )}
      {active > 0 && (
        <span className={(done > 0 || errs > 0) ? "ml-2 text-blue-500" : "text-blue-500"}>
          {active} active
        </span>
      )}
    </span>
  );
}

export function FileTable({ files }: { files: FileStatusEntry[] }) {
  const [expandedErr, setExpandedErr] = useState<string | null>(null);
  const [collapsedDirs, setCollapsedDirs] = useState<Set<string>>(new Set());
  const [sortCol, setSortCol] = useState<SortCol>("file");
  const [sortDir, setSortDir] = useState<SortDir>("asc");

  const toggleSort = useCallback((col: SortCol) => {
    setSortCol((prev) => {
      if (prev === col) {
        setSortDir((d) => (d === "asc" ? "desc" : "asc"));
        return col;
      }
      setSortDir(col === "duration" ? "desc" : "asc");
      return col;
    });
  }, []);

  if (files.length === 0) {
    return <p className="text-xs text-zinc-400">No files</p>;
  }

  // Sort files before grouping
  const sortedFiles = [...files].sort((a, b) => compareFiles(a, b, sortCol, sortDir));
  const groups = groupByDirectory(sortedFiles);
  const hasMultipleDirs = groups.length > 1 || (groups.length === 1 && groups[0].dir !== "");

  function toggleDir(dir: string) {
    setCollapsedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dir)) next.delete(dir);
      else next.add(dir);
      return next;
    });
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-[11px] text-zinc-400 uppercase tracking-wider border-b border-zinc-200">
            {(["file", "status", "duration"] as SortCol[]).map((col) => {
              const labels: Record<SortCol, string> = { file: "File", status: "Status", duration: "Duration" };
              const arrow = sortCol === col ? (sortDir === "asc" ? " ↑" : " ↓") : "";
              return (
                <th
                  key={col}
                  className={`py-2 ${col !== "duration" ? "pr-4" : ""} font-medium cursor-pointer hover:text-zinc-600 select-none`}
                  onClick={() => toggleSort(col)}
                  aria-sort={sortCol === col ? (sortDir === "asc" ? "ascending" : "descending") : "none"}
                >
                  {labels[col]}{arrow}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {groups.map((group) => {
            const isCollapsed = collapsedDirs.has(group.dir);
            return (
              <DirGroup
                key={group.dir}
                dir={group.dir}
                files={group.files}
                showDirHeader={hasMultipleDirs}
                isCollapsed={isCollapsed}
                onToggle={() => toggleDir(group.dir)}
                expandedErr={expandedErr}
                setExpandedErr={setExpandedErr}
              />
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function DirGroup({
  dir,
  files,
  showDirHeader,
  isCollapsed,
  onToggle,
  expandedErr,
  setExpandedErr,
}: {
  dir: string;
  files: FileStatusEntry[];
  showDirHeader: boolean;
  isCollapsed: boolean;
  onToggle: () => void;
  expandedErr: string | null;
  setExpandedErr: (v: string | null) => void;
}) {
  return (
    <>
      {showDirHeader && (
        <tr
          className="cursor-pointer select-none hover:bg-zinc-50"
          onClick={onToggle}
        >
          <td colSpan={3} className="py-1.5 pl-1">
            <div className="flex items-center gap-1.5">
              <span className="text-[10px] text-zinc-400">
                {isCollapsed ? "\u25B6" : "\u25BC"}
              </span>
              <span className="text-xs font-mono text-zinc-600 font-medium">
                {dir || "."}
              </span>
              <span className="text-[11px] text-zinc-400">
                ({files.length})
              </span>
              <DirStats files={files} />
            </div>
          </td>
        </tr>
      )}
      {!isCollapsed &&
        files.map((f) => (
          <FileRow
            key={f.filename}
            file={f}
            indent={showDirHeader}
            expandedErr={expandedErr}
            setExpandedErr={setExpandedErr}
          />
        ))}
    </>
  );
}

function FileRow({
  file: f,
  indent,
  expandedErr,
  setExpandedErr,
}: {
  file: FileStatusEntry;
  indent: boolean;
  expandedErr: string | null;
  setExpandedErr: (v: string | null) => void;
}) {
  const [, basename] = splitPath(f.filename);
  const dur =
    f.started_at != null && f.finished_at != null
      ? `${(f.finished_at - f.started_at).toFixed(1)}s`
      : "";
  const hasErr = f.status === "error";
  const isLongError = hasErr && f.error != null && f.error.length > 80;
  const isExpanded = expandedErr === f.filename;
  const isProcessing = f.status === "processing";
  const errorCodes = hasErr && f.error_codes ? f.error_codes : [];
  const progressLabel = displayProgressLabel(f.progress_stage, f.progress_label);

  return (
    <>
      <tr
        className={`border-b border-zinc-50 ${
          hasErr && isLongError ? "cursor-pointer hover:bg-zinc-50" : ""
        }`}
        onClick={() =>
          hasErr &&
          isLongError &&
          setExpandedErr(isExpanded ? null : f.filename)
        }
      >
        <td className={`py-1.5 pr-4 font-mono text-xs ${indent ? "pl-5" : ""}`}>
          {basename}
        </td>
        <td className="py-1.5 pr-4">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={`inline-block w-1.5 h-1.5 rounded-full flex-shrink-0 ${statusDotColor(f.status)} ${
                isProcessing ? "status-dot-pulse" : ""
              }`}
            />
            <span className="text-xs text-zinc-500 capitalize">{f.status}</span>
            {hasErr && f.error_category && (
              <span
                className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium ${
                  CATEGORY_COLORS[f.error_category] ?? "bg-zinc-100 text-zinc-500"
                }`}
              >
                {CATEGORY_LABELS[f.error_category] ?? f.error_category}
              </span>
            )}
            {errorCodes.map((code) => (
              <a
                key={code}
                href={`https://talkbank.org/errors/${code}`}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => e.stopPropagation()}
                className="inline-flex items-center px-1 py-0.5 rounded bg-red-100 text-red-700 text-[10px] font-mono font-semibold hover:bg-red-200 transition-colors no-underline"
                title={`View documentation for ${code}`}
              >
                {code}
              </a>
            ))}
            {hasErr && f.error_line != null && (
              <span className="text-[10px] font-mono text-zinc-400">
                line {f.error_line}
              </span>
            )}
            {hasErr && (
              <span className="text-[11px] text-red-500 truncate max-w-xs">
                {errorSnippet(f.error)}
              </span>
            )}
            {/* Pipeline phase indicator */}
            {isProcessing && f.progress_stage && (
              <PipelineStageBar stage={f.progress_stage} />
            )}
            {/* Sub-file progress with counter (has current/total) */}
            {isProcessing &&
              f.progress_total != null &&
              f.progress_total > 0 && (
                <div className="flex items-center gap-1.5 min-w-0">
                  <div className="w-16 h-1 bg-zinc-200 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-blue-500 rounded-full progress-striped transition-all"
                      style={{
                        width: `${Math.round(
                          ((f.progress_current ?? 0) / f.progress_total) * 100,
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="text-[10px] text-zinc-400 whitespace-nowrap">
                    {progressLabel && (
                      <span className="mr-1">{progressLabel}</span>
                    )}
                    {f.progress_current ?? 0}/{f.progress_total}
                  </span>
                </div>
              )}
            {/* Stage label only (no current/total) */}
            {isProcessing &&
              progressLabel &&
              !(f.progress_total != null && f.progress_total > 0) && (
                <span className="text-[10px] text-zinc-400 italic">
                  {progressLabel}
                </span>
              )}
          </div>
        </td>
        <td className="py-1.5 text-xs text-zinc-400 font-mono">{dur}</td>
      </tr>
      {hasErr && isLongError && isExpanded && (
        <tr>
          <td
            colSpan={3}
            className="py-2 px-4 bg-red-50 text-[11px] text-red-700 font-mono whitespace-pre-wrap"
          >
            {f.error ?? "Unknown error"}
          </td>
        </tr>
      )}
    </>
  );
}
