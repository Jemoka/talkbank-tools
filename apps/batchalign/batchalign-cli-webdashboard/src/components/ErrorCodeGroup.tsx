import { useState } from "react";
import type { ErrorCodeGroup as ErrorCodeGroupType } from "../hooks/useFileFilters";
import type { FileStatusEntry } from "../types";

function splitBasename(filename: string): string {
  const idx = filename.lastIndexOf("/");
  return idx === -1 ? filename : filename.slice(idx + 1);
}

function ErrorFileEntry({ file }: { file: FileStatusEntry }) {
  const [expanded, setExpanded] = useState(false);
  const basename = splitBasename(file.filename);
  const hasDetail = file.error != null && file.error.length > 0;

  return (
    <div>
      <div className="flex items-center gap-2 py-0.5">
        <button
          type="button"
          className={`font-mono text-xs ${hasDetail ? "hover:underline cursor-pointer" : ""} text-zinc-700`}
          onClick={() => hasDetail && setExpanded(!expanded)}
        >
          {basename}
        </button>
        {file.error_line != null && (
          <span className="text-[10px] font-mono text-zinc-400">
            line {file.error_line}
          </span>
        )}
        {file.bug_report_id && (
          <span className="text-[10px] text-rose-500 font-mono ml-2">
            Report {file.bug_report_id.slice(0, 8)}
          </span>
        )}
        {hasDetail && (
          <span className="text-[10px] text-zinc-400">
            {expanded ? "\u25BC" : "\u25B6"}
          </span>
        )}
      </div>
      {expanded && hasDetail && (
        <pre className="mt-1 mb-2 ml-4 p-2 bg-red-50 rounded text-[11px] text-red-700 font-mono whitespace-pre-wrap overflow-x-auto max-h-40 overflow-y-auto">
          {file.error}
        </pre>
      )}
    </div>
  );
}

export function ErrorCodeGroup({ group }: { group: ErrorCodeGroupType }) {
  const [collapsed, setCollapsed] = useState(group.files.length > 10);
  const codeDisplay = group.code === "general" ? "General" : group.code;

  return (
    <div className="ml-4 mb-2">
      <button
        type="button"
        className="flex items-center gap-2 text-xs cursor-pointer hover:bg-zinc-50 rounded px-1 py-0.5 -ml-1"
        onClick={() => setCollapsed(!collapsed)}
      >
        <span className="text-[10px] text-zinc-400">
          {collapsed ? "\u25B6" : "\u25BC"}
        </span>
        {group.code !== "general" && (
          <span className="font-mono text-red-600 font-semibold">
            {codeDisplay}
          </span>
        )}
        <span className="text-zinc-600 truncate max-w-sm">
          {group.label}
        </span>
        <span className="text-zinc-400">
          ({group.files.length} {group.files.length === 1 ? "file" : "files"})
        </span>
      </button>
      {!collapsed && (
        <div className="ml-5 mt-1">
          {group.files.map((f) => (
            <ErrorFileEntry key={f.filename} file={f} />
          ))}
        </div>
      )}
    </div>
  );
}
