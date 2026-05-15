/**
 * SVG cost matrix grid for DP alignment visualization.
 *
 * Shows the cost matrix as a grid of cells with gradient coloring
 * (green=low cost → red=high cost). Active cell pulses during fill,
 * traceback path is highlighted.
 */

import type { AlignStep } from "../../engines/dpAlignment";

interface CostGridProps {
  /** Number of rows (reference.length + 1). */
  rows: number;
  /** Number of columns (payload.length + 1). */
  cols: number;
  /** Flat cost matrix (row-major). */
  costMatrix: number[];
  /** Reference words (row labels). */
  reference: string[];
  /** Payload words (column labels). */
  payload: string[];
  /** Steps to show up to (inclusive). */
  steps: AlignStep[];
  /** Current step index. */
  currentStep: number;
  /** Number of prefix elements stripped. */
  prefixStripped: number;
  /** Number of suffix elements stripped. */
  suffixStripped: number;
}

const CELL_SIZE = 40;
const LABEL_SIZE = 60;
const HEADER_SIZE = 20;

export function CostGrid({
  rows,
  cols,
  costMatrix,
  reference,
  payload,
  steps,
  currentStep,
  prefixStripped,
  suffixStripped,
}: CostGridProps) {
  const width = LABEL_SIZE + cols * CELL_SIZE + 4;
  const height = HEADER_SIZE + LABEL_SIZE + rows * CELL_SIZE + 4;

  // Find max cost for color scaling
  const maxCost = Math.max(1, ...costMatrix.filter((c) => c > 0));

  // Determine which cells have been filled up to currentStep
  const filledCells = new Set<string>();
  const tracebackCells = new Set<string>();

  for (let s = 0; s <= currentStep && s < steps.length; s++) {
    const step = steps[s];
    if (step.kind === "fill") {
      filledCells.add(`${step.i},${step.j}`);
    } else if (step.kind === "traceback") {
      tracebackCells.add(`${step.i},${step.j}`);
    }
  }

  // Current active cell
  const currentStepData = steps[currentStep];
  const activeKey = currentStepData ? `${currentStepData.i},${currentStepData.j}` : null;

  function cellColor(cost: number, key: string): string {
    if (tracebackCells.has(key)) return "#93c5fd"; // blue-300
    if (!filledCells.has(key) && key !== "0,0") return "#f3f4f6"; // gray-100 (unfilled)
    const ratio = cost / maxCost;
    // green(0) → yellow(0.5) → red(1)
    if (ratio <= 0.5) {
      const r = Math.round(ratio * 2 * 255);
      return `rgb(${r}, 220, 100)`;
    }
    const g = Math.round((1 - (ratio - 0.5) * 2) * 220);
    return `rgb(255, ${g}, 80)`;
  }

  return (
    <div className="overflow-x-auto">
      <svg viewBox={`0 0 ${width} ${height}`} className="max-w-2xl" style={{ height: "auto" }}>
        {/* Column headers (payload words) */}
        {payload.map((word, j) => (
          <text
            key={`col-${j}`}
            x={LABEL_SIZE + (j + 1) * CELL_SIZE + CELL_SIZE / 2}
            y={HEADER_SIZE + LABEL_SIZE - 6}
            textAnchor="middle"
            className="fill-gray-600 text-[9px] font-mono"
          >
            {word.length > 5 ? word.slice(0, 4) + "\u2026" : word}
          </text>
        ))}

        {/* Gap column header */}
        <text
          x={LABEL_SIZE + CELL_SIZE / 2}
          y={HEADER_SIZE + LABEL_SIZE - 6}
          textAnchor="middle"
          className="fill-gray-400 text-[9px] font-mono"
        >
          -
        </text>

        {/* Row labels (reference words) */}
        <text
          x={LABEL_SIZE - 4}
          y={HEADER_SIZE + LABEL_SIZE + CELL_SIZE / 2 + 4}
          textAnchor="end"
          className="fill-gray-400 text-[9px] font-mono"
        >
          -
        </text>
        {reference.map((word, i) => (
          <text
            key={`row-${i}`}
            x={LABEL_SIZE - 4}
            y={HEADER_SIZE + LABEL_SIZE + (i + 1) * CELL_SIZE + CELL_SIZE / 2 + 4}
            textAnchor="end"
            className="fill-gray-600 text-[9px] font-mono"
          >
            {word.length > 6 ? word.slice(0, 5) + "\u2026" : word}
          </text>
        ))}

        {/* Grid cells */}
        {Array.from({ length: rows }, (_, i) =>
          Array.from({ length: cols }, (_, j) => {
            const key = `${i},${j}`;
            const cost = costMatrix[i * cols + j];
            const x = LABEL_SIZE + j * CELL_SIZE;
            const y = HEADER_SIZE + LABEL_SIZE + i * CELL_SIZE;
            const isActive = key === activeKey;
            const isFilled = filledCells.has(key) || (i === 0 && j === 0);
            const isTraceback = tracebackCells.has(key);

            return (
              <g key={key}>
                <rect
                  x={x + 1}
                  y={y + 1}
                  width={CELL_SIZE - 2}
                  height={CELL_SIZE - 2}
                  rx={3}
                  fill={cellColor(cost, key)}
                  stroke={isActive ? "#2563eb" : isTraceback ? "#3b82f6" : "#e5e7eb"}
                  strokeWidth={isActive ? 2.5 : 1}
                  opacity={isFilled || (i === 0 && j === 0) ? 1 : 0.3}
                >
                  {isActive && (
                    <animate
                      attributeName="stroke-width"
                      values="2.5;3.5;2.5"
                      dur="0.8s"
                      repeatCount="indefinite"
                    />
                  )}
                </rect>
                {(isFilled || (i === 0 && j === 0)) && (
                  <text
                    x={x + CELL_SIZE / 2}
                    y={y + CELL_SIZE / 2 + 4}
                    textAnchor="middle"
                    className="fill-gray-800 text-[10px] font-mono font-medium"
                  >
                    {cost}
                  </text>
                )}
              </g>
            );
          }),
        )}

        {/* Prefix/suffix strip indicators */}
        {prefixStripped > 0 && (
          <text x={4} y={HEADER_SIZE + 10} className="fill-emerald-600 text-[8px]">
            {prefixStripped} prefix stripped
          </text>
        )}
        {suffixStripped > 0 && (
          <text x={width - 4} y={HEADER_SIZE + 10} textAnchor="end" className="fill-emerald-600 text-[8px]">
            {suffixStripped} suffix stripped
          </text>
        )}
      </svg>
    </div>
  );
}
