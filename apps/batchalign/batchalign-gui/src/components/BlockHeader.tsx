// Mustard-tinted chapter divider with a slate-blue mono index + uppercase
// title. Used for the FILES / PIPELINE / etc. left-pane sections.

import type { ReactNode } from "react";

interface Props {
  index: string;
  title: string;
  sub?: string;
  control?: ReactNode;
}

export default function BlockHeader({
  index,
  title,
  sub,
  control,
}: Props) {
  return (
    <div
      style={{
        padding: "10px 20px 8px",
        background: "var(--bg-tinted)",
        borderBottom: "var(--hairline)",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          fontWeight: 700,
          color: "var(--dark-blue)",
        }}
      >
        {index}
      </span>
      <span
        style={{
          fontFamily: "var(--font-sans)",
          fontSize: "var(--fs-sm)",
          fontWeight: 700,
          letterSpacing: "0.04em",
          textTransform: "uppercase",
          color: "var(--fg)",
        }}
      >
        {title}
      </span>
      <span style={{ flex: 1 }} />
      {sub && (
        <span
          style={{
            fontSize: "var(--fs-xs)",
            color: "var(--fg-muted)",
          }}
        >
          {sub}
        </span>
      )}
      {control}
    </div>
  );
}
