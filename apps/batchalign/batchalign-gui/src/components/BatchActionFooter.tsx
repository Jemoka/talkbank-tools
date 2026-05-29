// Sticky footer at the bottom of the left config pane.
//
// Per the design canvas (variant-a.jsx), the "start batch" affordance
// is NOT inside the FILES block — it lives in its own footer with a
// short summary line ("N files · M steps") on the left and the
// primary action on the right. The footer sits below the scrolling
// config column so it stays reachable when the panels grow tall.

import { useStore } from "../store";
import { useStartBatch } from "../hooks/useStartBatch";

export default function BatchActionFooter() {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  const { canStart, isRunning, start } = useStartBatch();
  if (!batch) return null;

  return (
    <div
      style={{
        padding: "12px 20px",
        borderTop: "var(--hairline)",
        background: "var(--bg)",
        display: "flex",
        gap: 10,
        alignItems: "center",
      }}
    >
      <div
        style={{
          fontSize: "var(--fs-sm)",
          color: "var(--fg-muted)",
        }}
      >
        <span className="ba-num">{batch.fileOrder.length}</span> files ·{" "}
        <span className="ba-num">{batch.pipeline.length}</span> steps
      </div>
      <div style={{ flex: 1 }} />
      <button
        type="button"
        className="ba-btn ba-btn--primary"
        disabled={!canStart || isRunning}
        onClick={start}
      >
        {isRunning ? "running…" : "start batch"}
      </button>
    </div>
  );
}
