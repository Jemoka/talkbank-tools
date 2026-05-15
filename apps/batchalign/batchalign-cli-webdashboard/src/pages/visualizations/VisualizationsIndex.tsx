/** Landing page listing all available algorithm visualizations. */

const VISUALIZATIONS = [
  {
    name: "Retokenization Mapper",
    description: "See how CHAT words map to Stanza tokens through normalization and span-join.",
    href: "/dashboard/visualizations/retokenize",
    status: "ready" as const,
  },
  {
    name: "DP Alignment Explorer",
    description:
      "Step through the Hirschberg cost matrix fill and traceback for sequence alignment.",
    href: "/dashboard/visualizations/dp-alignment",
    status: "ready" as const,
  },
  {
    name: "ASR Pipeline Waterfall",
    description: "Follow raw ASR tokens through 7 post-processing stages to final utterances.",
    href: "/dashboard/visualizations/asr-pipeline",
    status: "coming" as const,
  },
  {
    name: "FA Timeline",
    description:
      "DAW-style timeline showing utterance grouping, forced alignment timing, and post-processing.",
    href: "/dashboard/visualizations/fa-timeline",
    status: "coming" as const,
  },
];

export function VisualizationsIndex() {
  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">Algorithm Visualizations</h1>
      <p className="text-sm text-gray-500 mb-6">
        Interactive explorations of batchalign3's core algorithms. Static mode uses sample data;
        live mode shows real traces from completed jobs.
      </p>
      <div className="grid gap-4 sm:grid-cols-2">
        {VISUALIZATIONS.map((viz) => (
          <a
            key={viz.href}
            href={viz.status === "ready" ? viz.href : undefined}
            className={`block p-4 rounded-lg border transition-colors ${
              viz.status === "ready"
                ? "border-gray-200 hover:border-blue-300 hover:bg-blue-50/50 cursor-pointer"
                : "border-gray-100 bg-gray-50 opacity-60 cursor-default"
            }`}
          >
            <div className="flex items-center gap-2 mb-1">
              <span className="font-medium text-sm">{viz.name}</span>
              {viz.status === "coming" && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-200 text-gray-500">
                  Coming soon
                </span>
              )}
            </div>
            <p className="text-xs text-gray-500">{viz.description}</p>
          </a>
        ))}
      </div>
    </div>
  );
}
