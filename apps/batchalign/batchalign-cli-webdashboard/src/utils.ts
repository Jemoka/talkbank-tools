import type { components } from "./generated/api";

type FileProgressStage = components["schemas"]["FileProgressStage"];
type LanguageSpec = components["schemas"]["LanguageSpec"];

/** Formatting helpers. */

/**
 * Extract a display string from a LanguageSpec value.
 *
 * `LanguageSpec` is `"Auto" | { Resolved: string }` on the wire.
 * Returns `"auto"` for auto-detection, or the resolved 3-letter code.
 */
export function displayLang(spec: LanguageSpec | undefined): string {
  if (!spec) return "eng";
  if (spec === "Auto") return "auto";
  if (typeof spec === "object" && "Resolved" in spec) return spec.Resolved;
  return String(spec);
}

/**
 * Whether a LanguageSpec represents the default (eng) — used to hide
 * the language badge when it would just say "eng".
 */
export function isDefaultLang(spec: LanguageSpec | undefined): boolean {
  return displayLang(spec) === "eng";
}

/**
 * Canonical dashboard-side labels for typed file progress stages.
 *
 * The server also derives `progress_label`, but the dashboard prefers the
 * stable stage code when present so UI logic is not coupled to free-form text.
 */
const PROGRESS_STAGE_LABELS: Record<FileProgressStage, string> = {
  processing: "Processing",
  reading: "Reading",
  resolving_audio: "Resolving audio",
  recovering_utterance_timing: "Recovering utterance timing",
  recovering_timing_fallback: "Recovering timing (fallback)",
  aligning: "Aligning",
  transcribing: "Transcribing",
  benchmarking: "Benchmarking",
  checking_cache: "Checking cache",
  applying_results: "Applying results",
  post_processing: "Post-processing",
  building_chat: "Building CHAT",
  segmenting_utterances: "Segmenting utterances",
  analyzing_morphosyntax: "Analyzing morphosyntax",
  finalizing: "Finalizing",
  writing: "Writing",
  parsing: "Parsing",
  analyzing: "Analyzing",
  segmenting: "Segmenting",
  translating: "Translating",
  resolving_coreference: "Resolving coreference",
  comparing: "Comparing",
  retry_scheduled: "Retry scheduled",
};

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || seconds < 0) return "";
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  if (m >= 60) {
    const h = Math.floor(m / 60);
    const rm = m % 60;
    return `${h}h ${rm}m`;
  }
  return `${m}m ${s}s`;
}

export function formatTimestamp(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function relativeTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffS = Math.floor((now - then) / 1000);
  if (diffS < 10) return "just now";
  if (diffS < 60) return `${diffS}s ago`;
  const diffM = Math.floor(diffS / 60);
  if (diffM < 60) return `${diffM}m ago`;
  const diffH = Math.floor(diffM / 60);
  if (diffH < 24) return `${diffH}h ago`;
  const diffD = Math.floor(diffH / 24);
  return `${diffD}d ago`;
}

export function statusColor(status: string): string {
  switch (status) {
    case "queued":
      return "bg-gray-100 text-gray-700";
    case "running":
    case "processing":
      return "bg-blue-100 text-blue-700";
    case "completed":
    case "done":
      return "bg-green-100 text-green-700";
    case "failed":
    case "error":
      return "bg-red-100 text-red-700";
    case "cancelled":
      return "bg-amber-100 text-amber-700";
    case "interrupted":
      return "bg-orange-100 text-orange-700";
    case "writeback_failed":
      return "bg-amber-100 text-amber-700";
    default:
      return "bg-gray-100 text-gray-700";
  }
}

/** Status dot color (Tailwind bg-* class). */
export function statusDotColor(status: string): string {
  switch (status) {
    case "queued":
      return "bg-amber-400";
    case "running":
    case "processing":
      return "bg-blue-500";
    case "completed":
    case "done":
      return "bg-emerald-500";
    case "failed":
    case "error":
      return "bg-red-500";
    case "cancelled":
      return "bg-gray-400";
    case "interrupted":
      return "bg-orange-400";
    case "writeback_failed":
      return "bg-amber-500";
    default:
      return "bg-gray-400";
  }
}

/** Command badge styling: [bgClass, textClass]. */
export function commandStyle(cmd: string): [string, string] {
  switch (cmd) {
    case "align":
      return ["bg-indigo-100", "text-indigo-700"];
    case "morphotag":
      return ["bg-violet-100", "text-violet-700"];
    case "transcribe":
    case "transcribe_s":
      return ["bg-emerald-100", "text-emerald-700"];
    case "benchmark":
      return ["bg-amber-100", "text-amber-700"];
    case "translate":
      return ["bg-teal-100", "text-teal-700"];
    case "opensmile":
      return ["bg-rose-100", "text-rose-700"];
    case "utseg":
      return ["bg-sky-100", "text-sky-700"];
    case "coref":
      return ["bg-orange-100", "text-orange-700"];
    default:
      return ["bg-gray-100", "text-gray-600"];
  }
}

/** Compact display for source_dir: show last 2-3 path components. */
export function shortPath(p: string | null | undefined): string {
  if (!p) return "";
  const parts = p.replace(/\/+$/, "").split("/").filter(Boolean);
  if (parts.length <= 3) return parts.join("/");
  return "\u2026/" + parts.slice(-3).join("/");
}

export function progressPercent(completed: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((completed / total) * 100);
}

/** Friendly display name for the submitter. */
export function submitterName(
  byName: string | null | undefined,
  byIp: string | null | undefined,
): string {
  if (byName && byName !== byIp) return byName;
  if (byIp) return byIp;
  return "";
}

/** Pretty-print JSON-ish API values for read-only dashboard display. */
export function formatJsonDisplay(value: unknown): string {
  if (value == null) return "";
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

/**
 * Resolve the operator-facing progress label for one file.
 *
 * The typed `progress_stage` is preferred when present because it is the
 * stable contract. The older `progress_label` remains as a display fallback.
 */
export function displayProgressLabel(
  stage: FileProgressStage | null | undefined,
  label: string | null | undefined,
): string {
  if (stage) {
    return PROGRESS_STAGE_LABELS[stage] ?? label ?? "";
  }
  return label ?? "";
}
