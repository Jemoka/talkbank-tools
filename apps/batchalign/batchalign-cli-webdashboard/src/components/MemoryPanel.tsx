/** System memory gauge panel.
 *
 * Shows a segmented bar of used vs available RAM with the memory gate
 * threshold marked as a reference line. Designed to sit in the dashboard
 * right column alongside the WorkerProfilePanel.
 *
 * Data source: `system_memory_*` and `memory_gate_*` fields from the
 * `/health` endpoint, stored in the Zustand `healthMap`.
 */

import type { HealthResponse } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Format MB as human-readable (e.g. "148.2 GB" or "3,072 MB"). */
function formatMb(mb: number): string {
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }
  return `${mb.toLocaleString()} MB`;
}

/** Threshold proximity: green / amber / red. */
function gateProximity(
  availableMb: number,
  thresholdMb: number,
): "safe" | "warning" | "danger" {
  if (thresholdMb <= 0) return "safe";
  if (availableMb < thresholdMb * 2) return "danger";
  if (availableMb < thresholdMb * 4) return "warning";
  return "safe";
}

const PROXIMITY_STYLES = {
  safe: "text-emerald-600 bg-emerald-50",
  warning: "text-amber-600 bg-amber-50",
  danger: "text-red-600 bg-red-50",
} as const;

type PressureLevel = "healthy" | "guarded" | "constrained" | "critical";

/** Map host_memory_pressure to display style and label. */
const PRESSURE_BADGE: Record<PressureLevel, { style: string; label: string }> = {
  healthy:     { style: "text-emerald-700 bg-emerald-50", label: "Healthy" },
  guarded:     { style: "text-amber-700 bg-amber-50",     label: "Guarded" },
  constrained: { style: "text-orange-700 bg-orange-50",   label: "Constrained" },
  critical:    { style: "text-red-700 bg-red-50",         label: "Critical" },
} as const;

/** Dot color matching the pressure level badge. */
const PRESSURE_DOT: Record<PressureLevel, string> = {
  healthy: "bg-emerald-500",
  guarded: "bg-amber-500",
  constrained: "bg-orange-500",
  critical: "bg-red-500",
} as const;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface MemoryPanelProps {
  health: HealthResponse | undefined;
}

/** System memory gauge with gate threshold indicator.
 *
 * Communicates at a glance:
 * 1. How much RAM is consumed vs total.
 * 2. How close available memory is to the gate threshold.
 * 3. Whether any jobs have been rejected by the memory gate.
 */
export function MemoryPanel({ health }: MemoryPanelProps) {
  if (!health) return null;

  const totalMb = health.system_memory_total_mb ?? 0;
  const usedMb = health.system_memory_used_mb ?? 0;
  const availableMb = health.system_memory_available_mb ?? 0;
  const gateMb = health.memory_gate_threshold_mb ?? 0;
  const gateAborts = health.memory_gate_aborts ?? 0;

  if (totalMb === 0) return null;

  const pressure = health.host_memory_pressure as PressureLevel | undefined;

  const usedPct = Math.round((usedMb / totalMb) * 100);
  const gatePct = gateMb > 0 ? Math.round(((totalMb - gateMb) / totalMb) * 100) : 0;
  const proximity = gateProximity(availableMb, gateMb);

  return (
    <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-gray-100">
        <div className="flex items-center gap-2">
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
              d="M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25a2.25 2.25 0 0 1-2.25-2.25v-2.25Z"
            />
          </svg>
          <span className="text-sm font-semibold text-gray-700">Memory</span>
        </div>
        <div className="flex items-center gap-2">
          {pressure && PRESSURE_BADGE[pressure] && (
            <span
              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium ${PRESSURE_BADGE[pressure].style}`}
            >
              <span
                className={`inline-block w-1.5 h-1.5 rounded-full ${PRESSURE_DOT[pressure]}`}
              />
              {PRESSURE_BADGE[pressure].label}
            </span>
          )}
          <span className="text-xs text-gray-400 font-mono">
            {formatMb(totalMb)} total
          </span>
        </div>
      </div>

      {/* Gauge */}
      <div className="px-4 pt-3 pb-2">
        <div className="relative w-full h-3 bg-gray-100 rounded-full overflow-hidden">
          {/* Used portion */}
          <div
            className={`absolute inset-y-0 left-0 rounded-full transition-all duration-700 ${
              proximity === "danger"
                ? "bg-red-400"
                : proximity === "warning"
                  ? "bg-amber-400"
                  : "bg-zinc-500"
            }`}
            style={{ width: `${usedPct}%` }}
          />
          {/* Gate threshold marker */}
          {gateMb > 0 && gatePct > 0 && gatePct < 100 && (
            <div
              className="absolute inset-y-0 w-0.5 bg-red-500/60"
              style={{ left: `${gatePct}%` }}
              title={`Memory gate: ${formatMb(gateMb)} threshold`}
            />
          )}
        </div>

        {/* Labels */}
        <div className="flex items-center justify-between mt-2 text-xs">
          <span className="text-gray-500">
            <span className="font-mono font-medium text-gray-700">
              {formatMb(usedMb)}
            </span>{" "}
            used
          </span>
          <span className="text-gray-500">
            <span className="font-mono font-medium text-gray-700">
              {formatMb(availableMb)}
            </span>{" "}
            available
          </span>
        </div>
      </div>

      {/* Gate status */}
      {gateMb > 0 && (
        <div className="px-4 pb-3">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium ${PROXIMITY_STYLES[proximity]}`}
            >
              <span
                className={`inline-block w-1.5 h-1.5 rounded-full ${
                  proximity === "danger"
                    ? "bg-red-500"
                    : proximity === "warning"
                      ? "bg-amber-500"
                      : "bg-emerald-500"
                }`}
              />
              Gate: {formatMb(gateMb)} threshold
            </span>
            {gateAborts > 0 && (
              <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium bg-red-50 text-red-600">
                {gateAborts} {gateAborts === 1 ? "rejection" : "rejections"}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
