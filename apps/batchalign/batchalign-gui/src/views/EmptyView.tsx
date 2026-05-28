// Cold-launch state. Centered dropzone overlaid on a faded silhouette
// of the working layout the user will see once they open a folder.

import DropZone from "../components/DropZone";

export default function EmptyView() {
  return (
    <div
      style={{
        flex: 1,
        position: "relative",
        minHeight: 0,
        overflow: "hidden",
      }}
    >
      <Silhouette />
      <DropZone />
    </div>
  );
}

function Silhouette() {
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        opacity: 0.28,
        pointerEvents: "none",
      }}
    >
      <div
        style={{
          width: 480,
          flexShrink: 0,
          borderRight: "var(--hairline)",
          display: "flex",
          flexDirection: "column",
          background: "var(--bg)",
        }}
      >
        <SilhouetteBlock label="1 · files" rows={3} />
        <SilhouetteBlock label="2 · pipeline" rows={5} />
      </div>
      <div
        style={{
          flex: 1,
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
            {Array.from({ length: 9 }).map((_, i) => (
              <tr key={i}>
                <td></td>
                <td>
                  <Bar w={180 + (i % 3) * 30} />
                </td>
                <td>
                  <Bar w={36} />
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
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function SilhouetteBlock({ label, rows }: { label: string; rows: number }) {
  return (
    <div style={{ borderBottom: "var(--hairline)" }}>
      <div
        style={{
          padding: "10px 20px 8px",
          background: "var(--bg-tinted)",
          borderBottom: "var(--hairline)",
          fontFamily: "var(--font-mono)",
          fontSize: 10,
          letterSpacing: "0.12em",
          color: "var(--fg-muted)",
          textTransform: "uppercase",
          fontWeight: 600,
        }}
      >
        {label}
      </div>
      <div style={{ padding: "14px 20px" }}>
        {Array.from({ length: rows }).map((_, i) => (
          <div
            key={i}
            style={{
              display: "grid",
              gridTemplateColumns: "120px 1fr",
              gap: 12,
              padding: "6px 0",
              borderBottom: "1px dotted var(--gray-1)",
            }}
          >
            <Bar w={90} />
            <Bar w="90%" />
          </div>
        ))}
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
