// Tiny colored pill — same source-of-truth labels the dropdown sidebar
// uses. Maps store FileStatus + BatchState to the ba-badge variants.

import type { BatchState, FileStatus } from "../store";

interface Props {
  state: FileStatus | BatchState;
}

const MAP: Record<string, { cls: string; label: string }> = {
  running: { cls: "ba-badge ba-badge--running", label: "running" },
  queued: { cls: "ba-badge ba-badge--queued", label: "queued" },
  done: { cls: "ba-badge ba-badge--done", label: "done" },
  failed: { cls: "ba-badge ba-badge--fail", label: "failed" },
  fail: { cls: "ba-badge ba-badge--fail", label: "failed" },
  idle: { cls: "ba-badge ba-badge--queued", label: "idle" },
  pending: { cls: "ba-badge ba-badge--pending", label: "pending" },
};

export default function StatusBadge({ state }: Props) {
  const m = MAP[state] ?? MAP.queued;
  return <span className={m.cls}>{m.label}</span>;
}
