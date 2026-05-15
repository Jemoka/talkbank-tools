/**
 * Retokenization Mapper visualization.
 *
 * Static mode: two text inputs → span ruler + bipartite mapping graph.
 * Live mode: shows actual retokenization mappings from a job trace.
 */

import { useState, useMemo } from "react";
import { useParams } from "wouter";
import { ModeToggle } from "../../components/visualizations/ModeToggle";
import { SpanRuler } from "../../components/visualizations/SpanRuler";
import { MappingGraph } from "../../components/visualizations/MappingGraph";
import { buildWordTokenMapping, SAMPLE_DATA } from "../../engines/retokenize";
import { useTraceQuery } from "../../hooks/useTraceQuery";

export function RetokenizationPage() {
  const params = useParams<{ id?: string }>();
  const jobId = params.id;
  const isLive = !!jobId;

  // Static mode state
  const [wordsInput, setWordsInput] = useState(SAMPLE_DATA.words.join(" "));
  const [tokensInput, setTokensInput] = useState(SAMPLE_DATA.tokens.join(" "));

  // Live mode
  const { data: traces } = useTraceQuery(jobId);
  const [selectedFile, setSelectedFile] = useState(0);
  const [selectedUtterance, setSelectedUtterance] = useState(0);

  // Compute mapping
  const result = useMemo(() => {
    if (isLive && traces) {
      const fileKeys = Object.keys(traces.files).sort((a, b) => Number(a) - Number(b));
      const fileTraces = traces.files[fileKeys[selectedFile]];
      if (fileTraces && fileTraces.retokenizations.length > 0) {
        const rt = fileTraces.retokenizations[Math.min(selectedUtterance, fileTraces.retokenizations.length - 1)];
        return buildWordTokenMapping(rt.original_words, rt.stanza_tokens);
      }
      return null;
    }

    const words = wordsInput
      .trim()
      .split(/\s+/)
      .filter((w) => w.length > 0);
    const tokens = tokensInput
      .trim()
      .split(/\s+/)
      .filter((t) => t.length > 0);
    if (words.length === 0 && tokens.length === 0) return null;
    return buildWordTokenMapping(words, tokens);
  }, [isLive, traces, selectedFile, selectedUtterance, wordsInput, tokensInput]);

  return (
    <div className="min-h-screen">
      <header className="bg-[var(--bg-header)] text-white">
        <div className="max-w-5xl mx-auto px-4 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <a href="/dashboard/visualizations" className="text-white/50 hover:text-white/80 text-xs no-underline">
              Visualizations
            </a>
            <span className="text-white/30">/</span>
            <span className="font-mono text-sm font-semibold text-white/90">
              Retokenization Mapper
            </span>
          </div>
          <ModeToggle mode={isLive ? "live" : "static"} />
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-4 py-6">
        {/* Input section */}
        {!isLive && (
          <div className="mb-6 space-y-3">
            <div>
              <label className="text-xs font-medium text-gray-600 block mb-1">
                Original words (space-separated)
              </label>
              <input
                type="text"
                value={wordsInput}
                onChange={(e) => setWordsInput(e.target.value)}
                className="w-full px-3 py-2 rounded border border-gray-200 text-sm font-mono focus:border-violet-400 focus:outline-none"
                placeholder="don't wanna go there"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-gray-600 block mb-1">
                Stanza tokens (space-separated)
              </label>
              <input
                type="text"
                value={tokensInput}
                onChange={(e) => setTokensInput(e.target.value)}
                className="w-full px-3 py-2 rounded border border-gray-200 text-sm font-mono focus:border-blue-400 focus:outline-none"
                placeholder="do n't wan na go there"
              />
            </div>
          </div>
        )}

        {isLive && traces && (
          <div className="mb-6 flex items-center gap-4">
            <div>
              <label className="text-xs font-medium text-gray-600 block mb-1">File</label>
              <select
                value={selectedFile}
                onChange={(e) => {
                  setSelectedFile(Number(e.target.value));
                  setSelectedUtterance(0);
                }}
                className="px-2 py-1 rounded border border-gray-200 text-sm"
              >
                {Object.entries(traces.files).map(([idx, ft]) => (
                  <option key={idx} value={Number(idx)}>
                    {ft.filename}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-xs font-medium text-gray-600 block mb-1">Utterance</label>
              <select
                value={selectedUtterance}
                onChange={(e) => setSelectedUtterance(Number(e.target.value))}
                className="px-2 py-1 rounded border border-gray-200 text-sm"
              >
                {(() => {
                  const fileKeys = Object.keys(traces.files).sort(
                    (a, b) => Number(a) - Number(b),
                  );
                  const ft = traces.files[fileKeys[selectedFile]];
                  if (!ft) return null;
                  return ft.retokenizations.map((rt, i) => (
                    <option key={i} value={i}>
                      Utterance {rt.utterance_index}
                    </option>
                  ));
                })()}
              </select>
            </div>
          </div>
        )}

        {isLive && !traces && (
          <div className="text-sm text-gray-500 py-8 text-center">
            No traces available. The job may not have been submitted with debug_traces enabled.
          </div>
        )}

        {result && (
          <div className="space-y-8">
            {/* Fallback indicator */}
            {result.usedFallback && (
              <div className="text-xs px-3 py-2 bg-amber-50 border border-amber-200 rounded text-amber-700">
                Normalized text diverged — using length-proportional fallback mapping (no DP).
              </div>
            )}

            {/* Section 1: Normalization info */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Normalized Text
              </h2>
              <div className="font-mono text-sm space-y-1">
                <div>
                  <span className="text-violet-600">Words:</span>{" "}
                  <span className="text-gray-700">{result.normalizedOriginal || "(empty)"}</span>
                </div>
                <div>
                  <span className="text-blue-600">Tokens:</span>{" "}
                  <span className="text-gray-700">{result.normalizedTokens || "(empty)"}</span>
                </div>
                {result.normalizedOriginal === result.normalizedTokens && (
                  <div className="text-emerald-600 text-xs">Normalized text matches exactly.</div>
                )}
              </div>
            </section>

            {/* Section 2: Span Ruler */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Span Ruler
              </h2>
              <SpanRuler
                words={result.originalWords}
                tokens={result.stanzaTokens}
                wordRanges={result.originalRanges}
                tokenRanges={result.tokenRanges}
                totalChars={result.normalizedOriginal.length}
              />
            </section>

            {/* Section 3: Mapping Graph */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Word → Token Mapping
              </h2>
              <MappingGraph
                words={result.originalWords}
                tokens={result.stanzaTokens}
                mapping={result.mapping}
              />
              <div className="flex gap-4 mt-2 text-[10px] text-gray-400">
                <span>
                  <span className="inline-block w-3 h-0.5 bg-gray-300 mr-1 align-middle" />
                  1:1
                </span>
                <span>
                  <span className="inline-block w-3 h-0.5 bg-emerald-400 mr-1 align-middle" />
                  1:N fan-out
                </span>
                <span>
                  <span className="inline-block w-3 h-0.5 bg-orange-400 mr-1 align-middle" />
                  N:1 fan-in
                </span>
              </div>
            </section>

            {/* Section 4: Raw mapping table */}
            <section>
              <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
                Index Mapping
              </h2>
              <div className="overflow-x-auto">
                <table className="text-xs font-mono border-collapse">
                  <thead>
                    <tr className="text-gray-500">
                      <th className="pr-4 text-left font-medium">Word Idx</th>
                      <th className="pr-4 text-left font-medium">Word</th>
                      <th className="pr-4 text-left font-medium">Token Indices</th>
                      <th className="text-left font-medium">Tokens</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.mapping.map((tokenIndices, wordIdx) => (
                      <tr key={wordIdx} className="border-t border-gray-100">
                        <td className="pr-4 py-1 text-gray-400">{wordIdx}</td>
                        <td className="pr-4 py-1 text-violet-700">
                          {result.originalWords[wordIdx]}
                        </td>
                        <td className="pr-4 py-1 text-gray-500">[{tokenIndices.join(", ")}]</td>
                        <td className="py-1 text-blue-700">
                          {tokenIndices.map((ti) => result.stanzaTokens[ti]).join(" ")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </div>
        )}
      </main>
    </div>
  );
}
