import { useEffect, useState } from "react";
import type { Phase } from "../hooks/useValidation";
import type { ValidationStats } from "../types";

interface Props {
  phase: Phase;
  processedFiles: number;
  totalFiles: number;
  totalErrors: number;
  stats: ValidationStats | null;
  startTime: number | null;
  onRevalidate: () => void;
  onCancel: () => void;
  onExport: () => void;
}

function formatEta(seconds: number): string {
  if (seconds < 60) return `~${Math.ceil(seconds)}s remaining`;
  const m = Math.floor(seconds / 60);
  const s = Math.ceil(seconds % 60);
  return `~${m}m ${s}s remaining`;
}

export default function ProgressBar({
  phase,
  processedFiles,
  totalFiles,
  totalErrors,
  stats,
  startTime,
  onRevalidate,
  onCancel,
  onExport,
}: Props) {
  const pct = totalFiles > 0 ? (processedFiles / totalFiles) * 100 : 0;

  // Update ETA every second during validation
  const [, setTick] = useState(0);
  useEffect(() => {
    if (phase !== "running" || !startTime) return;
    const id = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, [phase, startTime]);

  let etaText: string | null = null;
  if (phase === "running" && startTime && processedFiles >= 5 && processedFiles < totalFiles) {
    const elapsed = (Date.now() - startTime) / 1000;
    const perFile = elapsed / processedFiles;
    const remaining = perFile * (totalFiles - processedFiles);
    etaText = formatEta(remaining);
  }

  return (
    <div className="status-bar">
      {phase === "idle" && <span>Ready</span>}

      {phase === "discovering" && <span>Discovering files...</span>}

      {phase === "running" && (
        <>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <span className="progress-text">
            {processedFiles}/{totalFiles}
          </span>
          {totalErrors > 0 && (
            <span className="error-count-text">{totalErrors} errors</span>
          )}
          {etaText && <span className="eta-text">{etaText}</span>}
        </>
      )}

      {phase === "finished" && stats && (
        <span>
          {stats.totalFiles} files: {stats.validFiles} valid, {stats.invalidFiles} invalid
          {stats.cancelled ? " (cancelled)" : ""}
        </span>
      )}

      <div className="actions">
        {phase === "running" && (
          <button onClick={onCancel}>Cancel</button>
        )}
        {phase === "finished" && (
          <>
            <button className="primary" onClick={onRevalidate}>
              Re-validate
            </button>
            <button onClick={onExport}>Export</button>
          </>
        )}
      </div>
    </div>
  );
}
