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
import CommandPreview from "../components/CommandPreview";

export default function BatchView() {
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
            <FileTable />
          </div>
        </div>
      </div>
      <CommandPreview />
    </div>
  );
}
