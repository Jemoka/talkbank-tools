/**
 * TypeScript port of retokenize/mapping.rs for static visualization mode.
 *
 * Implements the same word-to-token mapping algorithm:
 * 1. Try deterministic span-join mapping when normalized text matches.
 * 2. Fall back to length-proportional monotonic mapping.
 */

/** Result of mapping original words to Stanza tokens. */
export interface RetokenizeResult {
  /** Original CHAT words. */
  originalWords: string[];
  /** Stanza tokens after retokenization. */
  stanzaTokens: string[];
  /** Normalized concatenation of original words. */
  normalizedOriginal: string;
  /** Normalized concatenation of Stanza tokens. */
  normalizedTokens: string;
  /** mapping[wordIdx] = list of token indices. */
  mapping: number[][];
  /** Character ranges for each original word in normalized form. */
  originalRanges: [number, number][];
  /** Character ranges for each token in normalized form. */
  tokenRanges: [number, number][];
  /** Whether the fallback (length-proportional) mapping was used. */
  usedFallback: boolean;
}

function normalizeAlignmentUnit(text: string): string {
  return text.toLowerCase();
}

/**
 * Try deterministic span-join mapping when normalized concatenated text
 * is identical on both sides.
 */
function tryDeterministicMapping(
  originalWords: string[],
  stanzaTokens: string[],
): RetokenizeResult | null {
  if (originalWords.length === 0 || stanzaTokens.length === 0) {
    return {
      originalWords,
      stanzaTokens,
      normalizedOriginal: "",
      normalizedTokens: "",
      mapping: originalWords.map(() => []),
      originalRanges: [],
      tokenRanges: [],
      usedFallback: false,
    };
  }

  const originalRanges: [number, number][] = [];
  const tokenRanges: [number, number][] = [];
  let originalConcat = "";
  let tokenConcat = "";

  let cursor = 0;
  for (const word of originalWords) {
    const normalized = normalizeAlignmentUnit(word);
    if (normalized.length === 0) return null;
    const len = [...normalized].length; // char count, not byte count
    originalRanges.push([cursor, cursor + len]);
    cursor += len;
    originalConcat += normalized;
  }

  cursor = 0;
  for (const token of stanzaTokens) {
    const normalized = normalizeAlignmentUnit(token);
    if (normalized.length === 0) return null;
    const len = [...normalized].length;
    tokenRanges.push([cursor, cursor + len]);
    cursor += len;
    tokenConcat += normalized;
  }

  if (originalConcat !== tokenConcat) return null;

  const mapping: number[][] = originalWords.map(() => []);
  let tokenIdx = 0;

  for (let wordIdx = 0; wordIdx < originalRanges.length; wordIdx++) {
    const [wordStart, wordEnd] = originalRanges[wordIdx];

    while (tokenIdx < tokenRanges.length && tokenRanges[tokenIdx][1] <= wordStart) {
      tokenIdx++;
    }

    let cursorIdx = tokenIdx;
    while (cursorIdx < tokenRanges.length) {
      const [tokenStart, tokenEnd] = tokenRanges[cursorIdx];
      if (tokenStart >= wordEnd) break;
      if (tokenEnd > wordStart) {
        mapping[wordIdx].push(cursorIdx);
      }
      cursorIdx++;
    }

    if (mapping[wordIdx].length === 0) return null;
  }

  return {
    originalWords,
    stanzaTokens,
    normalizedOriginal: originalConcat,
    normalizedTokens: tokenConcat,
    mapping,
    originalRanges,
    tokenRanges,
    usedFallback: false,
  };
}

/**
 * Length-proportional monotonic fallback (no DP).
 */
function buildLengthFallbackMapping(
  originalWords: string[],
  stanzaTokens: string[],
): number[][] {
  const wordCount = originalWords.length;
  const tokenCount = stanzaTokens.length;
  const mapping: number[][] = originalWords.map(() => []);

  if (wordCount === 0 || tokenCount === 0) return mapping;

  if (wordCount === tokenCount) {
    for (let i = 0; i < wordCount; i++) {
      mapping[i].push(i);
    }
    return mapping;
  }

  for (let wordIdx = 0; wordIdx < wordCount; wordIdx++) {
    const start = Math.floor((wordIdx * tokenCount) / wordCount);
    let end = Math.floor(((wordIdx + 1) * tokenCount) / wordCount);
    if (end <= start) {
      end = Math.min(start + 1, tokenCount);
    }
    for (let tokenIdx = start; tokenIdx < end; tokenIdx++) {
      mapping[wordIdx].push(tokenIdx);
    }
  }

  return mapping;
}

/**
 * Build a word-to-token mapping, matching the Rust implementation.
 */
export function buildWordTokenMapping(
  originalWords: string[],
  stanzaTokens: string[],
): RetokenizeResult {
  const deterministic = tryDeterministicMapping(originalWords, stanzaTokens);
  if (deterministic) return deterministic;

  const normalizedOriginal = originalWords.map(normalizeAlignmentUnit).join("");
  const normalizedTokens = stanzaTokens.map(normalizeAlignmentUnit).join("");

  // Build ranges for display even though we used fallback
  const originalRanges: [number, number][] = [];
  const tokenRanges: [number, number][] = [];
  let cursor = 0;
  for (const w of originalWords) {
    const len = [...normalizeAlignmentUnit(w)].length;
    originalRanges.push([cursor, cursor + len]);
    cursor += len;
  }
  cursor = 0;
  for (const t of stanzaTokens) {
    const len = [...normalizeAlignmentUnit(t)].length;
    tokenRanges.push([cursor, cursor + len]);
    cursor += len;
  }

  return {
    originalWords,
    stanzaTokens,
    normalizedOriginal,
    normalizedTokens,
    mapping: buildLengthFallbackMapping(originalWords, stanzaTokens),
    originalRanges,
    tokenRanges,
    usedFallback: true,
  };
}

/** Pre-populated sample data for static/educational mode. */
export const SAMPLE_DATA = {
  words: ["don't", "wanna", "go", "there"],
  tokens: ["do", "n't", "wan", "na", "go", "there"],
};
