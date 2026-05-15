import { useState } from "react";
import type { ErrorGroup } from "../hooks/useFileFilters";
import { ErrorCodeGroup } from "./ErrorCodeGroup";

const CATEGORY_COLORS: Record<string, string> = {
  input: "border-amber-200",
  media: "border-purple-200",
  system: "border-red-200",
  processing: "border-orange-200",
  validation: "border-rose-300",
};

const CATEGORY_TEXT: Record<string, string> = {
  input: "text-amber-700",
  media: "text-purple-700",
  system: "text-red-700",
  processing: "text-orange-700",
  validation: "text-rose-700",
};

export function ErrorPanel({ errorGroups }: { errorGroups: ErrorGroup[] }) {
  const [collapsed, setCollapsed] = useState(false);

  if (errorGroups.length === 0) return null;

  const totalErrors = errorGroups.reduce((sum, g) => sum + g.totalFiles, 0);

  return (
    <div className="bg-red-50/50 border border-red-100 rounded-lg overflow-hidden">
      {/* Header */}
      <button
        type="button"
        className="w-full flex items-center gap-2 px-4 py-2.5 text-left cursor-pointer hover:bg-red-50/80"
        onClick={() => setCollapsed(!collapsed)}
      >
        <span className="text-[10px] text-zinc-400">
          {collapsed ? "\u25B6" : "\u25BC"}
        </span>
        <span className="text-sm font-medium text-red-700">
          {totalErrors} {totalErrors === 1 ? "error" : "errors"}
        </span>
        <span className="text-xs text-red-500">
          {errorGroups.map((g) => `${g.categoryLabel} (${g.totalFiles})`).join(" \u00b7 ")}
        </span>
      </button>

      {/* Body */}
      {!collapsed && (
        <div className="px-4 pb-3">
          {errorGroups.map((group) => (
            <div
              key={group.category}
              className={`border-l-2 ${CATEGORY_COLORS[group.category] ?? "border-zinc-200"} pl-3 mb-3 last:mb-0`}
            >
              {/* Category header */}
              <div className="flex items-center gap-2 mb-1">
                <span
                  className={`text-xs font-semibold ${CATEGORY_TEXT[group.category] ?? "text-zinc-600"}`}
                >
                  {group.categoryLabel}
                </span>
                <span className="text-[10px] text-zinc-400">
                  ({group.totalFiles} {group.totalFiles === 1 ? "file" : "files"})
                </span>
              </div>

              {/* Validation bug banner */}
              {group.category === "validation" && (
                <p className="text-[11px] text-rose-600 mb-2 italic">
                  This is a pipeline bug, not your input. A diagnostic report has been filed automatically.
                </p>
              )}

              {/* Error code groups */}
              {group.codeGroups.map((cg) => (
                <ErrorCodeGroup key={cg.code} group={cg} />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
