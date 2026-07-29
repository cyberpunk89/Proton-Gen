import { computeCommandScore } from "bits-ui";

export interface FuzzyMatch {
  /** Higher is better. Comparable only between results for the same query. */
  score: number;
  /** Half-open [start, end) spans of `haystack` to highlight. */
  ranges: [number, number][];
}

/**
 * Greedy left-to-right subsequence scan. Returns the matched character
 * indices, or null when `lowerQuery` is not a subsequence of `lowerHaystack`.
 *
 * Both arguments must already be lowercased — this is the hot path and is
 * called once per candidate per keystroke, so it does no work it can avoid.
 */
export function subsequence(
  lowerHaystack: string,
  lowerQuery: string,
): number[] | null {
  if (!lowerQuery) return [];
  const hits: number[] = [];
  let h = 0;
  for (let q = 0; q < lowerQuery.length; q++) {
    const c = lowerQuery[q];
    while (h < lowerHaystack.length && lowerHaystack[h] !== c) h++;
    if (h === lowerHaystack.length) return null;
    hits.push(h);
    h++;
  }
  return hits;
}

/** Collapses sorted, de-duplicated indices into contiguous [start, end) spans. */
function toRanges(indices: number[]): [number, number][] {
  if (indices.length === 0) return [];
  const sorted = [...new Set(indices)].sort((a, b) => a - b);
  const ranges: [number, number][] = [];
  let start = sorted[0];
  let prev = sorted[0];
  for (let i = 1; i < sorted.length; i++) {
    const n = sorted[i];
    if (n === prev + 1) {
      prev = n;
      continue;
    }
    ranges.push([start, prev + 1]);
    start = n;
    prev = n;
  }
  ranges.push([start, prev + 1]);
  return ranges;
}

/**
 * Rank `haystack` against `query`, returning a score and highlight ranges,
 * or null if it does not match.
 *
 * Two stages, because neither alone is sufficient:
 *
 * 1. `subsequence()` is the cheap prefilter AND the only source of match
 *    positions. Multi-word queries split on whitespace and are AND-ed, so
 *    "dxvk hdr" matches a haystack containing both regardless of order.
 * 2. Survivors are scored by bits-ui's `computeCommandScore` (the cmdk
 *    command-score algorithm), which ranks far better than we would but
 *    returns a scalar only — hence stage 1 for the ranges.
 */
export function fuzzy(
  haystack: string,
  query: string,
  keywords?: string[],
): FuzzyMatch | null {
  const q = query.trim();
  if (!q) return { score: 1, ranges: [] };

  const lowerHaystack = haystack.toLowerCase();
  const tokens = q.toLowerCase().split(/\s+/).filter(Boolean);

  // Every token must match somewhere, else this candidate is out.
  const indices: number[] = [];
  for (const token of tokens) {
    const hits = subsequence(lowerHaystack, token);
    if (!hits) {
      // Fall back to the keyword list before rejecting outright, so a token
      // that only appears in metadata (a category, an alias) still counts.
      if (!keywords?.some((k) => subsequence(k.toLowerCase(), token))) return null;
      continue;
    }
    indices.push(...hits);
  }

  const ranges = toRanges(indices);

  // computeCommandScore treats the query as one contiguous string, so it
  // returns exactly 0 for every multi-word query — "dxvk hdr" scores 0
  // against DXVK_HDR. Scoring the whole query would therefore flatten every
  // multi-word result to a single tied value and destroy the ranking.
  //
  // So: use the whole-query score when it is meaningful, and otherwise
  // average the per-token scores, which does discriminate (DXVK_HDR scores
  // 0.99 for "dxvk" on a prefix match vs PROTON_DXVK_GPLASYNC's 0.79
  // mid-word). Every candidate for a given query takes the same branch, so
  // scores stay comparable within a result set — which is all that ranking
  // requires.
  const whole = computeCommandScore(haystack, q, keywords);
  if (whole > 0) return { score: whole, ranges };

  let sum = 0;
  for (const token of tokens) sum += computeCommandScore(haystack, token, keywords);
  const score = tokens.length > 0 ? sum / tokens.length : 0;

  // Floor a matched-but-unscored candidate above zero so it still outranks a
  // non-match rather than being dropped by a `score > 0` filter downstream.
  return { score: score > 0 ? score : Number.EPSILON, ranges };
}
