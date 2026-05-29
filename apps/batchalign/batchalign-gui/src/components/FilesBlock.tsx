// Block 1 (files): the folder picker, in-place toggle, file count
// summary, and the "start batch" CTA. Drives BATCH_OPENED for the
// initial folder pick (handled at the EmptyView layer).

import { useStore } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { submitRecipe } from "../api";
import BlockHeader from "./BlockHeader";
import FieldRow from "./FieldRow";
import PathInput from "./PathInput";
import Toggle from "./Toggle";

/// Pick the per-recipe BackendSpec the daemon expects given a verb's
/// stored config blob. Each verb's Pydantic request model declares one
/// or more `*_backend: BackendSpec` kwargs (see python/batchalign/api.py
/// `_build_recipe_request_model`); we map our verb-config UI onto those
/// kwarg names here. Returns the body kwargs dict (without `inputs`).
function buildRecipeKwargs(
  verb: string,
  config: Record<string, unknown>,
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
    default:
      return {};
  }
}

export default function FilesBlock() {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return null;

  const onStart = async () => {
    const firstVerb = batch.pipeline[0];
    if (!firstVerb) {
      console.error("start_batch: empty pipeline");
      return;
    }
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
      // Kick the Rust shell to relay this job's SSE stream as
      // `progress-v2` events on a per-batch channel.
      await invoke("start_batch_pump", {
        batchId: batch.id,
        jobId: job.job_id,
      });
    } catch (err) {
      console.error("start_batch failed", err);
      dispatch({
        type: "DAEMON_FAILED",
        reason: `start ${firstVerb}: ${err}`,
      });
    }
  };


  const fileCount = batch.fileOrder.length;
  const totalSize = batch.fileOrder.reduce(
    (sum, id) => sum + batch.files[id].sizeBytes,
    0,
  );
  const totalMs = batch.fileOrder.reduce(
    (sum, id) => sum + (batch.files[id].durationMs ?? 0),
    0,
  );

  return (
    <>
      <BlockHeader
        index="1"
        title="files"
        control={
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 7,
              fontSize: "var(--fs-xs)",
              color: batch.inPlace ? "var(--fg)" : "var(--fg-muted)",
              fontWeight: batch.inPlace ? 600 : 500,
              cursor: "pointer",
              userSelect: "none",
            }}
            onClick={() =>
              dispatch({
                type: "BATCH_INPLACE_CHANGED",
                batchId: batch.id,
                inPlace: !batch.inPlace,
              })
            }
          >
            <Toggle on={batch.inPlace} />
            in place
          </span>
        }
      />
      <div style={{ padding: "12px 20px 14px" }}>
        <FieldRow label="folder">
          <PathInput value={batch.folderPath} directory />
        </FieldRow>
        {!batch.inPlace && (
          <FieldRow label="write to">
            <PathInput
              value={batch.outputPath ?? ""}
              onChange={(p) =>
                dispatch({
                  type: "BATCH_OUTPUT_CHANGED",
                  batchId: batch.id,
                  outputPath: p,
                })
              }
              directory
            />
          </FieldRow>
        )}
        <div
          style={{
            display: "flex",
            alignItems: "baseline",
            marginTop: 10,
            paddingTop: 8,
            borderTop: "1px dotted var(--gray-1)",
            gap: 6,
          }}
        >
          <div
            className="ba-num"
            style={{ fontSize: "var(--fs-md)", fontWeight: 700 }}
          >
            {fileCount} files
          </div>
          <div
            style={{
              color: "var(--fg-muted)",
              fontSize: "var(--fs-sm)",
            }}
          >
            · {(totalSize / 1024 / 1024).toFixed(1)} MB · {fmtTotal(totalMs)}
          </div>
          <div style={{ flex: 1 }} />
          <button
            type="button"
            className="ba-btn ba-btn--primary"
            disabled={batch.state === "running"}
            onClick={onStart}
          >
            {batch.state === "running" ? "running…" : "start batch"}
          </button>
        </div>
      </div>
    </>
  );
}

function fmtTotal(ms: number): string {
  if (ms === 0) return "—";
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
}
