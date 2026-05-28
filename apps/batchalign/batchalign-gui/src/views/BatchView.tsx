// The working layout. Left = config (FilesBlock + PipelineBlock).
// Right = JobsHeader + FileTable. CLI preview pinned to the bottom.

import FilesBlock from "../components/FilesBlock";
import PipelineBlock from "../components/PipelineBlock";
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
