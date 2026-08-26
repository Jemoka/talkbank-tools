// Centered dropzone for the empty state. "Open folder…" button uses the
// Tauri dialog plugin; the parent then dispatches BATCH_OPENED.

import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useStore, type Batch, type FileRow } from "../store";

// No default pipeline — the user chooses every step themselves so the
// chain reflects exactly what they intend to run. PipelineBlock renders
// an "+ add step" affordance for the empty case.

interface FolderSummary {
  files: Array<{
    source_id: string;
    stem: string;
    filename: string;
    size_bytes: number;
    duration_ms: number | null;
    kind: "media" | "chat";
  }>;
}

export default function DropZone() {
  const { dispatch } = useStore();

  const onOpen = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    await openFolder(selected);
  };

  const openFolder = async (path: string) => {
    let summary: FolderSummary | null = null;
    try {
      summary = (await invoke("list_folder_files", {
        path,
      })) as FolderSummary;
    } catch (err) {
      console.error("list_folder_files failed", err);
    }
    const files: Record<string, FileRow> = {};
    const fileOrder: string[] = [];
    for (const f of summary?.files ?? []) {
      files[f.source_id] = {
        source_id: f.source_id,
        stem: f.stem,
        filename: f.filename,
        sizeBytes: f.size_bytes,
        durationMs: f.duration_ms,
        kind: f.kind,
        status: "queued",
        // Pipeline stages are materialized when the user picks verbs
        // — empty until then so the row doesn't display ghost stages.
        stages: [],
        log: [],
      };
      fileOrder.push(f.source_id);
    }

    const batchId = `b-${Date.now().toString(36)}-${Math.random()
      .toString(36)
      .slice(2, 6)}`;
    const name = path.split(/[\\/]/).pop() ?? "batch";
    const batch: Batch = {
      id: batchId,
      name,
      folderPath: path,
      inPlace: true,
      outputPath: null,
      pipeline: [],
      config: {
        transcribe: {},
        diarize: {},
        align: {},
        morphotag: {},
        translate: {},
        compare: {},
      },
      files,
      fileOrder,
      state: "idle",
      jobId: null,
      startedAt: null,
      finishedAt: null,
      expandedFileId: null,
    };
    dispatch({ type: "BATCH_OPENED", batch });
  };

  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 40,
      }}
    >
      <div
        style={{
          width: 520,
          background: "var(--bg)",
          border: "2px dashed var(--gray-2)",
          borderRadius: "var(--r-1)",
          padding: "36px 28px 28px",
          textAlign: "center",
        }}
      >
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 28,
            color: "var(--gray-3)",
            lineHeight: 1,
            marginBottom: 14,
          }}
        >
          ↓
        </div>
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontWeight: 600,
            fontSize: 22,
            color: "var(--fg)",
            lineHeight: 1.15,
            marginBottom: 6,
          }}
        >
          drop a folder
        </div>
        <div
          style={{
            fontSize: "var(--fs-sm)",
            color: "var(--fg-muted)",
            marginBottom: 22,
          }}
        >
          folders are searched recursively.
        </div>
        <button
          type="button"
          className="ba-btn ba-btn--primary"
          onClick={onOpen}
          style={{ minWidth: 160 }}
        >
          open folder…
        </button>
      </div>
    </div>
  );
}
