/** Pipeline stage indicator for active file processing.
 *
 * Groups the 23 `FileProgressStage` variants into 5 visual phases and
 * renders them as a compact segmented indicator. The active phase fills
 * and pulses; completed phases are solid; future phases are gray.
 *
 * Used in `FileTable` rows and `JobDetailPageView` for processing files.
 */

import type { components } from "../generated/api";

type FileProgressStage = components["schemas"]["FileProgressStage"];

// ---------------------------------------------------------------------------
// Phase mapping
// ---------------------------------------------------------------------------

/** Logical pipeline phase grouping related stages. */
interface Phase {
  key: string;
  label: string;
  color: string;
  activeBg: string;
  stages: Set<FileProgressStage>;
}

const PHASES: Phase[] = [
  {
    key: "read",
    label: "Read",
    color: "bg-zinc-400",
    activeBg: "bg-zinc-500",
    stages: new Set<FileProgressStage>([
      "reading",
      "resolving_audio",
      "checking_cache",
    ]),
  },
  {
    key: "transcribe",
    label: "Transcribe",
    color: "bg-emerald-400",
    activeBg: "bg-emerald-500",
    stages: new Set<FileProgressStage>([
      "transcribing",
      "recovering_utterance_timing",
      "recovering_timing_fallback",
    ]),
  },
  {
    key: "align",
    label: "Align",
    color: "bg-indigo-400",
    activeBg: "bg-indigo-500",
    stages: new Set<FileProgressStage>(["aligning", "applying_results"]),
  },
  {
    key: "analyze",
    label: "Analyze",
    color: "bg-violet-400",
    activeBg: "bg-violet-500",
    stages: new Set<FileProgressStage>([
      "analyzing_morphosyntax",
      "segmenting_utterances",
      "translating",
      "resolving_coreference",
      "segmenting",
      "analyzing",
      "comparing",
      "benchmarking",
    ]),
  },
  {
    key: "finalize",
    label: "Finalize",
    color: "bg-amber-400",
    activeBg: "bg-amber-500",
    stages: new Set<FileProgressStage>([
      "post_processing",
      "building_chat",
      "finalizing",
      "writing",
    ]),
  },
];

/** Find which phase index a stage belongs to, or -1 if unknown. */
function phaseIndexForStage(stage: FileProgressStage | null | undefined): number {
  if (!stage) return -1;
  for (let i = 0; i < PHASES.length; i++) {
    if (PHASES[i].stages.has(stage)) return i;
  }
  return -1;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface PipelineStageBarProps {
  /** Current file progress stage (null/undefined = unknown/generic processing). */
  stage: FileProgressStage | null | undefined;
}

/** Compact 5-segment pipeline phase indicator.
 *
 * Each segment is a small pill. The active phase pulses, completed phases
 * are solid, and future phases are gray outlines.
 */
export function PipelineStageBar({ stage }: PipelineStageBarProps) {
  const activeIdx = phaseIndexForStage(stage);

  return (
    <div className="flex items-center gap-0.5" title={stage ?? "processing"}>
      {PHASES.map((phase, i) => {
        const isActive = i === activeIdx;
        const isCompleted = activeIdx >= 0 && i < activeIdx;

        let className =
          "w-4 h-1.5 rounded-full transition-all duration-300 ";
        if (isActive) {
          className += `${phase.activeBg} status-dot-pulse`;
        } else if (isCompleted) {
          className += phase.color;
        } else {
          className += "bg-gray-200";
        }

        return (
          <div
            key={phase.key}
            className={className}
            title={phase.label}
          />
        );
      })}
    </div>
  );
}

/** Labels for the 5 pipeline phases, useful for legends. */
export { PHASES as PIPELINE_PHASES };
