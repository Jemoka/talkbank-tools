/**
 * TypeScript port of dp_align.rs `align_small` with step emission for
 * visualization. Only the full-table algorithm (not Hirschberg) since we
 * want the cost matrix for display.
 *
 * Cost model: match=0, substitution=2, gap=1.
 */

const COST_SUB = 2;
const COST_GAP = 1;

export type MatchMode = "exact" | "case_insensitive";

export type ActionKind = "start" | "match" | "substitution" | "extra_payload" | "extra_reference";

/** A single cell fill step during DP matrix construction. */
export interface FillStep {
  kind: "fill";
  /** Row index (reference axis, 0 = gap row). */
  i: number;
  /** Column index (payload axis, 0 = gap column). */
  j: number;
  /** Cost written to this cell. */
  cost: number;
  /** Action chosen for this cell. */
  action: ActionKind;
}

/** A single traceback step. */
export interface TracebackStep {
  kind: "traceback";
  i: number;
  j: number;
  action: ActionKind;
}

export type AlignStep = FillStep | TracebackStep;

export interface AlignResultItem {
  kind: "match" | "extra_payload" | "extra_reference";
  key: string;
  payloadIdx?: number;
  referenceIdx?: number;
}

export interface DpAlignmentResult {
  /** Payload sequence. */
  payload: string[];
  /** Reference sequence. */
  reference: string[];
  /** Number of prefix elements stripped. */
  prefixStripped: number;
  /** Number of suffix elements stripped. */
  suffixStripped: number;
  /** Flat cost matrix, row-major, (refLen+1) * (payLen+1). */
  costMatrix: number[];
  /** Step-by-step trace of the fill + traceback. */
  steps: AlignStep[];
  /** Final alignment result. */
  result: AlignResultItem[];
  /** Rows in cost matrix. */
  rows: number;
  /** Columns in cost matrix. */
  cols: number;
}

function matches(a: string, b: string, mode: MatchMode): boolean {
  if (mode === "exact") return a === b;
  return a.toLowerCase() === b.toLowerCase();
}

/**
 * Run DP alignment with step-by-step emission for visualization.
 *
 * Uses the full-table algorithm (not Hirschberg) so we can display the
 * cost matrix. Includes prefix/suffix stripping.
 */
export function alignWithSteps(
  payload: string[],
  reference: string[],
  mode: MatchMode = "exact",
): DpAlignmentResult {
  // Strip common prefix
  let prefixLen = 0;
  while (
    prefixLen < payload.length &&
    prefixLen < reference.length &&
    matches(payload[prefixLen], reference[prefixLen], mode)
  ) {
    prefixLen++;
  }

  // Strip common suffix
  let suffixLen = 0;
  while (
    suffixLen < payload.length - prefixLen &&
    suffixLen < reference.length - prefixLen &&
    matches(
      payload[payload.length - 1 - suffixLen],
      reference[reference.length - 1 - suffixLen],
      mode,
    )
  ) {
    suffixLen++;
  }

  const midPay = payload.slice(prefixLen, payload.length - suffixLen);
  const midRef = reference.slice(prefixLen, reference.length - suffixLen);

  const rows = midRef.length + 1;
  const cols = midPay.length + 1;

  // dp[i * cols + j] = (cost, action, prevI, prevJ)
  const costs = new Array<number>(rows * cols).fill(0);
  const actions = new Array<ActionKind>(rows * cols).fill("start");
  const prevI = new Array<number>(rows * cols).fill(0);
  const prevJ = new Array<number>(rows * cols).fill(0);
  const idx = (r: number, c: number) => r * cols + c;

  const steps: AlignStep[] = [];

  // Initialize first column
  for (let i = 1; i < rows; i++) {
    costs[idx(i, 0)] = i * COST_GAP;
    actions[idx(i, 0)] = "extra_reference";
    prevI[idx(i, 0)] = i - 1;
    prevJ[idx(i, 0)] = 0;
    steps.push({ kind: "fill", i, j: 0, cost: i * COST_GAP, action: "extra_reference" });
  }

  // Initialize first row
  for (let j = 1; j < cols; j++) {
    costs[idx(0, j)] = j * COST_GAP;
    actions[idx(0, j)] = "extra_payload";
    prevI[idx(0, j)] = 0;
    prevJ[idx(0, j)] = j - 1;
    steps.push({ kind: "fill", i: 0, j, cost: j * COST_GAP, action: "extra_payload" });
  }

  // Fill the matrix
  for (let i = 1; i < rows; i++) {
    for (let j = 1; j < cols; j++) {
      const isMatch = matches(midRef[i - 1], midPay[j - 1], mode);
      const subCost = costs[idx(i - 1, j - 1)] + (isMatch ? 0 : COST_SUB);
      const delCost = costs[idx(i - 1, j)] + COST_GAP;
      const insCost = costs[idx(i, j - 1)] + COST_GAP;

      let action: ActionKind;
      let cost: number;
      let pi: number;
      let pj: number;

      if (subCost <= delCost && subCost <= insCost) {
        action = isMatch ? "match" : "substitution";
        cost = subCost;
        pi = i - 1;
        pj = j - 1;
      } else if (delCost <= subCost && delCost <= insCost) {
        action = "extra_reference";
        cost = delCost;
        pi = i - 1;
        pj = j;
      } else {
        action = "extra_payload";
        cost = insCost;
        pi = i;
        pj = j - 1;
      }

      costs[idx(i, j)] = cost;
      actions[idx(i, j)] = action;
      prevI[idx(i, j)] = pi;
      prevJ[idx(i, j)] = pj;

      steps.push({ kind: "fill", i, j, cost, action });
    }
  }

  // Traceback
  const tracebackSteps: TracebackStep[] = [];
  const output: AlignResultItem[] = [];
  let ti = rows - 1;
  let tj = cols - 1;

  while (ti > 0 || tj > 0) {
    const k = idx(ti, tj);
    const action = actions[k];
    const pi = prevI[k];
    const pj = prevJ[k];

    tracebackSteps.push({ kind: "traceback", i: ti, j: tj, action });

    switch (action) {
      case "match":
        output.push({
          kind: "match",
          key: midRef[pi].toString(),
          payloadIdx: prefixLen + pj,
          referenceIdx: prefixLen + pi,
        });
        break;
      case "substitution":
        output.push({
          kind: "extra_payload",
          key: midPay[pj],
          payloadIdx: prefixLen + pj,
        });
        output.push({
          kind: "extra_reference",
          key: midRef[pi],
          referenceIdx: prefixLen + pi,
        });
        break;
      case "extra_payload":
        output.push({
          kind: "extra_payload",
          key: midPay[pj],
          payloadIdx: prefixLen + pj,
        });
        break;
      case "extra_reference":
        output.push({
          kind: "extra_reference",
          key: midRef[pi],
          referenceIdx: prefixLen + pi,
        });
        break;
      case "start":
        break;
    }

    ti = pi;
    tj = pj;
  }

  output.reverse();

  // Prepend prefix matches
  const prefixResults: AlignResultItem[] = [];
  for (let i = 0; i < prefixLen; i++) {
    prefixResults.push({
      kind: "match",
      key: reference[i],
      payloadIdx: i,
      referenceIdx: i,
    });
  }

  // Append suffix matches
  const suffixResults: AlignResultItem[] = [];
  const paySuffStart = payload.length - suffixLen;
  const refSuffStart = reference.length - suffixLen;
  for (let i = 0; i < suffixLen; i++) {
    suffixResults.push({
      kind: "match",
      key: reference[refSuffStart + i],
      payloadIdx: paySuffStart + i,
      referenceIdx: refSuffStart + i,
    });
  }

  return {
    payload,
    reference,
    prefixStripped: prefixLen,
    suffixStripped: suffixLen,
    costMatrix: costs,
    steps: [...steps, ...tracebackSteps],
    result: [...prefixResults, ...output, ...suffixResults],
    rows,
    cols,
  };
}

/** Pre-populated sample data for static/educational mode. */
export const SAMPLE_PAYLOAD = ["the", "cat", "sat"];
export const SAMPLE_REFERENCE = ["the", "big", "cat", "sat"];
