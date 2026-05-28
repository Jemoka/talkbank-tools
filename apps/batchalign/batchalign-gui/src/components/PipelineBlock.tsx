// Block 2 (pipeline): verb chain tabs + the active verb's panel.

import { useState } from "react";
import { useStore, type VerbStep } from "../store";
import BlockHeader from "./BlockHeader";
import VerbChainTabs from "./VerbChainTabs";
import { panelFor } from "../panels/registry";

export default function PipelineBlock() {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  const [selected, setSelected] = useState<VerbStep>(
    batch?.pipeline[0] ?? "transcribe",
  );
  if (!batch) return null;

  // Keep `selected` valid if the pipeline changes underneath us.
  if (!batch.pipeline.includes(selected) && batch.pipeline[0]) {
    setSelected(batch.pipeline[0]);
  }

  const Panel = panelFor(selected);
  const config = batch.config[selected] ?? {};

  return (
    <>
      <BlockHeader index="2" title="pipeline" />
      <div style={{ padding: "12px 20px 16px" }}>
        <VerbChainTabs selected={selected} onSelect={setSelected} />
        <Panel batchId={batch.id} config={config} />
      </div>
    </>
  );
}
