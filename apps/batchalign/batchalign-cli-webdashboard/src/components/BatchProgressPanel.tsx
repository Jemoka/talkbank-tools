/**
 * Per-language-group progress panel for batched text commands.
 *
 * Rendered on the job detail page when the job is running a batched command
 * (morphotag, utseg, translate, coref) and batch_progress data is available.
 * Shows one progress bar per language with utterance counts and an overall
 * summary.
 */

/** Language group progress from the server API. */
interface LanguageGroupProgress {
  lang: string;
  completed_utterances: number;
  total_utterances: number;
}

/** Aggregate batch progress from the server API. */
interface BatchInferProgress {
  language_groups: Record<string, LanguageGroupProgress>;
}

export function BatchProgressPanel({
  progress,
}: {
  progress: BatchInferProgress | null | undefined;
}) {
  if (!progress) return null;

  const groups = Object.values(progress.language_groups).sort((a, b) => {
    // Sort: in-progress first, then completed, alphabetical within each.
    const aDone = a.completed_utterances >= a.total_utterances;
    const bDone = b.completed_utterances >= b.total_utterances;
    if (aDone !== bDone) return aDone ? 1 : -1;
    return a.lang.localeCompare(b.lang);
  });

  if (groups.length === 0) return null;

  const totalUtterances = groups.reduce((s, g) => s + g.total_utterances, 0);
  const completedUtterances = groups.reduce(
    (s, g) => s + g.completed_utterances,
    0,
  );
  const completedGroups = groups.filter(
    (g) => g.completed_utterances >= g.total_utterances,
  ).length;
  const overallPct =
    totalUtterances > 0
      ? Math.round((100 * completedUtterances) / totalUtterances)
      : 100;

  return (
    <div className="mt-4 rounded-lg border border-blue-100 bg-blue-50/50 px-4 py-3">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-blue-600 mb-3">
        Batch Progress
      </h3>

      <div className="space-y-2">
        {groups.map((g) => (
          <LanguageRow key={g.lang} group={g} />
        ))}
      </div>

      <div className="mt-3 pt-2 border-t border-blue-100 text-xs text-blue-700">
        {completedGroups}/{groups.length} languages done,{" "}
        {completedUtterances.toLocaleString()}/
        {totalUtterances.toLocaleString()} utterances ({overallPct}%)
      </div>
    </div>
  );
}

function LanguageRow({ group }: { group: LanguageGroupProgress }) {
  const { lang, completed_utterances, total_utterances } = group;
  const isDone = completed_utterances >= total_utterances;
  const pct =
    total_utterances > 0
      ? Math.round((100 * completed_utterances) / total_utterances)
      : 100;

  return (
    <div className="flex items-center gap-3 text-sm">
      <span className="w-8 font-mono text-xs text-zinc-600 uppercase">
        {lang}
      </span>
      <div className="flex-1 h-2 bg-blue-100 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-300 ${
            isDone ? "bg-emerald-400" : "bg-blue-400"
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="w-24 text-right text-xs text-zinc-500 tabular-nums">
        {isDone ? (
          <span className="text-emerald-600">done</span>
        ) : (
          `${completed_utterances}/${total_utterances}`
        )}
      </span>
    </div>
  );
}
