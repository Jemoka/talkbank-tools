// The per-batch jobs table. Rows derive from the active batch in the
// store; expanding a row toggles `expandedFileId`. The drawer is rendered
// inline as a colspan'd tr underneath.

import { Fragment } from "react";
import { useStore } from "../store";
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

function fmtDuration(ms: number | null): string {
  if (ms == null) return "—";
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, "0")}:${sec
      .toString()
      .padStart(2, "0")}`;
  }
  return `${m}:${sec.toString().padStart(2, "0")}`;
}

export default function FileTable() {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return null;
  const expandedId = batch.expandedFileId;

  return (
    <table className="ba-table" style={{ tableLayout: "fixed" }}>
      <colgroup>
        <col style={{ width: 22 }} />
        <col />
        <col style={{ width: 70 }} />
        <col style={{ width: 60 }} />
        <col style={{ width: "38%" }} />
        <col style={{ width: 86 }} />
      </colgroup>
      <thead>
        <tr>
          <th></th>
          <th>file</th>
          <th>size</th>
          <th>duration</th>
          <th>pipeline</th>
          <th>status</th>
        </tr>
      </thead>
      <tbody>
        {batch.fileOrder.map((id) => {
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
                  className="ba-num"
                  style={{
                    color: "var(--fg-muted)",
                    fontSize: "var(--fs-sm)",
                    borderBottom: isOpen ? "1px solid transparent" : undefined,
                  }}
                >
                  {fmtDuration(file.durationMs)}
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
              </tr>
              {isOpen && (
                <tr>
                  <td
                    colSpan={6}
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
