// Right-pane header: "N of M done|processing|queued · eta · elapsed ·
// workers · cancel|reveal". Pure derivation from the active batch in the
// store.

import { useStore } from "../store";
import Stat from "./Stat";

function fmtHMS(ms: number): string {
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

export default function JobsHeader() {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return null;

  const total = batch.fileOrder.length;
  const done = batch.fileOrder.filter(
    (id) => batch.files[id].status === "done",
  ).length;
  const running = batch.fileOrder.filter(
    (id) => batch.files[id].status === "running",
  ).length;
  const isDone = batch.state === "done";
  const isFailed = batch.state === "failed";
  const isRunning = batch.state === "running";
  const verb = isDone
    ? "done"
    : isFailed
      ? "failed"
      : isRunning
        ? "processing"
        : "queued";

  const elapsed =
    batch.startedAt != null
      ? fmtHMS((batch.finishedAt ?? Date.now()) - batch.startedAt)
      : "—";

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "14px 20px 12px",
        borderBottom: "var(--hairline)",
        gap: 16,
        background: "var(--bg)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          gap: 14,
        }}
      >
        <div>
          <div className="ba-eyebrow">batch</div>
          <div
            style={{
              fontSize: "var(--fs-lg)",
              fontWeight: 500,
              marginTop: 1,
              lineHeight: 1.1,
            }}
          >
            <span className="ba-num">{isDone ? done : running}</span>
            <span style={{ color: "var(--fg-muted)" }}> of </span>
            <span className="ba-num">{total}</span>
            <span style={{ color: "var(--fg-muted)" }}> {verb}</span>
          </div>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 22 }}>
        {!isDone && <Stat label="elapsed" value={elapsed} mono />}
        {isDone && <Stat label="elapsed" value={elapsed} mono />}
        {isRunning ? (
          <button className="ba-btn ba-btn--sm">cancel batch</button>
        ) : isDone ? (
          <button className="ba-btn ba-btn--sm">reveal outputs</button>
        ) : null}
      </div>
    </div>
  );
}
