// Full-window overlay shown while the embedded Python daemon is
// booting — and not torn down until /capabilities has loaded. Captures
// every pointer event so the user can't click partially-armed widgets
// (file picker, recipe panels, "start batch") before the daemon is
// reachable.
//
// Design intent: visible but calm. A breathing two-bar tick that
// advances once per second, an elapsed timer, and a one-line
// expectation-setting message. The audience is clinical researchers,
// not developers — no implementation jargon (PyApp, uvicorn, SSE)
// on screen.

import { useEffect, useMemo, useState } from "react";
import { useStore } from "../store";

export default function DaemonBootOverlay() {
  const { daemon } = useStore();
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    const startedAt = Date.now();
    const tick = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startedAt) / 1000));
    }, 250);
    return () => clearInterval(tick);
  }, []);

  // Two-bar tick that advances once per second. Even seconds show the
  // left bar lit; odd seconds show the right. No transitions — discrete
  // pulses are calmer than a smooth animation.
  const tickGlyph = useMemo(() => {
    const left = elapsed % 2 === 0 ? "▍" : "▏";
    const right = elapsed % 2 === 0 ? "▏" : "▍";
    return `${left} ${right}`;
  }, [elapsed]);

  const hasError = !!daemon.error;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-busy={!hasError}
      aria-label={hasError ? "Loading failed" : "Loading"}
      onMouseDownCapture={swallow}
      onClickCapture={swallow}
      onKeyDownCapture={swallow}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1000,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(250, 250, 250, 0.96)",
        backdropFilter: "blur(2px)",
        WebkitBackdropFilter: "blur(2px)",
      }}
    >
      <div
        style={{
          width: 560,
          maxWidth: "calc(100% - 80px)",
          padding: "28px 32px 26px",
          background: "var(--bg)",
          border: hasError ? "1px solid #B85C5C" : "var(--hairline)",
          borderRadius: "var(--r-1)",
          boxShadow: "0 1px 0 var(--gray-1), 0 12px 28px rgba(0,0,0,0.06)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "baseline",
            justifyContent: "space-between",
            marginBottom: 18,
            paddingBottom: 12,
            borderBottom: "var(--hairline)",
          }}
        >
          <div
            className="ba-eyebrow"
            style={{
              color: hasError ? "#B85C5C" : "var(--fg-muted)",
            }}
          >
            {hasError ? "loading failed" : "loading"}
          </div>
          <div
            className="ba-num"
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--fg-meta)",
              fontVariantNumeric: "tabular-nums",
            }}
          >
            {hasError ? "—" : formatElapsed(elapsed)}
          </div>
        </div>

        {hasError ? (
          <ErrorBody reason={daemon.error!} />
        ) : (
          <BootingBody tickGlyph={tickGlyph} progressLine={daemon.progressLine} />
        )}
      </div>
    </div>
  );
}

function BootingBody({
  tickGlyph,
  progressLine,
}: {
  tickGlyph: string;
  progressLine: string | null;
}) {
  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 14,
          marginBottom: 14,
        }}
      >
        <span
          className="ba-mono"
          aria-hidden="true"
          style={{
            fontSize: 28,
            lineHeight: 1,
            letterSpacing: "0.06em",
            color: "var(--fg)",
            minWidth: 56,
          }}
        >
          {tickGlyph}
        </span>
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontWeight: 600,
            fontSize: 20,
            color: "var(--fg)",
            lineHeight: 1.15,
          }}
        >
          Loading…
        </div>
      </div>
      <p
        style={{
          fontSize: "var(--fs-sm)",
          lineHeight: 1.55,
          color: "var(--fg-muted)",
          margin: 0,
          marginBottom: progressLine ? 10 : 0,
        }}
      >
        This takes a few minutes upon first load, but will be much faster
        after that.
      </p>
      {progressLine && (
        <div
          className="ba-mono"
          aria-live="polite"
          style={{
            fontSize: "var(--fs-xs)",
            color: "var(--fg-meta)",
            background: "var(--bg-sunken)",
            border: "var(--hairline)",
            borderRadius: "var(--r-1)",
            padding: "6px 10px",
            marginTop: 6,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={progressLine}
        >
          {progressLine}
        </div>
      )}
    </>
  );
}

function ErrorBody({ reason }: { reason: string }) {
  return (
    <>
      <div
        style={{
          fontFamily: "var(--font-sans)",
          fontWeight: 600,
          fontSize: 20,
          color: "var(--fg)",
          marginBottom: 14,
          lineHeight: 1.2,
        }}
      >
        Batchalign couldn&rsquo;t start.
      </div>
      <div
        className="ba-mono"
        style={{
          fontSize: "var(--fs-xs)",
          textTransform: "uppercase",
          letterSpacing: "0.12em",
          color: "var(--fg-meta)",
          marginBottom: 6,
        }}
      >
        reason
      </div>
      <pre
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "var(--fs-sm)",
          lineHeight: 1.5,
          color: "var(--fg)",
          background: "var(--bg-sunken)",
          border: "var(--hairline)",
          borderRadius: "var(--r-1)",
          margin: 0,
          padding: "10px 12px",
          whiteSpace: "pre-wrap",
          maxHeight: 240,
          overflow: "auto",
        }}
      >
        {reason}
      </pre>
      <div
        style={{
          fontSize: "var(--fs-sm)",
          color: "var(--fg-muted)",
          marginTop: 14,
          lineHeight: 1.5,
        }}
      >
        relaunching the app is the easiest first step. if it keeps
        failing, the terminal window has the application&rsquo;s
        stdout/stderr prefixed with{" "}
        <code
          className="ba-mono"
          style={{
            background: "var(--bg-sunken)",
            padding: "0 4px",
            borderRadius: 2,
          }}
        >
          [daemon stdout]
        </code>{" "}
        /{" "}
        <code
          className="ba-mono"
          style={{
            background: "var(--bg-sunken)",
            padding: "0 4px",
            borderRadius: 2,
          }}
        >
          [daemon stderr]
        </code>
        .
      </div>
    </>
  );
}

function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds.toString().padStart(2, "0")}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}

function swallow(e: React.SyntheticEvent) {
  e.stopPropagation();
  e.preventDefault();
}
