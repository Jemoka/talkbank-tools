// Encapsulates the "submit the active batch's first verb as a recipe
// and kick off the SSE pump" flow so the start button can live in the
// batch action footer (per the design canvas — variant-a.jsx places it
// in the left pane's sticky footer, NOT inside the files block).

import { invoke } from "@tauri-apps/api/core";
import { submitRecipe } from "../api";
import { useStore, type VerbConfig, type VerbStep } from "../store";

function buildRecipeKwargs(
  verb: VerbStep,
  config: VerbConfig,
): Record<string, unknown> {
  switch (verb) {
    case "transcribe": {
      const engine = (config.engine as string) || "WhisperXBackend";
      const lang = (config.lang as string) || "eng";
      const speakers = (config.speakers as number) ?? 2;
      const diarize = (config.diarize as boolean) ?? true;
      const out: Record<string, unknown> = {
        asr_backend: { kind: engine, kwargs: { lang } },
      };
      if (diarize) {
        out.speaker_backend = {
          kind: "PyannoteBackend",
          kwargs: { num_speakers: speakers },
        };
      }
      return out;
    }
    case "align":
      return {
        fa_backend: {
          kind: (config.engine as string) || "WhisperXFaBackend",
          kwargs: {},
        },
      };
    case "morphotag":
      return {
        stanza_backend: {
          kind: "StanzaBackend",
          kwargs: { lang: (config.lang as string) || "eng" },
        },
      };
    case "translate":
      return {
        translate_backend: {
          kind: (config.engine as string) || "GoogleTranslateBackend",
          kwargs: { target: (config.target as string) || "eng" },
        },
      };
    case "compare":
      return {
        stanza_backend: {
          kind: "StanzaBackend",
          kwargs: { lang: (config.lang as string) || "eng" },
        },
      };
  }
}

export interface UseStartBatchResult {
  /** True iff the current batch has at least one file and one verb. */
  canStart: boolean;
  /** True while the batch is mid-run; the button should reflect this. */
  isRunning: boolean;
  /** Fire the recipe submission + SSE pump. */
  start: () => Promise<void>;
}

export function useStartBatch(): UseStartBatchResult {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;

  const isRunning = batch?.state === "running";
  const canStart =
    !!batch && batch.fileOrder.length > 0 && batch.pipeline.length > 0;

  const start = async () => {
    if (!batch) return;
    const firstVerb = batch.pipeline[0];
    if (!firstVerb) return;
    const kwargs = buildRecipeKwargs(firstVerb, batch.config[firstVerb]);
    const inputs = batch.fileOrder.map((id) => ({
      kind: "media" as const,
      path: `${batch.folderPath}/${batch.files[id].filename}`,
    }));
    try {
      const job = await submitRecipe(firstVerb, { ...kwargs, inputs });
      dispatch({
        type: "BATCH_STARTED",
        batchId: batch.id,
        jobId: job.job_id,
        files: batch.fileOrder.map((id) => batch.files[id]),
      });
      await invoke("start_batch_pump", {
        batchId: batch.id,
        jobId: job.job_id,
      });
    } catch (err) {
      // Recipe-submission failure is a batch-level error, NOT a
      // daemon failure. The daemon is fine — it just rejected this
      // particular request (e.g. bad kwargs, missing model). Don't
      // dispatch DAEMON_FAILED or we'd hide the entire UI behind the
      // boot overlay's error state.
      console.error("start_batch failed", err);
    }
  };

  return { canStart, isRunning, start };
}
