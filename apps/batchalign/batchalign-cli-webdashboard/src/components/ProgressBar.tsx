import { progressPercent } from "../utils";

export function ProgressBar({
  completed,
  total,
  animated = false,
}: {
  completed: number;
  total: number;
  animated?: boolean;
}) {
  const pct = progressPercent(completed, total);
  const indeterminate = animated && pct === 0;

  return (
    <div className="w-full bg-zinc-200 rounded-full h-1.5 overflow-hidden">
      {indeterminate ? (
        <div className="h-full rounded-full bg-blue-400/60 progress-indeterminate" />
      ) : (
        <div
          className={`h-full rounded-full transition-all duration-500 ${
            animated ? "bg-blue-500 progress-striped" : "bg-blue-500"
          }`}
          style={{ width: `${pct}%` }}
        />
      )}
    </div>
  );
}
