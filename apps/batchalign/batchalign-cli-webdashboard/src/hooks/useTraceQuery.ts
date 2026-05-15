/**
 * React Query hook for fetching algorithm traces from the server.
 *
 * GET /jobs/{jobId}/traces → JobTraces (or 204 No Content if traces not collected).
 */

import { useQuery } from "@tanstack/react-query";

/** Trace types mirroring Rust `types::traces` module. */

export interface RetokenizationTrace {
  utterance_index: number;
  original_words: string[];
  stanza_tokens: string[];
  normalized_original: string;
  normalized_tokens: string;
  mapping: number[][];
  used_fallback: boolean;
}

export interface AlignStepTrace {
  action: string;
  i: number;
  j: number;
}

export interface AlignResultTrace {
  kind: string;
  key: string;
  payload_idx?: number;
  reference_idx?: number;
}

export interface DpAlignmentTrace {
  context: string;
  payload: string[];
  reference: string[];
  match_mode: string;
  prefix_stripped: number;
  suffix_stripped: number;
  cost_matrix: number[];
  traceback: AlignStepTrace[];
  result: AlignResultTrace[];
}

export interface FileTraces {
  filename: string;
  dp_alignments: DpAlignmentTrace[];
  asr_pipeline: unknown | null;
  fa_timeline: unknown | null;
  retokenizations: RetokenizationTrace[];
}

export interface JobTraces {
  files: Record<string, FileTraces>;
}

async function fetchTraces(jobId: string): Promise<JobTraces | null> {
  const res = await fetch(`/jobs/${jobId}/traces`);
  if (res.status === 204) return null;
  if (!res.ok) throw new Error(`Failed to fetch traces: ${res.status}`);
  return (await res.json()) as JobTraces;
}

/**
 * Fetch algorithm traces for a job. Returns null if traces were not collected.
 * Only enabled when jobId is provided.
 */
export function useTraceQuery(jobId: string | undefined) {
  return useQuery({
    queryKey: ["traces", jobId],
    queryFn: () => fetchTraces(jobId!),
    enabled: !!jobId,
    staleTime: 60_000, // traces don't change once job completes
    retry: 1,
  });
}
