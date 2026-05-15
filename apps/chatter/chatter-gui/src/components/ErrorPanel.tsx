/**
 * Error detail panel — right side of the main layout.
 *
 * Renders validation errors using pre-rendered miette HTML from the Rust
 * backend. This gives identical output to the CLI — same box-drawing
 * characters, same source snippets, same caret underlines.
 */

import { useCallback, useMemo, useState } from "react";
import type { FileEntry, ParseError } from "../types";

type SeverityFilter = "all" | "Error" | "Warning";

interface Props {
  file: FileEntry | null;
  clanAvailable: boolean;
  onOpenInClan: (file: string, error: ParseError) => void;
  onRevealFile?: (path: string) => void;
}

export default function ErrorPanel({
  file,
  clanAvailable,
  onOpenInClan,
  onRevealFile,
}: Props) {
  const [codeFilter, setCodeFilter] = useState("");
  const [severityFilter, setSeverityFilter] = useState<SeverityFilter>("all");
  const [collapsedCards, setCollapsedCards] = useState<Set<number>>(new Set());
  const [allCollapsed, setAllCollapsed] = useState(false);

  const filteredDiagnostics = useMemo(() => {
    if (!file) return [];
    return file.diagnostics.filter((d) => {
      if (
        severityFilter !== "all" &&
        d.error.severity !== severityFilter
      ) {
        return false;
      }
      if (
        codeFilter &&
        !d.error.code.toLowerCase().includes(codeFilter.toLowerCase())
      ) {
        return false;
      }
      return true;
    });
  }, [file, codeFilter, severityFilter]);

  const toggleCard = useCallback((index: number) => {
    setCollapsedCards((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }, []);

  const toggleAll = useCallback(() => {
    if (allCollapsed) {
      setCollapsedCards(new Set());
      setAllCollapsed(false);
    } else {
      setCollapsedCards(new Set(filteredDiagnostics.map((_, i) => i)));
      setAllCollapsed(true);
    }
  }, [allCollapsed, filteredDiagnostics]);

  if (!file) {
    return (
      <div className="error-panel">
        <div className="error-panel-empty">Select a file to view errors</div>
      </div>
    );
  }

  if (file.diagnostics.length === 0) {
    const statusLabel =
      file.status?.type === "valid"
        ? "Valid"
        : file.status?.type === "parseError"
          ? `Parse error: ${file.status.message}`
          : file.status?.type === "readError"
            ? `Read error: ${file.status.message}`
            : "No errors";

    return (
      <div className="error-panel">
        <h2>
          <span
            className={onRevealFile ? "file-path-link" : undefined}
            onClick={onRevealFile ? () => onRevealFile(file.path) : undefined}
          >
            {file.name}
          </span>
        </h2>
        <div className="error-panel-empty" style={{ color: "var(--color-valid)" }}>
          {statusLabel}
        </div>
      </div>
    );
  }

  const showBulkToggle = filteredDiagnostics.length >= 5;

  return (
    <div className="error-panel">
      <h2>
        <span
          className={onRevealFile ? "file-path-link" : undefined}
          onClick={onRevealFile ? () => onRevealFile(file.path) : undefined}
        >
          {file.name}
        </span>
        {" \u2014 "}
        {filteredDiagnostics.length} error
        {filteredDiagnostics.length !== 1 ? "s" : ""}
        {filteredDiagnostics.length !== file.diagnostics.length &&
          ` (${file.diagnostics.length} total)`}
      </h2>

      <div className="filter-bar">
        <input
          type="text"
          placeholder="Filter by code\u2026"
          value={codeFilter}
          onChange={(e) => setCodeFilter(e.target.value)}
        />
        <div className="severity-toggle">
          {(["all", "Error", "Warning"] as const).map((s) => (
            <button
              key={s}
              className={severityFilter === s ? "active" : ""}
              onClick={() => setSeverityFilter(s)}
            >
              {s === "all" ? "All" : s === "Error" ? "Errors" : "Warnings"}
            </button>
          ))}
        </div>
        {showBulkToggle && (
          <button className="bulk-toggle" onClick={toggleAll}>
            {allCollapsed ? "Expand All" : "Collapse All"}
          </button>
        )}
      </div>

      {filteredDiagnostics.map((diagnostic, i) => (
        <ErrorCard
          key={`${diagnostic.error.code}-${diagnostic.error.location.start}-${i}`}
          error={diagnostic.error}
          renderedHtml={diagnostic.renderedHtml}
          renderedText={diagnostic.renderedText}
          clanAvailable={clanAvailable}
          onOpenInClan={() => onOpenInClan(file.path, diagnostic.error)}
          collapsed={collapsedCards.has(i)}
          onToggle={() => toggleCard(i)}
        />
      ))}
    </div>
  );
}

function ErrorCard({
  error,
  renderedHtml,
  renderedText,
  clanAvailable,
  onOpenInClan,
  collapsed,
  onToggle,
}: {
  error: ParseError;
  renderedHtml: string;
  renderedText: string;
  clanAvailable: boolean;
  onOpenInClan: () => void;
  collapsed: boolean;
  onToggle: () => void;
}) {
  const [copyLabel, setCopyLabel] = useState("Copy");

  const handleCopy = useCallback(() => {
    void navigator.clipboard.writeText(renderedText).then(() => {
      setCopyLabel("Copied \u2713");
      setTimeout(() => setCopyLabel("Copy"), 1500);
    });
  }, [renderedText]);

  return (
    <div className="error-card">
      <div className="error-card-toggle" onClick={onToggle}>
        <span className={`error-card-chevron ${collapsed ? "collapsed" : ""}`}>
          {"\u25BE"}
        </span>
        <span className={`error-code ${error.severity === "Error" ? "error" : "warning"}`}>
          [{error.code}]
        </span>
        <span className="error-message">
          {error.message}
        </span>
      </div>

      <div className={`error-card-body ${collapsed ? "collapsed" : ""}`}>
        {renderedHtml ? (
          <pre
            className="error-miette"
            dangerouslySetInnerHTML={{ __html: renderedHtml }}
          />
        ) : (
          <div className="error-card-header">
            <span className={`error-code ${error.severity === "Error" ? "error" : "warning"}`}>
              [{error.code}]
            </span>
            <span className="error-message">{error.message}</span>
          </div>
        )}

        <div className="error-actions">
          <button onClick={handleCopy}>{copyLabel}</button>
          {clanAvailable && (
            <button onClick={onOpenInClan}>Open in CLAN</button>
          )}
        </div>
      </div>
    </div>
  );
}
