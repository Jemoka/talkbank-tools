/** Operational counters panel ("vital signs").
 *
 * Compact display of server-lifetime counters from the health endpoint:
 * worker crashes, forced terminations, memory gate rejections, and
 * throughput stats. Designed for the dashboard right column.
 */

import type { HealthResponse } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface Vital {
  label: string;
  value: number;
  /** Show in red/amber when nonzero. */
  severity: "neutral" | "warning" | "error";
}

function buildVitals(health: HealthResponse): Vital[] {
  const vitals: Vital[] = [];

  const crashes = health.worker_crashes ?? 0;
  if (crashes > 0) {
    vitals.push({ label: "crashes", value: crashes, severity: "error" });
  }

  const forced = health.forced_terminal_errors ?? 0;
  if (forced > 0) {
    vitals.push({ label: "forced kills", value: forced, severity: "error" });
  }

  const gateAborts = health.memory_gate_aborts ?? 0;
  if (gateAborts > 0) {
    vitals.push({
      label: "gate rejects",
      value: gateAborts,
      severity: "warning",
    });
  }

  const started = health.attempts_started ?? 0;
  if (started > 0) {
    vitals.push({ label: "attempts", value: started, severity: "neutral" });
  }

  const retried = health.attempts_retried ?? 0;
  if (retried > 0) {
    vitals.push({ label: "retries", value: retried, severity: "warning" });
  }

  const deferred = health.deferred_work_units ?? 0;
  if (deferred > 0) {
    vitals.push({ label: "deferred", value: deferred, severity: "neutral" });
  }

  return vitals;
}

const SEVERITY_STYLES = {
  neutral: "text-gray-600 bg-gray-50",
  warning: "text-amber-700 bg-amber-50",
  error: "text-red-700 bg-red-50",
} as const;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface VitalsRowProps {
  health: HealthResponse | undefined;
}

/** Compact operational counter badges.
 *
 * Only renders when there are nonzero counters worth showing.
 * Errors (crashes, forced kills) appear first in red, then warnings
 * in amber, then neutral throughput stats in gray.
 */
export function VitalsRow({ health }: VitalsRowProps) {
  if (!health) return null;

  const vitals = buildVitals(health);
  if (vitals.length === 0) return null;

  return (
    <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
      <div className="flex items-center gap-2 px-4 py-2.5 border-b border-gray-100">
        <svg
          className="w-4 h-4 text-gray-400"
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth={1.5}
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 0 1 3 19.875v-6.75ZM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V8.625ZM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V4.125Z"
          />
        </svg>
        <span className="text-sm font-semibold text-gray-700">Vitals</span>
      </div>

      <div className="flex flex-wrap gap-2 px-4 py-3">
        {vitals.map((v) => (
          <span
            key={v.label}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-mono ${SEVERITY_STYLES[v.severity]}`}
          >
            <span className="font-semibold">{v.value}</span>
            <span className="font-sans text-[11px] opacity-70">{v.label}</span>
          </span>
        ))}
      </div>
    </div>
  );
}
