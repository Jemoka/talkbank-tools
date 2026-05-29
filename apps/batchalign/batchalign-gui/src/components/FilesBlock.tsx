// Block 1 (files): the folder picker, in-place toggle, and a quiet
// count/size/duration summary line. The "start batch" action lives in
// the BatchActionFooter at the bottom of the left pane (per the
// design canvas's variant-a placement), NOT inside this block.

import { useStore } from "../store";
import BlockHeader from "./BlockHeader";
import FieldRow from "./FieldRow";
import PathInput from "./PathInput";
import Toggle from "./Toggle";

export default function FilesBlock() {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return null;

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
