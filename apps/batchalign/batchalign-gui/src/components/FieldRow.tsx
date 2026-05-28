// Left-aligned label, right-aligned control. Dotted underline between rows.

import type { ReactNode } from "react";

interface Props {
  label: string;
  sub?: string;
  children: ReactNode;
  advanced?: boolean;
}

export default function FieldRow({
  label,
  sub,
  children,
  advanced,
}: Props) {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "150px 1fr",
        alignItems: "baseline",
        padding: "7px 0",
        borderBottom: "1px dotted var(--gray-1)",
        gap: 12,
      }}
    >
      <div>
        <div
          style={{
            fontSize: "var(--fs-sm)",
            fontWeight: 500,
            color: advanced ? "var(--fg-muted)" : "var(--fg)",
          }}
        >
          {label}
        </div>
        {sub && (
          <div
            style={{
              fontSize: "var(--fs-xs)",
              color: "var(--fg-meta)",
              marginTop: 1,
              lineHeight: 1.3,
            }}
          >
            {sub}
          </div>
        )}
      </div>
      <div>{children}</div>
    </div>
  );
}
