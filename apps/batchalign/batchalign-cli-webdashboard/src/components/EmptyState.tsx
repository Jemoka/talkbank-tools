export function EmptyState() {
  return (
    <div className="text-center py-16">
      <div className="inline-flex items-center justify-center w-12 h-12 rounded-full bg-zinc-100 mb-4">
        <span className="text-zinc-400 text-lg">$</span>
      </div>
      <p className="text-sm text-zinc-400 font-medium">No jobs yet</p>
      <p className="text-xs text-zinc-300 mt-1.5 font-mono">
        batchalign3 transcribe input/ output/ --server ...
      </p>
    </div>
  );
}
