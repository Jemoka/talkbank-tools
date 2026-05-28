// Bottom CLI string + copy button. The command derives from the active
// batch's pipeline + config. For now it's a best-effort preview; the
// real source of truth for execution is the daemon recipe POST.

import { useState } from "react";
import { useStore } from "../store";

function buildCli(): string {
  const { activeBatchId, batches } = useStore.getState();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return "";
  const cmds = batch.pipeline.map((v) => {
    const args: string[] = [`batchalign3 ${v}`];
    args.push(batch.folderPath);
    if (!batch.inPlace && batch.outputPath) args.push(batch.outputPath);
    return args.join(" ");
  });
  return cmds.join(" ; ");
}

export default function CommandPreview() {
  const [copied, setCopied] = useState(false);
  const cmd = buildCli();
  if (!cmd) return null;
  return (
    <div
      style={{
        flexShrink: 0,
        borderTop: "var(--hairline)",
        background: "var(--bg-tinted)",
        padding: "8px 14px 8px 16px",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <div
        className="ba-eyebrow"
        style={{ width: 38, flexShrink: 0 }}
      >
        cli
      </div>
      <div
        style={{
          flex: 1,
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--fg)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        <span style={{ color: "var(--fg-muted)" }}>$ </span>
        {cmd}
      </div>
      <button
        className="ba-btn ba-btn--sm"
        onClick={() => {
          navigator.clipboard?.writeText(cmd);
          setCopied(true);
          setTimeout(() => setCopied(false), 1400);
        }}
        style={{ fontFamily: "var(--font-mono)", fontSize: 11 }}
      >
        {copied ? "✓ copied" : "copy"}
      </button>
    </div>
  );
}
