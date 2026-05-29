// Block 2 (pipeline): verb chain tabs + the active verb's panel.

import { useState } from "react";
import { useStore, type VerbStep } from "../store";
import BlockHeader from "./BlockHeader";
import VerbChainTabs from "./VerbChainTabs";
import { panelFor } from "../panels/registry";

export default function PipelineBlock() {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  // `null` = empty pipeline, nothing selectable. The VerbChainTabs
  // renders only the "+ add step" affordance in that state, and the
  // body below shows the empty-pipeline hint instead of a panel.
  const [selected, setSelected] = useState<VerbStep | null>(
    batch?.pipeline[0] ?? null,
  );
  if (!batch) return null;

  // Reconcile selection when the chain changes underneath us
  // (a remove, an add, or a fresh batch).
  if (selected && !batch.pipeline.includes(selected)) {
    setSelected(batch.pipeline[0] ?? null);
  } else if (!selected && batch.pipeline[0]) {
    setSelected(batch.pipeline[0]);
  }

  return (
    <>
      <BlockHeader index="2" title="pipeline" />
      <div style={{ padding: "12px 20px 16px" }}>
        <VerbChainTabs selected={selected} onSelect={setSelected} />
        {selected ? (
          (() => {
            const Panel = panelFor(selected);
            return (
              <Panel batchId={batch.id} config={batch.config[selected] ?? {}} />
            );
          })()
        ) : (
          <EmptyPipelineHint />
        )}
      </div>
    </>
  );
}

function EmptyPipelineHint() {
  return (
    <div
      style={{
        padding: "16px 0",
        fontSize: "var(--fs-sm)",
        color: "var(--fg-muted)",
        lineHeight: 1.5,
      }}
    >
      no pipeline steps yet. add one with{" "}
      <span
        className="ba-mono"
        style={{
          background: "var(--bg-sunken)",
          padding: "1px 6px",
          borderRadius: "var(--r-1)",
          fontSize: "var(--fs-xs)",
        }}
      >
        + add step
      </span>{" "}
      above. pick any combination of{" "}
      <span className="ba-mono">transcribe</span>,{" "}
      <span className="ba-mono">align</span>,{" "}
      <span className="ba-mono">morphotag</span>,{" "}
      <span className="ba-mono">translate</span>,{" "}
      <span className="ba-mono">compare</span> — they run in the order you
      list them.
    </div>
  );
}
