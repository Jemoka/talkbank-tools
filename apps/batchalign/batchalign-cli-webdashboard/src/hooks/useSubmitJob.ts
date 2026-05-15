/** React Query mutation for submitting a processing job via `POST /jobs`.
 *
 * Wraps `submitJob()` from the API layer and exposes mutation state so
 * the process form can show loading/error/success feedback.
 */

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { submitJob } from "../api";
import { controlPlaneOrigin } from "../runtime";
import type { JobInfo } from "../types";

export interface SubmitJobParams {
  command: string;
  lang?: string;
  numSpeakers?: number;
  sourcePaths: string[];
  outputPaths: string[];
  sourceDir?: string;
}

/** Build the command-specific `options` payload expected by the server. */
function buildOptions(command: string): Record<string, unknown> {
  // The server requires `options.command` to match the top-level `command` field.
  // All other fields use server defaults when omitted.
  return { command };
}

export function useSubmitJob() {
  const base = controlPlaneOrigin();
  const queryClient = useQueryClient();

  return useMutation<JobInfo, Error, SubmitJobParams>({
    mutationFn: (params) =>
      submitJob(
        {
          command: params.command,
          lang: params.lang ?? "eng",
          num_speakers: params.numSpeakers ?? 1,
          paths_mode: true,
          source_paths: params.sourcePaths,
          output_paths: params.outputPaths,
          source_dir: params.sourceDir,
          options: buildOptions(params.command),
        },
        base,
      ),
    onSuccess: () => {
      // Invalidate the jobs list so the dashboard picks up the new job.
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
  });
}
