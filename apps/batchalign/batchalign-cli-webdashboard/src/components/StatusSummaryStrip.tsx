import type { FileCounts, FilterTab } from "../hooks/useFileFilters";

const SEGMENTS: Array<{ key: keyof FileCounts; color: string; label: string; tab: FilterTab }> = [
  { key: "done", color: "bg-emerald-500", label: "done", tab: "done" },
  { key: "error", color: "bg-red-500", label: "error", tab: "error" },
  { key: "processing", color: "bg-blue-500", label: "processing", tab: "processing" },
  { key: "queued", color: "bg-zinc-300", label: "queued", tab: "queued" },
];

export function StatusSummaryStrip({
  counts,
  onStatusClick,
}: {
  counts: FileCounts;
  onStatusClick: (tab: FilterTab) => void;
}) {
  const total = counts.all;
  if (total === 0) return null;

  const pct = total > 0 ? Math.round(((counts.done + counts.error) / total) * 100) : 0;

  return (
    <div className="flex items-center gap-3">
      {/* Segmented bar */}
      <div className="flex-1 h-2 bg-zinc-100 rounded-full overflow-hidden flex">
        {SEGMENTS.map(({ key, color }) => {
          const w = (counts[key] / total) * 100;
          if (w === 0) return null;
          return (
            <div
              key={key}
              className={`h-full ${color} transition-all duration-500`}
              style={{ width: `${w}%` }}
            />
          );
        })}
      </div>

      {/* Percentage */}
      <span className="text-xs font-mono text-zinc-500 w-8 text-right shrink-0">
        {pct}%
      </span>

      {/* Clickable counts */}
      <div className="flex items-center gap-2 text-xs shrink-0">
        {SEGMENTS.map(({ key, color, label, tab }) => {
          const count = counts[key];
          if (count === 0) return null;
          return (
            <button
              key={key}
              type="button"
              className="inline-flex items-center gap-1 hover:underline cursor-pointer text-zinc-600"
              onClick={() => onStatusClick(tab)}
            >
              <span className={`inline-block w-1.5 h-1.5 rounded-full ${color}`} />
              <span className="font-mono">{count}</span>
              <span>{label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
