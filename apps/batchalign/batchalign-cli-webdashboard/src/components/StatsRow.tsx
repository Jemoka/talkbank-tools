import { useFilteredStats } from "../state";

export function StatsRow() {
  const s = useFilteredStats();
  const total = s.active + s.completed + s.failed;
  if (total === 0) return null;

  return (
    <div className="flex items-center gap-2 text-xs text-zinc-500 mb-4">
      <span className="text-zinc-400">{total} jobs</span>
      <span className="text-zinc-300">&middot;</span>
      {s.active > 0 && (
        <>
          <span className="text-blue-600 font-medium">{s.active} active</span>
          <span className="text-zinc-300">&middot;</span>
        </>
      )}
      <span>{s.completed} completed</span>
      {s.failed > 0 && (
        <>
          <span className="text-zinc-300">&middot;</span>
          <span className="text-red-600 font-medium">{s.failed} failed</span>
        </>
      )}
      <span className="text-zinc-300">&middot;</span>
      <span>{s.totalFiles.toLocaleString()} files</span>
    </div>
  );
}
