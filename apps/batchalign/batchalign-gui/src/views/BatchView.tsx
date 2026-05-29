// The working layout (variant-a from the design canvas).
//
// Left pane (480px fixed): scrolling config column with FILES + PIPELINE
// blocks, followed by a sticky footer holding the batch summary and
// the "start batch" CTA. The footer never scrolls — the start action
// must stay reachable regardless of how tall the pipeline panel grows.
//
// Right pane (flexible): JobsHeader on top, scrolling FileTable below.
//
// CLI preview pinned to the very bottom across both panes.

import FilesBlock from "../components/FilesBlock";
import PipelineBlock from "../components/PipelineBlock";
import BatchActionFooter from "../components/BatchActionFooter";
import JobsHeader from "../components/JobsHeader";
import FileTable from "../components/FileTable";
import JobsPlaceholder from "../components/JobsPlaceholder";
import CommandPreview from "../components/CommandPreview";
import { useStore } from "../store";
import { useFilteredFiles } from "../hooks/useFilteredFiles";

export default function BatchView() {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  const visible = useFilteredFiles();

  // The right pane shows ONLY what the daemon would genuinely process:
  //   - while idle (no batch started), it's a silhouette placeholder
  //     with a one-line reason ("pick a pipeline step", or "no files
  //     match the chosen verb")
  //   - while running / done / failed, it's the real FileTable
  const isIdle = batch?.state === "idle";
  const reason = pickReason(batch?.pipeline.length ?? 0, visible.length);

  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
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
          <div className="ba-scroll" style={{ flex: 1, minHeight: 0 }}>
            <FilesBlock />
            <PipelineBlock />
          </div>
          <BatchActionFooter />
        </div>
        <div
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            minWidth: 0,
          }}
        >
          <JobsHeader />
          <div className="ba-scroll" style={{ flex: 1, minHeight: 0 }}>
            {isIdle ? (
              <JobsPlaceholder reason={reason} />
            ) : (
              <FileTable />
            )}
          </div>
        </div>
      </div>
      <CommandPreview />
    </div>
  );
}

function pickReason(pipelineLen: number, visibleCount: number): string {
  if (pipelineLen === 0) {
    return "pick a pipeline step to preview the files batchalign will process.";
  }
  if (visibleCount === 0) {
    return "no files in this folder match the first step's input kind. add a different step, or drop in a folder with matching files.";
  }
  return `${visibleCount} file${visibleCount === 1 ? "" : "s"} will be processed. hit start batch when ready.`;
}
