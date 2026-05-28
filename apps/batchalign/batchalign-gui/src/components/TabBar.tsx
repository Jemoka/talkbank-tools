// Finder-style batch tabs. Tab state derives from the store; the bar
// reads `state.tabOrder` + `state.activeBatchId` and dispatches
// BATCH_ACTIVATED on click. Close icon is visible only on the active tab.

import { useStore } from "../store";

export default function TabBar() {
  const { tabOrder, batches, activeBatchId, dispatch } = useStore();

  if (tabOrder.length === 0) return null;

  const colorByState: Record<string, string> = {
    running: "var(--brown)",
    done: "var(--green)",
    failed: "var(--orange)",
    idle: "var(--gray-3)",
  };

  return (
    <div
      style={{
        flexShrink: 0,
        height: 34,
        display: "flex",
        alignItems: "stretch",
        background: "var(--bg-tinted)",
        borderBottom: "var(--hairline)",
        padding: "0 6px",
      }}
    >
      {tabOrder.map((id) => {
        const batch = batches[id];
        if (!batch) return null;
        const isActive = id === activeBatchId;
        const total = batch.fileOrder.length;
        const done = batch.fileOrder.filter(
          (fid) => batch.files[fid].status === "done",
        ).length;
        const summary =
          batch.state === "idle"
            ? null
            : batch.state === "done"
              ? `${done} / ${total} · complete`
              : `${done} / ${total}`;
        return (
          <div
            key={id}
            onClick={() => dispatch({ type: "BATCH_ACTIVATED", batchId: id })}
            style={{
              position: "relative",
              display: "flex",
              alignItems: "center",
              gap: 8,
              padding: "0 12px 0 10px",
              background: isActive ? "var(--bg)" : "transparent",
              borderTop: isActive
                ? "2px solid var(--dark-blue)"
                : "2px solid transparent",
              borderLeft: `1px solid ${isActive ? "var(--gray-1)" : "transparent"}`,
              borderRight: `1px solid ${isActive ? "var(--gray-1)" : "transparent"}`,
              marginBottom: -1,
              borderBottom: isActive
                ? "1px solid var(--bg)"
                : "var(--hairline)",
              cursor: "pointer",
              minWidth: 170,
              maxWidth: 220,
            }}
          >
            <span
              style={{
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: colorByState[batch.state] ?? "var(--gray-3)",
                flexShrink: 0,
              }}
            />
            <div
              style={{
                flex: 1,
                minWidth: 0,
                display: "flex",
                flexDirection: "column",
                lineHeight: 1.05,
              }}
            >
              <span
                style={{
                  fontSize: "var(--fs-sm)",
                  fontWeight: isActive ? 600 : 500,
                  color: "var(--fg)",
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
              >
                {batch.name}
              </span>
              {summary && (
                <span
                  className="ba-num"
                  style={{
                    fontSize: 10,
                    color: "var(--fg-muted)",
                    marginTop: 1,
                  }}
                >
                  {summary}
                </span>
              )}
            </div>
            <span
              onClick={(e) => {
                e.stopPropagation();
                dispatch({ type: "BATCH_CLOSED", batchId: id });
              }}
              style={{
                width: 14,
                height: 14,
                lineHeight: "12px",
                fontSize: 12,
                textAlign: "center",
                color: "var(--fg-meta)",
                cursor: "pointer",
                flexShrink: 0,
                visibility: isActive ? "visible" : "hidden",
              }}
            >
              ×
            </span>
          </div>
        );
      })}
      <div
        style={{
          flex: 1,
          borderBottom: "var(--hairline)",
          marginBottom: -1,
        }}
      />
    </div>
  );
}
