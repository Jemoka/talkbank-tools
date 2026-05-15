/**
 * Temporal workflow metadata panel for the job detail view.
 *
 * Displayed when the job's control_plane.backend is "temporal". Shows
 * workflow ID, run ID, temporal status, task queue, and history length.
 * Links to the Temporal UI if available.
 */

import type { JobControlPlaneInfo } from "../types";

/** Default Temporal UI URL (dev server). Operators should configure this. */
const TEMPORAL_UI_BASE = "http://localhost:8233";

export function TemporalPanel({
  controlPlane,
}: {
  controlPlane: JobControlPlaneInfo | null | undefined;
}) {
  if (!controlPlane || controlPlane.backend !== "temporal") return null;

  const temporal = controlPlane.temporal;
  if (!temporal) return null;

  const workflowUrl = temporal.workflow_id
    ? `${TEMPORAL_UI_BASE}/namespaces/default/workflows/${temporal.workflow_id}/${temporal.run_id ?? ""}`
    : null;

  return (
    <div className="mt-4 rounded-lg border border-indigo-100 bg-indigo-50/50 px-4 py-3">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-indigo-600 mb-2">
        Temporal Workflow
      </h3>
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-sm">
        <Row label="Backend" value={controlPlane.backend} />
        <Row label="Status" value={temporal.status ?? "unknown"} />
        <Row label="Workflow ID" value={temporal.workflow_id ?? "—"} mono />
        <Row label="Run ID" value={temporal.run_id ?? "—"} mono />
        <Row label="Task Queue" value={temporal.task_queue ?? "—"} />
        <Row
          label="History"
          value={
            temporal.history_length != null
              ? `${temporal.history_length} events`
              : "—"
          }
        />
        {temporal.describe_error && (
          <div className="col-span-2 mt-1 text-xs text-red-600">
            Describe error: {temporal.describe_error}
          </div>
        )}
      </div>
      {workflowUrl && (
        <a
          href={workflowUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="mt-2 inline-block text-xs text-indigo-600 hover:text-indigo-800 underline"
        >
          Open in Temporal UI →
        </a>
      )}
    </div>
  );
}

function Row({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <>
      <span className="text-zinc-500">{label}</span>
      <span className={mono ? "font-mono text-xs" : ""}>{value}</span>
    </>
  );
}
