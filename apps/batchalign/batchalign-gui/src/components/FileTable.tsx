// The per-batch jobs table. Rows derive from the active batch in the
// store; expanding a row toggles `expandedFileId`. The drawer is rendered
// inline as a colspan'd tr underneath.

import { Fragment } from "react";
import { useStore } from "../store";
import { useFilteredFiles } from "../hooks/useFilteredFiles";
import PipelineRibbon from "./PipelineRibbon";
import StatusBadge from "./StatusBadge";
import LogDrawer from "./LogDrawer";

function fmtSize(bytes: number): string {
  if (bytes === 0) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[u]}`;
}

export default function FileTable() {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  // Restrict to files the daemon would actually pick up given the
  // first verb in the pipeline (mirrors python/batchalign/inputs.py's
  // iter_media / iter_chat + per-recipe input-kind expectations).
  const visibleIds = useFilteredFiles();
  if (!batch) return null;
  const expandedId = batch.expandedFileId;

  return (
    <table className="ba-table" style={{ tableLayout: "fixed" }}>
      <colgroup>
        <col style={{ width: 22 }} />
        <col />
        <col style={{ width: 56 }} />
        <col style={{ width: 60 }} />
        <col style={{ width: "38%" }} />
        <col style={{ width: 86 }} />
        <col style={{ width: 70 }} />
      </colgroup>
      <thead>
        <tr>
          <th></th>
          <th>file</th>
          <th>lang</th>
          <th>size</th>
          <th>pipeline</th>
          <th>status</th>
          <th style={{ textAlign: "right" }}>eta</th>
        </tr>
      </thead>
      <tbody>
        {visibleIds.map((id) => {
          const file = batch.files[id];
          if (!file) return null;
          const isOpen = expandedId === id;
          return (
            <Fragment key={id}>
              <tr
                onClick={() =>
                  dispatch({
                    type: "FILE_EXPANDED",
                    batchId: batch.id,
                    sourceId: isOpen ? null : id,
                  })
                }
                style={{
                  cursor: "pointer",
                  background: isOpen ? "var(--bg-sunken)" : undefined,
                }}
              >
                <td
                  style={{
                    padding: "8px 6px 8px 9px",
                    borderLeft: isOpen
                      ? "3px solid var(--dark-blue)"
                      : "3px solid transparent",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  <span
                    style={{
                      display: "inline-block",
                      width: 10,
                      lineHeight: 1,
                      color: isOpen ? "var(--dark-blue)" : "var(--fg-meta)",
                      fontSize: 11,
                      transform: isOpen ? "rotate(90deg)" : "rotate(0)",
                      transformOrigin: "50% 50%",
                      fontWeight: isOpen ? 700 : 400,
                    }}
                  >
                    ▸
                  </span>
                </td>
                <td
                  style={{
                    overflow: "hidden",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  <div
                    style={{
                      fontSize: "var(--fs-md)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      fontWeight: isOpen ? 600 : 400,
                    }}
                  >
                    {file.stem}
                  </div>
                </td>
                <td
                  className="ba-mono"
                  style={{
                    color: "var(--fg-muted)",
                    fontSize: "var(--fs-sm)",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  {(batch.config.transcribe?.lang as string) || "—"}
                </td>
                <td
                  className="ba-num"
                  style={{
                    color: "var(--fg-muted)",
                    fontSize: "var(--fs-sm)",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  {fmtSize(file.sizeBytes)}
                </td>
                <td
                  style={{
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  <PipelineRibbon stages={file.stages} />
                </td>
                <td
                  style={{
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  <StatusBadge state={file.status} />
                </td>
                <td
                  className="ba-num"
                  style={{
                    color: "var(--fg-muted)",
                    fontSize: "var(--fs-sm)",
                    textAlign: "right",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  {/* TODO: surface daemon ETA from progress events. */}
                  —
                </td>
              </tr>
              {isOpen && (
                <tr>
                  <td
                    colSpan={7}
                    style={{
                      padding: 0,
                      background: "var(--bg-sunken)",
                      borderLeft: "3px solid var(--dark-blue)",
                      borderBottom: "1px solid var(--gray-2)",
                    }}
                  >
                    <LogDrawer file={file} />
                  </td>
                </tr>
              )}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}
