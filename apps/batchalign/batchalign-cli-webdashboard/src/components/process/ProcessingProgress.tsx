/** SSE-driven live progress view for an active job.
 *
 * Shows a progress bar, file count, elapsed time, and a live-updating file
 * list. Addresses the "frozen progress bar" problem with three strategies:
 *
 * 1. **Elapsed timer** — always ticking, proves the app is alive
 * 2. **Indeterminate bar** — for batched commands (morphotag, utseg, etc.)
 *    where no per-file progress is available during the batch_infer call
 * 3. **Sub-file progress** — for per-file commands (align, transcribe),
 *    shows progress_current/progress_total inline (e.g., "Group 3/12")
 * 4. **Contextual messages** — explain what's happening during long waits
 *
 * When the job completes, the file capability can reveal the output folder in
 * Finder/Explorer without this component importing raw Tauri APIs.
 */

import { useEffect, useState } from "react";
import { useJobStream } from "../../hooks/useJobStream";
import { useDesktopFiles } from "../../desktop/DesktopContext";
import { cancelJob } from "../../api";
import { controlPlaneOrigin } from "../../runtime";
import { ErrorRecovery } from "./ErrorRecovery";
import {
  formatDuration,
  statusDotColor,
  displayProgressLabel,
  progressPercent,
} from "../../utils";
import type { FileStatusEntry } from "../../types";

/** Server command IDs that use a single batch_infer call (no per-file progress
 * during processing). The prop `command` must be the server command ID (e.g.
 * "morphotag"), not a user-facing label, so this set is stable across label
 * copy changes.
 */
const BATCHED_COMMANDS = new Set([
  "morphotag",
  "utseg",
  "translate",
  "coref",
]);

/** Sort priority: processing first, then queued, completed, error. */
function fileSortKey(f: FileStatusEntry): number {
  switch (f.status) {
    case "processing":
      return 0;
    case "queued":
      return 1;
    case "done":
      return 2;
    case "error":
      return 3;
    default:
      return 4;
  }
}

/** Contextual messages shown during long waits for batched commands. */
const BATCH_WAIT_MESSAGES = [
  "All utterances are being processed together for best accuracy...",
  "The model is analyzing your entire corpus as one batch...",
  "This can take a few minutes for large collections of files...",
  "Batch processing is more accurate than file-by-file — hang tight...",
];

/** Contextual messages for per-file commands during processing. */
const PER_FILE_MESSAGES = [
  "Each file is processed individually...",
  "Files are processed in parallel when resources allow...",
];

/** Hook that returns elapsed seconds since mount, updating every second. */
function useElapsedTime(active: boolean): number {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (!active) return;
    setElapsed(0);
    const interval = setInterval(() => {
      setElapsed((prev) => prev + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, [active]);

  return elapsed;
}

/** Cycle through messages every N seconds. */
function useRotatingMessage(messages: string[], intervalSec: number, active: boolean): string {
  const [index, setIndex] = useState(0);

  useEffect(() => {
    if (!active) return;
    const interval = setInterval(() => {
      setIndex((prev) => (prev + 1) % messages.length);
    }, intervalSec * 1000);
    return () => clearInterval(interval);
  }, [messages, intervalSec, active]);

  return messages[index] ?? "";
}

/** Stage-specific explanatory hints for long-running stages.
 *
 * These appear as a subtle note below a file row to set expectations
 * and explain *why* a stage takes a long time, rather than leaving
 * the user staring at a frozen label.
 */
function stageHint(stage: string | null | undefined): string | null {
  switch (stage) {
    case "reading":
      return "Loading file contents from disk";
    case "writing":
      return "Saving processed results to disk";
    case "recovering_utterance_timing":
      return "Re-transcribing the audio to recover word timing — takes roughly as long as the recording itself";
    case "recovering_timing_fallback":
      return "Timing recovery failed on some segments, retrying with full-file transcription";
    case "transcribing":
      return "Transcribing audio — Rev.AI runs roughly in real-time, Whisper may take 2-5x the audio length";
    case "aligning":
      return "Running forced alignment on each utterance group";
    case "resolving_audio":
      return "Locating and preparing the audio file for processing";
    case "analyzing_morphosyntax":
      return "Running part-of-speech tagging and grammatical analysis — all files are batched together for GPU efficiency";
    default:
      return null;
  }
}

/** Format sub-file progress like "3/12" when available. */
function subProgress(f: FileStatusEntry): string | null {
  if (
    f.progress_current != null &&
    f.progress_total != null &&
    f.progress_total > 0
  ) {
    return `${f.progress_current}/${f.progress_total}`;
  }
  return null;
}

interface ProcessingProgressProps {
  jobId: string;
  totalFiles: number;
  command: string;
  /** Folder path to open when processing completes. */
  outputFolder: string | null;
  /** Return to home screen. */
  onReset: () => void;
}

export function ProcessingProgress({
  jobId,
  totalFiles,
  command,
  outputFolder,
  onReset,
}: ProcessingProgressProps) {
  const desktopFiles = useDesktopFiles();
  const { streamStatus, progress } = useJobStream(jobId);
  const { files: fileStatuses, completedFiles, jobStatus } = progress;
  const [cancelling, setCancelling] = useState(false);

  const isTerminal =
    jobStatus === "completed" ||
    jobStatus === "failed" ||
    jobStatus === "cancelled";
  const isSuccess = jobStatus === "completed";
  const isRunning = !isTerminal && streamStatus !== "connecting";

  const isBatched = BATCHED_COMMANDS.has(command);
  const noFilesCompleted = completedFiles === 0;

  // Indeterminate mode: batched command with nothing completed yet
  const showIndeterminate = isBatched && noFilesCompleted && isRunning;

  const percent = progressPercent(completedFiles, totalFiles);
  const elapsed = useElapsedTime(isRunning);
  const contextMessage = useRotatingMessage(
    isBatched ? BATCH_WAIT_MESSAGES : PER_FILE_MESSAGES,
    8,
    isRunning && noFilesCompleted,
  );

  async function handleCancel() {
    setCancelling(true);
    try {
      await cancelJob(jobId, controlPlaneOrigin());
    } catch {
      // SSE stream will reflect the actual state regardless
    }
    setCancelling(false);
  }

  const sortedFiles = [...fileStatuses.values()].sort(
    (a, b) => fileSortKey(a) - fileSortKey(b),
  );
  const errorFiles = sortedFiles.filter((f) => f.status === "error");

  return (
    <div className="space-y-4">
      {/* Summary bar — announced to screen readers on progress changes */}
      <div className="flex items-center justify-between" role="status" aria-live="polite" aria-atomic="true">
        <div className="flex items-center gap-3">
          <span className="inline-block text-xs font-medium px-2 py-0.5 rounded bg-gray-100 text-gray-600">
            {command}
          </span>
          <span className="text-lg font-semibold text-gray-800">
            {completedFiles} of {totalFiles} files
          </span>
          {/* Elapsed timer — ticking while running, frozen at completion */}
          {(isRunning || (isTerminal && elapsed > 0)) && (
            <span className="text-xs text-gray-400 tabular-nums">
              {formatDuration(elapsed)}
            </span>
          )}
          {/* ETA based on throughput */}
          {isRunning && completedFiles > 0 && completedFiles < totalFiles && elapsed > 0 && (
            <span className="text-xs text-gray-400 tabular-nums">
              ~{formatDuration(Math.round(((totalFiles - completedFiles) / completedFiles) * elapsed))} remaining
            </span>
          )}
        </div>
        {!isTerminal && (
          <div className="flex items-center gap-3">
            <span className="text-xs text-gray-400">
              {streamStatus === "connecting" ? "Connecting..." : "Processing"}
            </span>
            <button
              type="button"
              onClick={handleCancel}
              disabled={cancelling}
              className="text-xs text-red-500 hover:text-red-700 transition-colors disabled:opacity-50"
            >
              {cancelling ? "Cancelling..." : "Cancel"}
            </button>
          </div>
        )}
      </div>

      {/* Progress bar */}
      <div className="w-full h-3 bg-gray-200 rounded-full overflow-hidden relative">
        {showIndeterminate ? (
          // Indeterminate shimmer for batched commands with no progress yet
          <div className="h-full rounded-full bg-blue-500 progress-indeterminate" />
        ) : (
          <div
            className={`h-full rounded-full transition-all duration-300 ${
              isSuccess
                ? "bg-emerald-500"
                : jobStatus === "failed"
                  ? "bg-red-500"
                  : jobStatus === "cancelled"
                    ? "bg-amber-500"
                    : "bg-blue-500 progress-striped"
            }`}
            style={{ width: `${Math.max(percent, isTerminal ? 100 : 2)}%` }}
          />
        )}
      </div>

      {/* Contextual message during long waits */}
      {isRunning && noFilesCompleted && (
        <p className="text-xs text-gray-400 italic transition-opacity duration-500">
          {contextMessage}
        </p>
      )}

      {/* Completion actions */}
      {isTerminal && (
        <div className="flex gap-3">
          {isSuccess && outputFolder && (
            <button
              type="button"
              onClick={() => {
                void desktopFiles.openPath(outputFolder);
              }}
              className="px-4 py-2 text-sm font-medium bg-emerald-600 text-white rounded-lg
                hover:bg-emerald-700 transition-colors"
            >
              Open Output Folder
            </button>
          )}
          <button
            type="button"
            onClick={onReset}
            className="px-4 py-2 text-sm font-medium bg-gray-100 text-gray-700 rounded-lg
              hover:bg-gray-200 transition-colors"
          >
            Process More Files
          </button>
        </div>
      )}

      {/* Error recovery (shown when job has failed files) */}
      {isTerminal && errorFiles.length > 0 && (
        <ErrorRecovery errorFiles={errorFiles} />
      )}

      {/* File list */}
      <div className="border border-gray-200 rounded-lg overflow-hidden">
        <div className="max-h-80 overflow-y-auto divide-y divide-gray-100">
          {sortedFiles.map((f) => {
            const sub = subProgress(f);
            const hint =
              f.status === "processing"
                ? stageHint(f.progress_stage)
                : null;
            return (
              <div key={f.filename} className="px-3 py-2">
                <div className="flex items-center gap-3 text-sm">
                  <span
                    className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${statusDotColor(f.status)} ${
                      f.status === "processing" ? "status-dot-pulse" : ""
                    }`}
                  />
                  <span className="flex-1 truncate text-gray-700 font-mono text-xs">
                    {f.filename}
                  </span>
                  {/* Sub-file progress counter (e.g., "3/12 groups" for align) */}
                  {f.status === "processing" && sub && (
                    <span className="text-xs text-blue-500 font-medium tabular-nums flex-shrink-0">
                      {sub}
                    </span>
                  )}
                  <span className="text-xs text-gray-400 flex-shrink-0">
                    {displayProgressLabel(f.progress_stage, f.progress_label)}
                  </span>
                  {f.finished_at && f.started_at && (
                    <span className="text-xs text-gray-400 flex-shrink-0 w-12 text-right tabular-nums">
                      {formatDuration(f.finished_at - f.started_at)}
                    </span>
                  )}
                </div>
                {/* Stage-specific hint explaining why this step is slow */}
                {hint && (
                  <div className="ml-5 mt-1 text-xs text-gray-400 italic">
                    {hint}
                  </div>
                )}
              </div>
            );
          })}
          {sortedFiles.length === 0 && (
            <div className="px-3 py-4 text-sm text-gray-400 text-center">
              Waiting for progress updates...
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
