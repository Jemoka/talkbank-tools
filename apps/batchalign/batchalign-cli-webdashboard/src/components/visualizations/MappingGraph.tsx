/**
 * SVG bipartite graph showing word→token mappings.
 * 1:1 = gray, 1:N fan-out = green, N:1 fan-in = orange.
 */

interface MappingGraphProps {
  /** Original word texts. */
  words: string[];
  /** Stanza token texts. */
  tokens: string[];
  /** mapping[wordIdx] = list of token indices. */
  mapping: number[][];
}

const BOX_W = 80;
const BOX_H = 28;
const GAP_Y = 60;
const PAD_X = 20;
const PAD_Y = 16;

function boxX(idx: number, count: number, totalWidth: number): number {
  if (count === 0) return 0;
  const totalUsed = count * BOX_W + (count - 1) * 8;
  const startX = (totalWidth - totalUsed) / 2;
  return startX + idx * (BOX_W + 8);
}

export function MappingGraph({ words, tokens, mapping }: MappingGraphProps) {
  const maxCount = Math.max(words.length, tokens.length, 1);
  const width = Math.max(maxCount * (BOX_W + 8) + 2 * PAD_X, 300);
  const height = BOX_H * 2 + GAP_Y + 2 * PAD_Y;

  const wordY = PAD_Y;
  const tokenY = PAD_Y + BOX_H + GAP_Y;

  // Compute inverse mapping (token → which words map to it)
  const inverseCount = new Array(tokens.length).fill(0);
  for (const wordTokens of mapping) {
    for (const ti of wordTokens) {
      if (ti < tokens.length) inverseCount[ti]++;
    }
  }

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full max-w-4xl"
      style={{ height: "auto" }}
    >
      {/* Lines connecting words to tokens */}
      {mapping.map((tokenIndices, wordIdx) => {
        const wx = boxX(wordIdx, words.length, width) + BOX_W / 2;
        const wy = wordY + BOX_H;
        const isFanOut = tokenIndices.length > 1;

        return tokenIndices.map((tokenIdx) => {
          const tx = boxX(tokenIdx, tokens.length, width) + BOX_W / 2;
          const ty = tokenY;
          const isFanIn = inverseCount[tokenIdx] > 1;

          let strokeClass = "stroke-gray-300";
          if (isFanOut) strokeClass = "stroke-emerald-400";
          if (isFanIn) strokeClass = "stroke-orange-400";

          return (
            <line
              key={`${wordIdx}-${tokenIdx}`}
              x1={wx}
              y1={wy}
              x2={tx}
              y2={ty}
              className={strokeClass}
              strokeWidth={2}
              strokeOpacity={0.7}
            />
          );
        });
      })}

      {/* Word boxes (top row) */}
      {words.map((word, i) => {
        const x = boxX(i, words.length, width);
        return (
          <g key={`w-${i}`}>
            <rect
              x={x}
              y={wordY}
              width={BOX_W}
              height={BOX_H}
              rx={4}
              className="fill-violet-50 stroke-violet-300"
              strokeWidth={1}
            />
            <text
              x={x + BOX_W / 2}
              y={wordY + BOX_H / 2 + 4}
              textAnchor="middle"
              className="fill-violet-700 text-[11px] font-mono"
            >
              {word.length > 10 ? word.slice(0, 9) + "\u2026" : word}
            </text>
          </g>
        );
      })}

      {/* Token boxes (bottom row) */}
      {tokens.map((token, i) => {
        const x = boxX(i, tokens.length, width);
        return (
          <g key={`t-${i}`}>
            <rect
              x={x}
              y={tokenY}
              width={BOX_W}
              height={BOX_H}
              rx={4}
              className="fill-blue-50 stroke-blue-300"
              strokeWidth={1}
            />
            <text
              x={x + BOX_W / 2}
              y={tokenY + BOX_H / 2 + 4}
              textAnchor="middle"
              className="fill-blue-700 text-[11px] font-mono"
            >
              {token.length > 10 ? token.slice(0, 9) + "\u2026" : token}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
