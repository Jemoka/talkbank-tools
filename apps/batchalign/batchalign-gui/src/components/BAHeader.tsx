// Wordmark bar: "Batchalign" (Kanit italic small-caps slate-blue) +
// optional eyebrow sub on the left, right-slot content (settings btn).

import type { ReactNode } from "react";

interface Props {
  sub?: string | null;
  right?: ReactNode;
}

export default function BAHeader({ sub = null, right = null }: Props) {
  return (
    <div
      style={{
        height: 52,
        flexShrink: 0,
        display: "flex",
        alignItems: "center",
        padding: "0 20px",
        background: "var(--bg)",
        borderBottom: "var(--hairline)",
        gap: 14,
      }}
    >
      <div className="ba-wordmark">
        <span className="mark">Batchalign</span>
      </div>
      {sub && (
        <>
          <div
            style={{
              width: 1,
              height: 14,
              background: "var(--gray-2)",
            }}
          />
          <div className="ba-eyebrow" style={{ marginTop: 1 }}>
            {sub}
          </div>
        </>
      )}
      <div style={{ flex: 1 }} />
      {right}
    </div>
  );
}
