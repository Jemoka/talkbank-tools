/** SSE `EventSource` wrapper for `/jobs/{id}/stream`.
 *
 * Opens a server-sent events connection to the batchalign server and delivers
 * typed progress updates. The stream auto-closes when the job reaches a
 * terminal state (`complete` event) or when the component unmounts.
 */

import { useEffect, useRef, useState } from "react";
import { controlPlaneOrigin } from "../runtime";
import type { FileStatusEntry } from "../types";

export type JobStreamStatus = "connecting" | "streaming" | "complete" | "error";

export interface FileProgress {
  /** File-level status entries, updated live as SSE events arrive. */
  files: Map<string, FileStatusEntry>;
  /** Number of files the server has finished processing. */
  completedFiles: number;
  /** Overall job status from the server. */
  jobStatus: string | null;
}

export interface JobStreamState {
  /** Connection status of the SSE stream. */
  streamStatus: JobStreamStatus;
  /** Live file progress data. */
  progress: FileProgress;
}

/** Subscribe to SSE progress for a specific job. */
export function useJobStream(jobId: string | null): JobStreamState {
  const base = controlPlaneOrigin();
  const [streamStatus, setStreamStatus] = useState<JobStreamStatus>("connecting");
  const [progress, setProgress] = useState<FileProgress>({
    files: new Map(),
    completedFiles: 0,
    jobStatus: null,
  });
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    if (!jobId) return;

    setStreamStatus("connecting");
    const url = `${base}/jobs/${jobId}/stream`;
    const es = new EventSource(url);
    esRef.current = es;

    es.onopen = () => setStreamStatus("streaming");

    es.addEventListener("snapshot", (e: MessageEvent) => {
      const data = JSON.parse(e.data);
      const files = new Map<string, FileStatusEntry>();
      for (const f of data.file_statuses ?? []) {
        files.set(f.filename, f);
      }
      setProgress({
        files,
        completedFiles: data.completed_files ?? 0,
        jobStatus: data.status ?? null,
      });
    });

    es.addEventListener("file_update", (e: MessageEvent) => {
      const data = JSON.parse(e.data);
      setProgress((prev) => {
        const next = new Map(prev.files);
        next.set(data.file.filename, data.file);
        return {
          files: next,
          completedFiles: data.completed_files ?? prev.completedFiles,
          jobStatus: prev.jobStatus,
        };
      });
    });

    es.addEventListener("job_update", (e: MessageEvent) => {
      const data = JSON.parse(e.data);
      setProgress((prev) => ({
        ...prev,
        completedFiles: data.completed_files ?? prev.completedFiles,
        jobStatus: data.status ?? prev.jobStatus,
      }));
    });

    es.addEventListener("complete", (e: MessageEvent) => {
      const data = JSON.parse(e.data);
      setProgress((prev) => ({
        ...prev,
        jobStatus: data.status ?? "completed",
      }));
      setStreamStatus("complete");
      es.close();
    });

    es.onerror = () => {
      setStreamStatus("error");
      es.close();
    };

    return () => {
      es.close();
      esRef.current = null;
    };
  }, [jobId, base]);

  return { streamStatus, progress };
}
