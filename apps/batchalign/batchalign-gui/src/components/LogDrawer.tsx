// Inline expanded view for a single file: stages on left, messages on
// right. Style ported from /tmp/batchalign-design/.../shared.jsx — the
// only change is that messages come from the store (`file.log`), not the
// mock array.

import type { FileRow } from "../store";

interface Props {
  file: FileRow;
}

function fmtTs(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number, w = 2) => n.toString().padStart(w, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

export default function LogDrawer({ file }: Props) {
  return (
    <div style={{ padding: "12px 20px 16px 20px" }}>
      <div style={{ display: "flex", gap: 18, alignItems: "flex-start" }}>
        <div style={{ flex: 1 }}>
          <div className="ba-eyebrow" style={{ marginBottom: 6 }}>
            stages
          </div>
          <table
            className="ba-table"
            style={{ fontSize: "var(--fs-sm)", background: "transparent" }}
          >
            <tbody>
              {file.stages.map((s) => (
                <tr key={s.verb}>
                  <td style={{ padding: "4px 8px", width: 130 }}>
                    <span
                      style={{
                        display: "inline-flex",
                        alignItems: "center",
                        gap: 5,
                      }}
                    >
                      <span
                        style={{
                          width: 7,
                          height: 7,
                          background: dotColor(s.state),
                          borderRadius: 1,
                          display: "inline-block",
                        }}
                      />
                      <span
                        style={{
                          fontSize: "var(--fs-sm)",
                          color: "var(--fg-muted)",
                        }}
                      >
                        {s.verb}
                      </span>
                    </span>
                  </td>
                  <td
                    style={{
                      padding: "4px 8px",
                      color:
                        s.state === "fail"
                          ? "var(--orange)"
                          : "var(--fg-muted)",
                    }}
                  >
                    {s.state === "running"
                      ? `${s.pct}%`
                      : s.state === "queued"
                        ? "waiting"
                        : s.state === "fail"
                          ? "failed"
                          : "complete"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div style={{ flex: 1.2 }}>
          <div
            style={{
              display: "flex",
              alignItems: "baseline",
              justifyContent: "space-between",
              marginBottom: 6,
            }}
          >
            <div className="ba-eyebrow">messages</div>
            {file.status === "failed" && (
              <button className="ba-btn ba-btn--sm">retry</button>
            )}
            {file.status === "running" && (
              <button className="ba-btn ba-btn--sm">cancel</button>
            )}
          </div>
          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              lineHeight: 1.55,
              background: "var(--surface)",
              border: "var(--rule)",
              borderRadius: "var(--r-1)",
              padding: "6px 0",
              color: "var(--fg)",
              maxHeight: 180,
              overflow: "auto",
            }}
          >
            {file.log.length === 0 && (
              <div
                style={{
                  padding: "0 10px",
                  color: "var(--fg-meta)",
                  fontStyle: "italic",
                }}
              >
                no messages yet
              </div>
            )}
            {file.log.map((entry, i) => {
              const isError = entry.level === "error";
              const isWarn = entry.level === "warn";
              return (
                <div
                  key={i}
                  style={{
                    background: isError
                      ? "rgba(221, 94, 66, 0.10)"
                      : "transparent",
                    borderLeft: isError
                      ? "2px solid var(--orange)"
                      : isWarn
                        ? "2px solid var(--brown)"
                        : "2px solid transparent",
                    paddingLeft: 8,
                    paddingRight: 10,
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      gap: 8,
                      whiteSpace: "nowrap",
                    }}
                  >
                    <span style={{ color: "var(--fg-meta)" }}>
                      {fmtTs(entry.ts)}
                    </span>
                    <span
                      style={{
                        width: 38,
                        flexShrink: 0,
                        color: isError
                          ? "var(--orange)"
                          : isWarn
                            ? "var(--brown)"
                            : "var(--fg-muted)",
                        fontWeight: isError || isWarn ? 700 : 500,
                      }}
                    >
                      {entry.level.toUpperCase()}
                    </span>
                    <span
                      style={{
                        whiteSpace: "normal",
                        color: isError ? "var(--orange)" : "var(--fg)",
                        fontWeight: isError ? 600 : 400,
                      }}
                    >
                      {entry.text}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

function dotColor(state: string): string {
  switch (state) {
    case "done":
      return "var(--green)";
    case "running":
      return "var(--brown)";
    case "fail":
      return "var(--orange)";
    default:
      return "var(--gray-2)";
  }
}
