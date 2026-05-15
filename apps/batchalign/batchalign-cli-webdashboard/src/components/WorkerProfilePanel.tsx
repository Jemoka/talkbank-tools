/** Workers & Memory panel — profile-aware worker status display.
 *
 * Shows active worker profiles, model sharing status, and warmup progress.
 * The key insight this panel communicates: GPU profile workers share loaded
 * models (ASR + FA + Speaker) in one process via ThreadPoolExecutor, saving
 * ~3 GB compared to loading each model in a separate process.
 *
 * Data source: the `live_worker_keys` and `warmup_status` fields from the
 * `/health` endpoint, stored in the Zustand `healthMap`.
 */

import type { HealthResponse } from "../types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Parsed representation of one worker key string from the health endpoint. */
interface ParsedWorkerKey {
  /** Profile name: "gpu", "stanza", or "io". */
  profile: "gpu" | "stanza" | "io";
  /** Language code (e.g., "eng", "fra"). */
  lang: string;
  /** Engine overrides suffix, if any. */
  engineOverrides: string;
  /** Total worker count for this key. */
  total: number;
  /** Idle count, or "shared" for GPU concurrent workers. */
  idle: number | "shared";
}

/** Aggregated profile summary for display. */
interface ProfileSummary {
  profile: "gpu" | "stanza" | "io";
  label: string;
  description: string;
  /** Commands served by this profile. */
  commands: string[];
  /** Parsed worker keys belonging to this profile. */
  workers: ParsedWorkerKey[];
  /** Total worker processes across all keys. */
  totalWorkers: number;
  /** Whether this profile uses concurrent model sharing. */
  isShared: boolean;
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

const PROFILE_META: Record<
  string,
  { label: string; description: string; commands: string[] }
> = {
  gpu: {
    label: "GPU",
    description: "Shared ASR + FA + Speaker models",
    commands: ["align", "transcribe", "transcribe_s", "benchmark"],
  },
  stanza: {
    label: "Stanza",
    description: "NLP processors (POS, parse, coref)",
    commands: ["morphotag", "utseg", "coref", "compare"],
  },
  io: {
    label: "IO",
    description: "Translation & audio analysis",
    commands: ["translate", "opensmile", "avqi"],
  },
};

/** Parse a worker key string like "profile:gpu:eng:{"fa":"wave2vec"} (1 total, shared)". */
function parseWorkerKey(raw: string): ParsedWorkerKey | null {
  // Format: "profile:<name>:<lang>[:<overrides>] (<N> total, <M> idle|shared)"
  const match = raw.match(
    /^profile:(gpu|stanza|io):(\w+)(:[^\s(]+)?\s*\((\d+)\s+total,\s+(shared|\d+\s+idle)\)/
  );
  if (!match) return null;

  const [, profile, lang, overridesPart, totalStr, idlePart] = match;
  return {
    profile: profile as "gpu" | "stanza" | "io",
    lang,
    engineOverrides: overridesPart?.slice(1) ?? "",
    total: parseInt(totalStr, 10),
    idle: idlePart === "shared" ? "shared" : parseInt(idlePart, 10),
  };
}

/** Aggregate parsed worker keys into profile summaries. */
function buildProfileSummaries(
  workerKeys: string[]
): Map<string, ProfileSummary> {
  const map = new Map<string, ProfileSummary>();

  // Initialize all profiles so they appear even when idle.
  for (const [key, meta] of Object.entries(PROFILE_META)) {
    map.set(key, {
      profile: key as "gpu" | "stanza" | "io",
      label: meta.label,
      description: meta.description,
      commands: meta.commands,
      workers: [],
      totalWorkers: 0,
      isShared: key === "gpu",
    });
  }

  for (const raw of workerKeys) {
    const parsed = parseWorkerKey(raw);
    if (!parsed) continue;
    const summary = map.get(parsed.profile);
    if (summary) {
      summary.workers.push(parsed);
      summary.totalWorkers += parsed.total;
    }
  }

  return map;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

const PROFILE_COLORS: Record<string, { dot: string; ring: string; bg: string }> = {
  gpu: {
    dot: "bg-amber-500",
    ring: "ring-amber-500/20",
    bg: "bg-amber-50",
  },
  stanza: {
    dot: "bg-indigo-500",
    ring: "ring-indigo-500/20",
    bg: "bg-indigo-50",
  },
  io: {
    dot: "bg-emerald-500",
    ring: "ring-emerald-500/20",
    bg: "bg-emerald-50",
  },
};

/** One profile row in the panel. */
function ProfileRow({ summary }: { summary: ProfileSummary }) {
  const colors = PROFILE_COLORS[summary.profile] ?? PROFILE_COLORS.io;
  const isActive = summary.totalWorkers > 0;

  return (
    <div
      className={`flex items-start gap-3 rounded-lg px-3 py-2.5 transition-colors ${
        isActive ? colors.bg : "bg-gray-50"
      }`}
    >
      {/* Status dot */}
      <div className="pt-0.5">
        <span
          className={`inline-block w-2 h-2 rounded-full ${
            isActive ? `${colors.dot} status-dot-pulse` : "bg-gray-300"
          }`}
        />
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-gray-800">
            {summary.label}
          </span>
          {isActive && (
            <span className="text-xs text-gray-500 font-mono">
              {summary.totalWorkers}{" "}
              {summary.totalWorkers === 1 ? "process" : "processes"}
            </span>
          )}
          {!isActive && (
            <span className="text-xs text-gray-400">idle</span>
          )}
        </div>

        <p className="text-xs text-gray-500 mt-0.5">{summary.description}</p>

        {/* Model sharing callout for GPU */}
        {summary.isShared && isActive && (
          <div className="flex items-center gap-1.5 mt-1.5">
            <svg
              className="w-3.5 h-3.5 text-amber-600 flex-shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth={2}
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M13.19 8.688a4.5 4.5 0 0 1 1.242 7.244l-4.5 4.5a4.5 4.5 0 0 1-6.364-6.364l1.757-1.757m13.35-.622 1.757-1.757a4.5 4.5 0 0 0-6.364-6.364l-4.5 4.5a4.5 4.5 0 0 0 1.242 7.244"
              />
            </svg>
            <span className="text-xs text-amber-700 font-medium">
              Models shared — align + transcribe reuse one process
            </span>
          </div>
        )}

        {/* Worker details */}
        {summary.workers.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-2">
            {summary.workers.map((w, i) => (
              <span
                key={i}
                className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono ${
                  w.idle === "shared"
                    ? "bg-amber-100 text-amber-800"
                    : "bg-gray-100 text-gray-600"
                }`}
              >
                {w.lang}
                {w.engineOverrides && (
                  <span className="text-gray-400 truncate max-w-[8rem]">
                    {w.engineOverrides}
                  </span>
                )}
                {w.idle === "shared" ? (
                  <span className="text-amber-600">shared</span>
                ) : (
                  <span>
                    {w.idle}/{w.total} idle
                  </span>
                )}
              </span>
            ))}
          </div>
        )}

        {/* Commands served */}
        <div className="flex flex-wrap gap-1 mt-1.5">
          {summary.commands.map((cmd) => (
            <span
              key={cmd}
              className="text-[10px] text-gray-400 bg-gray-100 rounded px-1.5 py-px"
            >
              {cmd}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

/** Warmup progress indicator. */
function WarmupStatus({
  status,
}: {
  status: string | undefined;
}) {
  if (!status || status === "complete") return null;

  const isInProgress = status === "in_progress";
  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-blue-50 rounded-lg">
      {isInProgress && (
        <div className="w-3 h-3 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
      )}
      <span className="text-xs text-blue-700 font-medium">
        {isInProgress
          ? "Warming up models..."
          : "Warmup not started"}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

interface WorkerProfilePanelProps {
  /** Health response for the current server. */
  health: HealthResponse | undefined;
}

/** Workers & Memory panel displaying profile-aware worker status.
 *
 * Designed to sit alongside the server status bar in the dashboard or
 * process page. Communicates three things at a glance:
 *
 * 1. Which worker profiles are active and how many processes each uses.
 * 2. That GPU models are shared (the key memory optimization).
 * 3. Whether warmup is still in progress.
 */
export function WorkerProfilePanel({ health }: WorkerProfilePanelProps) {
  if (!health) return null;

  const workerKeys = health.live_worker_keys ?? [];
  const profiles = buildProfileSummaries(workerKeys);
  const totalWorkers = health.live_workers ?? 0;

  // Sort: active profiles first, then by profile order (gpu, stanza, io).
  const order = ["gpu", "stanza", "io"];
  const sorted = [...profiles.values()].sort((a, b) => {
    const aActive = a.totalWorkers > 0 ? 0 : 1;
    const bActive = b.totalWorkers > 0 ? 0 : 1;
    if (aActive !== bActive) return aActive - bActive;
    return order.indexOf(a.profile) - order.indexOf(b.profile);
  });

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
              d="M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 0 0 2.25-2.25V6.75a2.25 2.25 0 0 0-2.25-2.25H6.75A2.25 2.25 0 0 0 4.5 6.75v10.5a2.25 2.25 0 0 0 2.25 2.25Z"
            />
          </svg>
          <span className="text-sm font-semibold text-gray-700">
            Workers
          </span>
        </div>
        <span className="text-xs text-gray-400 font-mono">
          {totalWorkers} {totalWorkers === 1 ? "process" : "processes"}
        </span>
      </div>

      {/* Profile rows */}
      <div className="p-2 space-y-1.5">
        <WarmupStatus status={health.warmup_status} />
        {sorted.map((summary) => (
          <ProfileRow key={summary.profile} summary={summary} />
        ))}
      </div>
    </div>
  );
}
