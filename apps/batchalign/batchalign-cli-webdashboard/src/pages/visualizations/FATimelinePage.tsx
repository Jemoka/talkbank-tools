/** FA Timeline — coming in Phase 4. */

import { ModeToggle } from "../../components/visualizations/ModeToggle";

export function FATimelinePage() {
  return (
    <div className="min-h-screen">
      <header className="bg-[var(--bg-header)] text-white">
        <div className="max-w-5xl mx-auto px-4 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <a
              href="/dashboard/visualizations"
              className="text-white/50 hover:text-white/80 text-xs no-underline"
            >
              Visualizations
            </a>
            <span className="text-white/30">/</span>
            <span className="font-mono text-sm font-semibold text-white/90">FA Timeline</span>
          </div>
          <ModeToggle mode="static" />
        </div>
      </header>
      <main className="max-w-5xl mx-auto px-4 py-6">
        <div className="text-sm text-gray-500 py-16 text-center">
          <p className="font-medium text-gray-700 mb-2">Coming in Phase 4</p>
          <p>
            This visualization will show a DAW-style SVG timeline with utterance grouping,
            forced alignment timing injection, and post-processing (continuous vs with-pauses
            modes, E362/E704 violation highlighting).
          </p>
        </div>
      </main>
    </div>
  );
}
