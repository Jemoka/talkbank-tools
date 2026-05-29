// Right-pane silhouette shown when there is nothing real to render
// — i.e. the user opened a folder but hasn't picked a pipeline yet,
// or the chosen first verb has no compatible inputs in the folder.
// Mirrors the silhouette pattern from EmptyView so the layout reads
// as "preview" rather than "broken".

export interface JobsPlaceholderProps {
  /** One-line explanation of *why* nothing is being shown right now. */
  reason: string;
}

export default function JobsPlaceholder({ reason }: JobsPlaceholderProps) {
  return (
    <div
      style={{
        flex: 1,
        position: "relative",
        minHeight: 0,
        overflow: "hidden",
      }}
    >
      {/* Faded layout silhouette behind the message — same gray bars
          the EmptyView uses, scaled to the right-pane width. */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.22,
          pointerEvents: "none",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <div
          style={{
            borderBottom: "var(--hairline)",
            padding: "14px 20px 12px",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "baseline",
          }}
        >
          <Bar w={120} h={16} />
          <div style={{ display: "flex", gap: 18 }}>
            {[40, 50, 60].map((w, i) => (
              <Bar key={i} w={w} h={14} />
            ))}
          </div>
        </div>
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
            {Array.from({ length: 9 }).map((_, i) => (
              <tr key={i}>
                <td></td>
                <td>
                  <Bar w={180 + (i % 3) * 30} />
                </td>
                <td>
                  <Bar w={20} />
                </td>
                <td>
                  <Bar w={36} />
                </td>
                <td>
                  <Bar w="80%" h={6} />
                </td>
                <td>
                  <Bar w={56} h={14} />
                </td>
                <td style={{ textAlign: "right" }}>
                  <Bar w={36} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Overlay message — sits on top, slightly inset from center
          so it doesn't look like a popover. */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 40,
          pointerEvents: "none",
        }}
      >
        <div
          style={{
            maxWidth: 420,
            textAlign: "center",
            color: "var(--fg-muted)",
            fontSize: "var(--fs-sm)",
            lineHeight: 1.55,
          }}
        >
          <div
            className="ba-eyebrow"
            style={{ marginBottom: 10 }}
          >
            no batch in flight
          </div>
          <div style={{ color: "var(--fg)" }}>{reason}</div>
        </div>
      </div>
    </div>
  );
}

function Bar({
  w,
  h = 10,
}: {
  w: number | string;
  h?: number;
}) {
  return (
    <div
      style={{
        width: typeof w === "number" ? `${w}px` : w,
        height: h,
        background: "var(--gray-1)",
        borderRadius: "var(--r-1)",
      }}
    />
  );
}
