import { useState } from "react";
import type { JobStatusValue } from "../types";
import { cancelJob, restartJob, deleteJob } from "../api";

export function ActionButtons({
  jobId,
  status,
  serverBase = "",
  onDeleted,
}: {
  jobId: string;
  status: JobStatusValue;
  /** Base URL for REST calls (e.g. "http://<host>:8000"). Empty = same origin. */
  serverBase?: string;
  onDeleted?: () => void;
}) {
  const [busy, setBusy] = useState(false);

  const canCancel = status === "queued" || status === "running";
  const canRestart = status === "failed" || status === "cancelled";
  const canDelete =
    status === "completed" ||
    status === "failed" ||
    status === "cancelled" ||
    status === "interrupted";

  async function act(fn: () => Promise<unknown>, after?: () => void) {
    setBusy(true);
    try {
      await fn();
      after?.();
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(false);
    }
  }

  const base =
    "inline-flex items-center px-3 py-1.5 text-xs font-medium rounded-md transition-colors disabled:opacity-40";

  return (
    <div className="flex gap-2">
      {canCancel && (
        <button
          disabled={busy}
          onClick={() => act(() => cancelJob(jobId, serverBase))}
          className={`${base} bg-amber-50 text-amber-700 hover:bg-amber-100`}
        >
          Cancel
        </button>
      )}
      {canRestart && (
        <button
          disabled={busy}
          onClick={() => act(() => restartJob(jobId, serverBase))}
          className={`${base} bg-blue-50 text-blue-700 hover:bg-blue-100`}
        >
          Restart
        </button>
      )}
      {canDelete && (
        <button
          disabled={busy}
          onClick={() => act(() => deleteJob(jobId, serverBase), onDeleted)}
          className={`${base} bg-red-50 text-red-700 hover:bg-red-100`}
        >
          Delete
        </button>
      )}
    </div>
  );
}
