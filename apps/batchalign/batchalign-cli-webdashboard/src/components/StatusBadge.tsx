import { statusDotColor } from "../utils";

export function StatusBadge({ status }: { status: string }) {
  const isRunning = status === "running" || status === "processing";
  return (
    <span className="inline-flex items-center gap-1.5">
      <span
        className={`inline-block w-2 h-2 rounded-full ${statusDotColor(status)} ${
          isRunning ? "status-dot-pulse" : ""
        }`}
      />
      <span className="text-xs text-zinc-500 capitalize">{status}</span>
    </span>
  );
}
