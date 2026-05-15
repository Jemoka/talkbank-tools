/** ASR Pipeline Waterfall — coming in Phase 3. */

import { ModeToggle } from "../../components/visualizations/ModeToggle";

export function ASRPipelinePage() {
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
            <span className="font-mono text-sm font-semibold text-white/90">
              ASR Pipeline Waterfall
            </span>
          </div>
          <ModeToggle mode="static" />
        </div>
      </header>
      <main className="max-w-5xl mx-auto px-4 py-6">
        <div className="text-sm text-gray-500 py-16 text-center">
          <p className="font-medium text-gray-700 mb-2">Coming in Phase 3</p>
          <p>
            This visualization will show raw ASR tokens flowing through 7 post-processing
            stages (compound merging, timing extraction, multi-word splitting, number
            expansion, Cantonese normalization, long-turn splitting, retokenization) to
            produce final utterances.
          </p>
        </div>
      </main>
    </div>
  );
}
