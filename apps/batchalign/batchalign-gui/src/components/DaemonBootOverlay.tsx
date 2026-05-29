// Full-window overlay shown while the embedded Python daemon is
// booting — and not torn down until /capabilities has loaded. Captures
// every pointer event so the user can't click partially-armed widgets
// (file picker, recipe panels, "start batch") before the daemon is
// reachable.
//
// Design intent: visible but calm. No spinner, no bouncing dots. A
// breathing two-bar tick that advances once per second, an elapsed
// timer, and a slowly rotating prose line drawn from a small pool of
// facts about what's actually happening during cold-start (PyApp
// unpacking CPython, pip installing the api extra, uvicorn binding,
// FastAPI introspection of the recipes module). Density without
// motion.

import { useEffect, useMemo, useState } from "react";
import { useStore } from "../store";

const FACTS: ReadonlyArray<string> = [
  "the embedded daemon is a PyApp bundle: a self-contained CPython runtime + the batchalign wheel + the [api] extra. first launch unpacks all three into ~/Library/Application Support/pyapp/. subsequent launches reuse the cache and start in under a second.",
  "every recipe — transcribe, align, morphotag, translate, compare — is a thin pairing of `tasks` and `backends` in batchalign.recipes. FastAPI introspects each recipe's signature at startup and generates a Pydantic request model so the HTTP surface is always in sync with the python.",
  "the GUI talks to the daemon over loopback HTTP. the sidecar binds 127.0.0.1 only and is shut down when the Tauri shell exits — no daemons survive a window close.",
  "uvicorn picks an unused port via --port 0; the GUI discovers it by reading the daemon's `DAEMON_PORT=<n>` stdout line (or, since 2026-05, by recognizing uvicorn's own \"Uvicorn running on http://...\" log as a fallback).",
  "ASR engines aren't loaded until the first transcribe job hits them. capabilities are advertised eagerly; weights are demand-loaded.",
  "the daemon's job registry is in-memory and per-process. workers are pinned to 1 by default — a shared backend would be needed to scale beyond a single host process.",
  "progress events stream over a per-job SSE channel at /jobs/<id>/events. the Tauri shell relays them onto the `progress-v2` event channel tagged with the originating batch id so the React store can route to the right tab.",
];

export default function DaemonBootOverlay() {
  const { daemon } = useStore();
  const [elapsed, setElapsed] = useState(0);
  const [factIdx, setFactIdx] = useState(() => Math.floor(Math.random() * FACTS.length));

  useEffect(() => {
    const startedAt = Date.now();
    const tick = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startedAt) / 1000));
    }, 250);
    const rotate = setInterval(() => {
      setFactIdx((i) => (i + 1) % FACTS.length);
    }, 9000);
    return () => {
      clearInterval(tick);
      clearInterval(rotate);
    };
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
  const fact = FACTS[factIdx];

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-busy={!hasError}
      aria-label={hasError ? "Daemon failed to start" : "Starting daemon"}
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
            {hasError ? "daemon failed" : "starting daemon"}
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
          <BootingBody tickGlyph={tickGlyph} fact={fact} />
        )}
      </div>
    </div>
  );
}

function BootingBody({
  tickGlyph,
  fact,
}: {
  tickGlyph: string;
  fact: string;
}) {
  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 14,
          marginBottom: 22,
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
          warming up the engine
        </div>
      </div>

      <div
        className="ba-mono"
        style={{
          fontSize: "var(--fs-xs)",
          textTransform: "uppercase",
          letterSpacing: "0.12em",
          color: "var(--fg-meta)",
          marginBottom: 8,
        }}
      >
        while you wait
      </div>
      <p
        style={{
          fontSize: "var(--fs-sm)",
          lineHeight: 1.55,
          color: "var(--fg)",
          margin: 0,
          minHeight: 88,
        }}
      >
        {fact}
      </p>
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
        the daemon couldn&rsquo;t start.
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
        failing, the Tauri terminal window has the daemon&rsquo;s
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
