/**
 * SVG-based span ruler showing word ranges above and token ranges below
 * a shared horizontal bar.
 */

interface SpanRulerProps {
  /** Original word texts. */
  words: string[];
  /** Stanza token texts. */
  tokens: string[];
  /** Character ranges for words in normalized form: [start, end). */
  wordRanges: [number, number][];
  /** Character ranges for tokens in normalized form: [start, end). */
  tokenRanges: [number, number][];
  /** Total character length of the normalized concatenation. */
  totalChars: number;
}

const BAR_Y = 50;
const BAR_HEIGHT = 4;
const SPAN_HEIGHT = 22;
const LABEL_OFFSET = 14;
const PADDING = 16;

export function SpanRuler({
  words,
  tokens,
  wordRanges,
  tokenRanges,
  totalChars,
}: SpanRulerProps) {
  if (totalChars === 0) return null;

  const width = 700;
  const height = BAR_Y + BAR_HEIGHT + SPAN_HEIGHT + LABEL_OFFSET + PADDING + 10;
  const scale = (charIdx: number) => (charIdx / totalChars) * (width - 2 * PADDING) + PADDING;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full max-w-3xl"
      style={{ height: "auto" }}
    >
      {/* Word spans (above bar) */}
      {wordRanges.map(([start, end], i) => {
        const x = scale(start);
        const w = scale(end) - x;
        const midX = x + w / 2;
        return (
          <g key={`w-${i}`}>
            <rect
              x={x}
              y={BAR_Y - SPAN_HEIGHT}
              width={Math.max(w, 2)}
              height={SPAN_HEIGHT}
              rx={3}
              className="fill-violet-100 stroke-violet-400"
              strokeWidth={1}
            />
            <text
              x={midX}
              y={BAR_Y - SPAN_HEIGHT + LABEL_OFFSET}
              textAnchor="middle"
              className="fill-violet-700 text-[10px] font-mono"
            >
              {words[i]}
            </text>
          </g>
        );
      })}

      {/* Central bar */}
      <rect
        x={PADDING}
        y={BAR_Y}
        width={width - 2 * PADDING}
        height={BAR_HEIGHT}
        rx={2}
        className="fill-gray-300"
      />

      {/* Token spans (below bar) */}
      {tokenRanges.map(([start, end], i) => {
        const x = scale(start);
        const w = scale(end) - x;
        const midX = x + w / 2;
        return (
          <g key={`t-${i}`}>
            <rect
              x={x}
              y={BAR_Y + BAR_HEIGHT + 4}
              width={Math.max(w, 2)}
              height={SPAN_HEIGHT}
              rx={3}
              className="fill-blue-100 stroke-blue-400"
              strokeWidth={1}
            />
            <text
              x={midX}
              y={BAR_Y + BAR_HEIGHT + 4 + LABEL_OFFSET}
              textAnchor="middle"
              className="fill-blue-700 text-[10px] font-mono"
            >
              {tokens[i]}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
