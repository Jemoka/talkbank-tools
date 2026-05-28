// Window chrome — full-bleed macOS-style frame with traffic lights only.
// No menubar, no toolbar; everything lives in <BAHeader/> below.

import type { ReactNode } from "react";

interface Props {
  children: ReactNode;
}

export default function BAWindow({ children }: Props) {
  return (
    <div
      className="ba-app"
      style={{
        height: "100vh",
        width: "100vw",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          height: 28,
          display: "flex",
          alignItems: "center",
          padding: "0 12px",
          background: "var(--bg)",
          borderBottom: "var(--hairline)",
          flexShrink: 0,
        }}
      >
        {/* On macOS the OS draws the traffic lights for us via decorations:
            true. We keep this strip as the title-bar dado regardless of OS. */}
      </div>
      {children}
    </div>
  );
}
