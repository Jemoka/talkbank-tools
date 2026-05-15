/**
 * DP Alignment Explorer visualization.
 *
 * Static mode: two editable sequence inputs → step-through cost matrix fill → traceback → result.
 * Live mode: shows actual DP alignment traces from a job.
 */

import { useState, useMemo, useCallback } from "react";
import { useParams } from "wouter";
import { ModeToggle } from "../../components/visualizations/ModeToggle";
import { StepControls } from "../../components/visualizations/StepControls";
import { CostGrid } from "../../components/visualizations/CostGrid";
import {
  alignWithSteps,
  SAMPLE_PAYLOAD,
  SAMPLE_REFERENCE,
  type MatchMode,
  type AlignResultItem,
  type DpAlignmentResult,
  type AlignStep,
} from "../../engines/dpAlignment";
import {
  useTraceQuery,
  type DpAlignmentTrace,
  type JobTraces,
} from "../../hooks/useTraceQuery";

/**
 * Convert a server-side DpAlignmentTrace (pre-computed matrix + traceback)
 * into the step-by-step format used by our visualization components.
 */
function traceToResult(trace: DpAlignmentTrace): DpAlignmentResult {
  const payLen = trace.payload.length;
  const refLen = trace.reference.length;
  const rows = refLen + 1;
  const cols = payLen + 1;

  // Reconstruct fill steps from the cost matrix.
  // The fill order is row-major: (0,0), (0,1), ..., (0,cols-1), (1,0), ...
  const steps: AlignStep[] = [];
  for (let i = 0; i < rows; i++) {
    for (let j = 0; j < cols; j++) {
      const cost = trace.cost_matrix[i * cols + j];
      let action: AlignStep["action"] = "start";
      if (i === 0 && j === 0) action = "start";
      else if (i === 0) action = "extra_payload";
      else if (j === 0) action = "extra_reference";
      else {
        // Determine which action was taken by examining neighbors
        const matchCost = trace.cost_matrix[(i - 1) * cols + (j - 1)];
        const payA = trace.payload[j - 1];
        const refA = trace.reference[i - 1];
        const isMatch =
          trace.match_mode === "case_insensitive"
            ? payA.toLowerCase() === refA.toLowerCase()
            : payA === refA;

        if (isMatch && cost === matchCost) {
          action = "match";
        } else {
          const subCost = matchCost + 2;
          const gapRefCost = trace.cost_matrix[(i - 1) * cols + j] + 1;
          const gapPayCost = trace.cost_matrix[i * cols + (j - 1)] + 1;
          if (cost === subCost && cost <= gapRefCost && cost <= gapPayCost)
            action = "substitution";
          else if (cost === gapRefCost && gapRefCost <= gapPayCost)
            action = "extra_reference";
          else action = "extra_payload";
        }
      }

      steps.push({ kind: "fill", i, j, cost, action });
    }
  }

  // Add traceback steps from server data
  for (const tb of trace.traceback) {
    steps.push({
      kind: "traceback",
      i: tb.i,
      j: tb.j,
      action: tb.action as AlignStep["action"],
    });
  }

  // Convert result items
  const result: AlignResultItem[] = trace.result.map((r) => ({
    kind: r.kind as AlignResultItem["kind"],
    key: r.key,
    payloadIdx: r.payload_idx,
    referenceIdx: r.reference_idx,
  }));

  return {
    payload: trace.payload,
    reference: trace.reference,
    prefixStripped: trace.prefix_stripped,
    suffixStripped: trace.suffix_stripped,
    costMatrix: trace.cost_matrix,
    steps,
    result,
    rows,
    cols,
  };
}

/** Collect all DP alignment traces from a JobTraces, with file info. */
function collectAlignments(
  traces: JobTraces,
): { fileIndex: number; filename: string; alignmentIndex: number; trace: DpAlignmentTrace }[] {
  const items: {
    fileIndex: number;
    filename: string;
    alignmentIndex: number;
    trace: DpAlignmentTrace;
  }[] = [];
  for (const [fileIdx, fileTraces] of Object.entries(traces.files)) {
    for (let ai = 0; ai < fileTraces.dp_alignments.length; ai++) {
      items.push({
        fileIndex: Number(fileIdx),
        filename: fileTraces.filename,
        alignmentIndex: ai,
        trace: fileTraces.dp_alignments[ai],
      });
    }
  }
  return items;
}

function AlignmentResult({ items }: { items: AlignResultItem[] }) {
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item, i) => {
        let bgClass: string;
        let label: string;
        switch (item.kind) {
          case "match":
            bgClass = "bg-emerald-100 text-emerald-800 border-emerald-300";
            label = `${item.key} [${item.payloadIdx}\u2194${item.referenceIdx}]`;
            break;
          case "extra_payload":
            bgClass = "bg-orange-100 text-orange-800 border-orange-300";
            label = `${item.key} [p:${item.payloadIdx}]`;
            break;
          case "extra_reference":
            bgClass = "bg-blue-100 text-blue-800 border-blue-300";
            label = `${item.key} [r:${item.referenceIdx}]`;
            break;
        }
        return (
          <span
            key={i}
            className={`px-2 py-0.5 rounded text-xs font-mono border ${bgClass}`}
          >
            {label}
          </span>
        );
      })}
    </div>
  );
}

export function DPAlignmentPage() {
  const params = useParams<{ id?: string }>();
  const jobId = params.id;
  const isLive = !!jobId;

  // Static mode state
  const [payloadInput, setPayloadInput] = useState(SAMPLE_PAYLOAD.join(" "));
  const [referenceInput, setReferenceInput] = useState(
    SAMPLE_REFERENCE.join(" "),
  );
  const [matchMode, setMatchMode] = useState<MatchMode>("exact");

  // Step control
  const [currentStep, setCurrentStep] = useState(0);

  // Live mode
  const { data: traces } = useTraceQuery(jobId);
  const [selectedIdx, setSelectedIdx] = useState(0);

  const liveAlignments = useMemo(
    () => (traces ? collectAlignments(traces) : []),
    [traces],
  );

  const dpResult = useMemo(() => {
    if (isLive) {
      if (liveAlignments.length === 0) return null;
      const idx = Math.min(selectedIdx, liveAlignments.length - 1);
      return traceToResult(liveAlignments[idx].trace);
    }

    const payload = payloadInput
      .trim()
      .split(/\s+/)
      .filter((w) => w.length > 0);
    const reference = referenceInput
      .trim()
      .split(/\s+/)
      .filter((w) => w.length > 0);
    if (payload.length === 0 && reference.length === 0) return null;

    return alignWithSteps(payload, reference, matchMode);
  }, [isLive, liveAlignments, selectedIdx, payloadInput, referenceInput, matchMode]);

  // Reset step when inputs change
  const handleInputChange = useCallback(
    (setter: (v: string) => void) =>
      (e: React.ChangeEvent<HTMLInputElement>) => {
        setter(e.target.value);
        setCurrentStep(0);
      },
    [],
  );

  const totalSteps = dpResult?.steps.length ?? 0;

  return (
    <div className="min-h-screen">
      <header className="bg-[var(--bg-header)] text-white">
        <div className="max-w-5xl mx-auto px-4 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <a
              href="/dashboard/visualizations"
              className="text-white/50 hover:text-white/80 text-xs no-underline"
            >
              Visualizations
            </a>
            <span className="text-white/30">/</span>
            <span className="font-mono text-sm font-semibold text-white/90">
              DP Alignment Explorer
            </span>
          </div>
          <ModeToggle mode={isLive ? "live" : "static"} />
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-4 py-6">
        {/* Static mode inputs */}
        {!isLive && (
          <div className="mb-6 space-y-3">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="text-xs font-medium text-gray-600 block mb-1">
                  Payload (space-separated)
                </label>
                <input
                  type="text"
                  value={payloadInput}
                  onChange={handleInputChange(setPayloadInput)}
                  className="w-full px-3 py-2 rounded border border-gray-200 text-sm font-mono focus:border-orange-400 focus:outline-none"
                  placeholder="the cat sat"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-gray-600 block mb-1">
                  Reference (space-separated)
                </label>
                <input
                  type="text"
                  value={referenceInput}
                  onChange={handleInputChange(setReferenceInput)}
                  className="w-full px-3 py-2 rounded border border-gray-200 text-sm font-mono focus:border-blue-400 focus:outline-none"
                  placeholder="the big cat sat"
                />
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-gray-500">Match mode:</span>
              <button
                className={`px-2 py-0.5 rounded text-xs ${
                  matchMode === "exact"
                    ? "bg-gray-800 text-white"
                    : "bg-gray-100 text-gray-600 hover:bg-gray-200"
                }`}
                onClick={() => {
                  setMatchMode("exact");
                  setCurrentStep(0);
                }}
              >
                Exact
              </button>
              <button
                className={`px-2 py-0.5 rounded text-xs ${
                  matchMode === "case_insensitive"
                    ? "bg-gray-800 text-white"
                    : "bg-gray-100 text-gray-600 hover:bg-gray-200"
                }`}
                onClick={() => {
                  setMatchMode("case_insensitive");
                  setCurrentStep(0);
                }}
              >
                Case Insensitive
              </button>
            </div>
          </div>
        )}

        {/* Live mode: alignment picker */}
        {isLive && liveAlignments.length > 0 && (
          <div className="mb-6">
            <label className="text-xs font-medium text-gray-600 block mb-1">
              Alignment ({liveAlignments.length} total)
            </label>
            <select
              value={selectedIdx}
              onChange={(e) => {
                setSelectedIdx(Number(e.target.value));
                setCurrentStep(0);
              }}
              className="px-3 py-2 rounded border border-gray-200 text-sm font-mono focus:border-blue-400 focus:outline-none"
            >
              {liveAlignments.map((item, idx) => (
                <option key={idx} value={idx}>
                  {item.filename} &mdash; {item.trace.context} (
                  {item.trace.payload.length}&times;
                  {item.trace.reference.length})
                </option>
              ))}
            </select>
          </div>
        )}

        {isLive && !traces && (
          <div className="text-sm text-gray-500 py-8 text-center">
            No traces available. The job may not have been submitted with
            debug_traces enabled.
          </div>
        )}

        {isLive && traces && liveAlignments.length === 0 && (
          <div className="text-sm text-gray-500 py-8 text-center">
            This job has traces, but no DP alignments were recorded.
          </div>
        )}

        {dpResult && (
          <div className="space-y-6">
            {/* Step controls */}
            <StepControls
              total={totalSteps}
              current={currentStep}
              onStep={setCurrentStep}
            />

            {/* Cost matrix */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Cost Matrix
              </h2>
              <CostGrid
                rows={dpResult.rows}
                cols={dpResult.cols}
                costMatrix={dpResult.costMatrix}
                reference={dpResult.reference.slice(
                  dpResult.prefixStripped,
                  dpResult.reference.length - dpResult.suffixStripped,
                )}
                payload={dpResult.payload.slice(
                  dpResult.prefixStripped,
                  dpResult.payload.length - dpResult.suffixStripped,
                )}
                steps={dpResult.steps}
                currentStep={currentStep}
                prefixStripped={dpResult.prefixStripped}
                suffixStripped={dpResult.suffixStripped}
              />
            </section>

            {/* Cost info */}
            <div className="text-xs text-gray-500">
              Cost model: match=0, substitution=2, gap=1. Matrix size:{" "}
              {dpResult.rows}&times;{dpResult.cols}. Final cost:{" "}
              {
                dpResult.costMatrix[
                  (dpResult.rows - 1) * dpResult.cols + dpResult.cols - 1
                ]
              }
              .
              {isLive && liveAlignments.length > 0 && (
                <>
                  {" "}
                  Context:{" "}
                  <span className="font-mono">
                    {liveAlignments[Math.min(selectedIdx, liveAlignments.length - 1)].trace.context}
                  </span>
                  . Match mode:{" "}
                  <span className="font-mono">
                    {liveAlignments[Math.min(selectedIdx, liveAlignments.length - 1)].trace.match_mode}
                  </span>
                  .
                </>
              )}
            </div>

            {/* Alignment result */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Alignment Result
              </h2>
              <AlignmentResult items={dpResult.result} />
              <div className="flex gap-4 mt-2 text-[10px] text-gray-400">
                <span>
                  <span className="inline-block w-3 h-2 bg-emerald-100 border border-emerald-300 rounded mr-1 align-middle" />
                  Match
                </span>
                <span>
                  <span className="inline-block w-3 h-2 bg-orange-100 border border-orange-300 rounded mr-1 align-middle" />
                  Extra payload
                </span>
                <span>
                  <span className="inline-block w-3 h-2 bg-blue-100 border border-blue-300 rounded mr-1 align-middle" />
                  Extra reference
                </span>
              </div>
            </section>
          </div>
        )}
      </main>
    </div>
  );
}
