// Per-row stage filler. Stages renders as adjacent flex children; each
// stage shows its label below as a small caption (unless `compact`).
//
// state colors mirror the design canvas:
//   done    → olive       (var(--green))
//   running → mustard     (var(--brown))
//   queued  → light gray  (var(--gray-2))
//   fail    → terracotta  (var(--orange))

import type { StageRow } from "../store";

const STATE_COLORS: Record<StageRow["state"], string> = {
  done: "var(--green)",
  running: "var(--brown)",
  queued: "var(--gray-2)",
  fail: "var(--orange)",
};

interface Props {
  stages: StageRow[];
  compact?: boolean;
}

export default function PipelineRibbon({ stages, compact = false }: Props) {
  const h = compact ? 4 : 8;
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 1,
        width: "100%",
      }}
    >
      {stages.map((s) => {
        const isRunning = s.state === "running";
        return (
          <div key={s.verb} style={{ flex: 1, position: "relative" }}>
            <div
              style={{
                height: h,
                background: "var(--bg-tinted)",
                border: "var(--hairline)",
                borderRadius: 1,
                overflow: "hidden",
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  bottom: 0,
                  width: isRunning
                    ? `${s.pct}%`
                    : s.state === "queued"
                      ? 0
                      : "100%",
                  background: STATE_COLORS[s.state],
                }}
              />
            </div>
            {!compact && (
              <div
                className="ba-eyebrow"
                style={{
                  marginTop: 4,
                  fontSize: 9,
                  letterSpacing: "0.09em",
                  color: isRunning
                    ? "var(--brown)"
                    : s.state === "queued"
                      ? "var(--fg-meta)"
                      : "var(--fg-muted)",
                  fontWeight: isRunning ? 700 : 500,
                }}
              >
                {s.verb}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
